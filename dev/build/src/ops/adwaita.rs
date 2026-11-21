//! `adwaita`: the Adwaita icon set a listing draws with, fetched at the release it pins.

use std::collections::BTreeSet;
use std::path::Path;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::fs;
use crate::paths::Workspace;
use crate::run_step;
use crate::toolchain::Tool;
use crate::{Error, Next};

/// Where the set is kept, from the root of the tree.
pub const FOLDER: &str = "res/desktop-icon-set/adwaita";

/// The release the pictures are fetched at.
///
/// A release rather than a branch, because a branch is a moving answer and a listing drawn from a
/// different theme every day is a listing that changes without a commit. [`STAMP`] records it.
pub const RELEASE: &str = "51.0";

/// The checksum the release tarball states, recorded beside the set for a reader who checks it.
const SHA256: &str = "ba561cf3c96305a47179fa5605856ad695c9238ea07f7093aebd9ba80c4e323b";

/// Where the release's files are fetched from.
const RAW: &str = "https://gitlab.gnome.org/GNOME/adwaita-icon-theme/-/raw";

/// The contexts a picture may be kept in, the places first.
const CONTEXTS: [&str; 2] = ["places", "mimetypes"];

/// The license the artwork is distributed under, fetched with it.
const LICENSE: &str = "COPYING_LGPL";

/// The file the release is recorded in.
const STAMP: &str = "VERSION";

/// The set's own file, which names every picture the set draws.
const THEME: &str = "icon-theme.toml";

/// Fetches the pictures the set names, where the build reads them.
///
/// The set's own file is the list: what `[fallback]` and `[[rules]]` name is what is fetched, so
/// another picture is a line in that file rather than a second list here. A set that records the
/// pinned release and holds every file is left as it is, so a build that already has it reaches the
/// network for nothing.
///
/// # Errors
///
/// Reports [`Error::Manifest`] when the set's file will not read, [`Error::Status`] when the license
/// will not download, and [`Error::Missing`] when a picture is in neither context.
pub fn fetch_adwaita(workspace: &Workspace) -> Result<(), Error> {
    let folder = workspace.root().join(FOLDER);
    let sources = sources(&folder)?;

    if settled(&folder, &sources) {
        return Ok(());
    }

    fs::create_directory(&folder)?;

    for name in &sources {
        fetch(&folder, name)?;
    }

    let license = folder.join(LICENSE);

    if !settled_file(&license) {
        let temporary = folder.join(format!("{LICENSE}.tmp"));

        fs::remove(&temporary)?;
        download(&temporary, &format!("{RAW}/{RELEASE}/{LICENSE}"))?;
        replace(&temporary, &license)?;
    }

    record(&folder)?;

    Ok(())
}

/// The pictures the set names, in the order they are fetched.
///
/// The paths are read from the set's own file rather than written here a second time, and they are
/// names rather than paths because the set keeps its pictures flat: the cache names an output after
/// the source's own name, so two pictures of one name would be one picture in the cache.
fn sources(folder: &Path) -> Result<BTreeSet<String>, Error> {
    let theme = folder.join(THEME);
    let text = std::fs::read_to_string(&theme).map_err(|error| Error::Manifest {
        path: theme.clone(),
        detail: error.to_string(),
    })?;
    let document = text
        .parse::<toml::Value>()
        .map_err(|error| Error::Manifest {
            path: theme.clone(),
            detail: error.to_string(),
        })?;

    let mut sources = BTreeSet::new();

    if let Some(fallback) = document.get("fallback").and_then(toml::Value::as_table) {
        for value in fallback.values() {
            adding(&mut sources, &theme, value)?;
        }
    }

    if let Some(rules) = document.get("rules").and_then(toml::Value::as_array) {
        for rule in rules {
            if let Some(icon) = rule.get("icon") {
                adding(&mut sources, &theme, icon)?;
            }
        }
    }

    Ok(sources)
}

/// Adds one path the set names, refusing a path the set does not keep flat.
fn adding(sources: &mut BTreeSet<String>, theme: &Path, value: &toml::Value) -> Result<(), Error> {
    let Some(path) = value.as_str() else {
        return Err(Error::Manifest {
            path: theme.to_path_buf(),
            detail: format!("`{value}` is not a path"),
        });
    };

    let Some(name) = path.strip_prefix("./") else {
        return Err(Error::Manifest {
            path: theme.to_path_buf(),
            detail: format!("`{path}` is not relative to the set"),
        });
    };

    if name.contains('/') || !name.ends_with(".svg") {
        return Err(Error::Manifest {
            path: theme.to_path_buf(),
            detail: format!("`{path}` is not a flat SVG name"),
        });
    }

    sources.insert(name.to_owned());

    Ok(())
}

/// Whether the set records the pinned release and holds every file it names.
fn settled(folder: &Path, sources: &BTreeSet<String>) -> bool {
    let recorded = std::fs::read_to_string(folder.join(STAMP)).unwrap_or_default();

    recorded
        .lines()
        .next()
        .is_some_and(|line| line.trim() == RELEASE)
        && sources.iter().all(|name| settled_file(&folder.join(name)))
        && settled_file(&folder.join(LICENSE))
}

/// Whether a file is there and holds something.
///
/// A file that is there but empty is not there: a download that was cut short leaves one.
fn settled_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

/// Fetches one picture, trying each context the theme keeps pictures in.
fn fetch(folder: &Path, name: &str) -> Result<(), Error> {
    let destination = folder.join(name);

    if settled_file(&destination) {
        return Ok(());
    }

    let temporary = folder.join(format!("{name}.tmp"));
    let mut tried = Vec::with_capacity(CONTEXTS.len());

    for context in CONTEXTS {
        let url = format!("{RAW}/{RELEASE}/Adwaita/scalable/{context}/{name}");

        fs::remove(&temporary)?;

        if probe(&temporary, &url).is_ok() {
            return replace(&temporary, &destination);
        }

        tried.push(url);
    }

    fs::remove(&temporary)?;

    Err(Error::Missing {
        summary: format!("the Adwaita picture `{name}` is not in release {RELEASE}"),
        advice: tried
            .into_iter()
            .map(|url| format!("check that {url} is there"))
            .collect(),
    })
}

/// Fetches one file, named by its path in the release.
fn download(destination: &Path, url: &str) -> Result<(), Error> {
    let destination = destination.to_string_lossy();

    Tool::curl().run([
        "-fsSL",
        "--retry",
        "3",
        "--retry-delay",
        "2",
        "--output",
        destination.as_ref(),
        url,
    ])
}

/// Probes one context for a picture, saying nothing about the contexts that do not hold it.
///
/// A name is in one of the two contexts, so a miss is how the search works rather than a failure a
/// reader has to read. What is fetched is written beside its own name, so the probe is the same
/// download with the error stream left quiet.
fn probe(destination: &Path, url: &str) -> Result<(), Error> {
    let destination = destination.to_string_lossy();

    Tool::curl().run([
        "-fsL",
        "--retry",
        "3",
        "--retry-delay",
        "2",
        "--output",
        destination.as_ref(),
        url,
    ])
}

/// Moves a fetched file onto its name, so a reader never sees half of it.
fn replace(from: &Path, to: &Path) -> Result<(), Error> {
    std::fs::rename(from, to).map_err(|error| Error::Io {
        doing: "moving a fetched picture into the set",
        detail: format!("{} -> {}: {error}", from.display(), to.display()),
    })
}

/// Records the release the pictures were fetched at, last.
fn record(folder: &Path) -> Result<(), Error> {
    let stamp = folder.join(STAMP);
    let temporary = folder.join(format!("{STAMP}.tmp"));
    let text = format!(
        "{RELEASE}\nsha256 {SHA256}\nhttps://download.gnome.org/sources/adwaita-icon-theme/51/adwaita-icon-theme-{RELEASE}.tar.xz\n"
    );

    fs::remove(&temporary)?;
    std::fs::write(&temporary, text).map_err(|error| Error::Io {
        doing: "recording the Adwaita release",
        detail: format!("{}: {error}", temporary.display()),
    })?;
    replace(&temporary, &stamp)
}

/// Fetches the Adwaita pictures the set names.
#[command(node = "adwaita")]
pub fn adwaita() -> StateAdwaita {
    StateAdwaita
}

/// The state the command starts in: the set's own file is the whole of what it names.
#[derive(Grouped)]
pub struct StateAdwaita;

/// Fetches the pictures, where the build reads them.
#[chain]
pub fn handle_adwaita(_state: StateAdwaita, ec: &mut ResExitCode) -> Next {
    run_step(ec, || fetch_adwaita(&Workspace::discover()?));

    empty_result!()
}
