//! The `bring` command: signing the entries of a Vault's Layout in to this project.
//!
//! The operands read the way `rola bring` reads its own: every word but the last is a reference (a
//! path in the Vault's Layout, a glob over those paths, or a `Uuid`), and the last, when there is
//! more than one, is where those references land. A reference always comes from the Vault's side;
//! only the destination is read as a path of this run's.
//!
//! What comes in is written down in three places at once: the mapping in `rola.toml`, the material
//! in `rola.lock`, and the content in the work tree. The mapping is appended as text rather than
//! serialized, so the comments and the shape a person gave the file stay as they are; the lock is
//! the file the project carries with this run's material appended; and only the materials this run
//! brought are placed.
//!
//! The reading of the operands is `rola bring`'s (`app/cli/rola/src/cmd_bring.rs`, the `split` down
//! to `landing_taken`): the same words mean the same thing in both programs, and a change to one
//! belongs in the other. The Vault side is [`upstream`]'s, shared with `update`.

// mingling's `#[command]` expands the entry type and the dispatch function beside the command, and
// the expansion is not documented; the items this module writes carry their own docs.
#![allow(missing_docs)]
// A command's words arrive as the value the framework's entry converts to, so the parameter is the
// framework's shape rather than this module's choice: taking them by reference is not an option the
// generated wrapper offers.
#![allow(clippy::needless_pass_by_value)]
// The plan is read from three inputs: the Vault's Layout, the project's configuration and lock, and
// the run's own directory and operands. Naming them in a struct would only rename the same list.
#![allow(clippy::too_many_arguments)]
// The command is one pass, and each step needs what the one before it read: the plan, the versions
// and content, the two files, then the work tree. Splitting it would pass the same values between
// helpers without making any of them reusable.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use globset::GlobBuilder;
use librorolala::layout::{EntryKind, Layout, LayoutPath};
use mingling::{
    Grouped,
    macros::{buffer, chain, command, r_eprintln, renderer},
    res::ResExitCode,
};
use rola_define_format::{Lock, LockPath, Material, MaterialKind, Vault, VaultName};
use rola_git_support::{Pin, ProjectConfig, Session};
use rorolala_utils_location::normalize;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::Refused;
use crate::exit_codes::EC_FAILED;
use crate::upstream;

/// Signs the entries of a Vault's Layout in to this project
///
/// The operands read the way `mv`'s do: every word but the last is a reference, and the last, when
/// there is more than one, is where those references are to land here. A reference is a path in the
/// Vault's Layout, a glob over those paths, or a `Uuid`.
///
/// With no destination each reference lands at the path the Vault's Layout names it by. A
/// destination that is an existing directory is a prefix: a reference that is a path keeps its whole
/// path under it, and a glob is rebased on the directory it selects from. Any other destination
/// names the whole landing path, so it takes exactly one reference.
///
/// What is signed in is written to `rola.toml` as a mapping and to `rola.lock` as a material, and
/// the content is placed in the work tree. Nothing is written unless every reference was read and
/// every landing was free.
#[command(node = "bring", entry = EntryBring)]
pub fn bring(words: Vec<String>) -> Next {
    // `decide` has read this line already, so a line it refused never gets here; a word that still
    // did not read is answered as the command's usage rather than guessed at.
    let Ok(given) = crate::options::given("bring", &words) else {
        return Refused { command: "bring" }.into();
    };

    let Some((refs, to)) = split(given.operands) else {
        return Refused { command: "bring" }.into();
    };

    StateBring {
        refs,
        to,
        vault: given.vault,
        by_name: given.by_name,
    }
    .into()
}

/// The state a bring starts in.
#[derive(Grouped)]
pub struct StateBring {
    /// What to bring in: paths in the Vault's Layout, globs over them, or `Uuid`s.
    refs: Vec<String>,
    /// Where they are to land here, or nothing to land each at the path the Vault names it by.
    to: Option<String>,
    /// The Vault to read, when the run named one.
    vault: Option<String>,
    /// Whether each reference keeps only its own name under the destination.
    by_name: bool,
}

/// Reads the references and writes what they name.
#[chain]
pub fn handle_bring(state: StateBring) -> Next {
    match signed_in(state) {
        Ok(result) => result.into(),
        Err(BringFailure::Refused(refusal)) => crate::BringRefused { refusal }.into(),
        Err(BringFailure::Fatal(message)) => crate::Fatal::new(message).into(),
    }
}

/// Result: what could not be placed after the entries were signed in.
#[derive(Grouped)]
pub struct ResultBrought {
    /// Each material that could not be placed, and why.
    pub failures: Vec<(String, String)>,
}

/// Renders what could not be placed, and ends the run on the first of them with git's 1.
#[renderer(buffer)]
pub fn render_brought(result: ResultBrought, ec: &mut ResExitCode) {
    for (path, error) in &result.failures {
        r_eprintln!(
            "{}{}",
            t!("message.error"),
            t!(
                "message.failed_path",
                path = path.as_str(),
                error = error.as_str()
            )
        );
    }

    if !result.failures.is_empty() {
        ec.exit_code = EC_FAILED;
    }
}

/// Why a bring could not act.
#[derive(Debug)]
enum BringFailure {
    /// A reference, a destination or a landing that cannot be used, answered before anything was
    /// written.
    Refused(Refusal),
    /// The project, the Vault or the content could not be reached, answered as `fatal:`.
    Fatal(String),
}

/// Why a reference or a landing cannot be used.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The reference names nothing in the named Vault's Layout.
    Unknown {
        /// The reference as it was written.
        reference: String,
        /// The Vault it was read against.
        vault: String,
    },
    /// The `Uuid` has no path in the Vault's Layout, and no destination named one.
    Unnamed {
        /// The `Uuid` as the run wrote it.
        id: Uuid,
        /// The Vault it was read against.
        vault: String,
    },
    /// A destination that is not a directory was given more than one reference.
    Destination,
    /// The destination is not inside the project.
    Outside(String),
    /// Something is already at the landing.
    Taken(String),
    /// The `Uuid` is already a mapping.
    Known(Uuid),
    /// The path is already a mapping or a material of the lock.
    PathKnown(String),
}

/// One reference read: what the Vault's Layout says about it, and where it lands here.
pub struct Planned {
    /// The `Uuid` it is known by.
    pub id: Uuid,
    /// What the Vault's Layout records it as.
    pub kind: EntryKind,
    /// The path it lands at, relative to the project's root.
    pub path: LockPath,
}

/// Reads the references against the Vault's Layout and this project, writing nothing.
///
/// # Errors
///
/// Returns the first reference that cannot be read, or the first landing that cannot be made.
fn plan(
    remote: &Layout,
    config: &ProjectConfig,
    lock: &Lock,
    root: &Path,
    cwd: &Path,
    refs: &[String],
    to: Option<&str>,
    by_name: bool,
    vault: &str,
) -> Result<Vec<Planned>, Refusal> {
    let destination = destination(root, cwd, to)?;

    // A destination that is not a directory names one landing. More than one operand is already
    // more than it can take, the way `mv` reads them; one operand naming several, a glob, is found
    // out once the references are read below.
    if matches!(destination, Destination::Exact(_)) && refs.len() > 1 {
        return Err(Refusal::Destination);
    }

    let matched = expand(remote, refs, &destination, by_name, vault)?;

    if matches!(destination, Destination::Exact(_)) && matched.len() > 1 {
        return Err(Refusal::Destination);
    }

    let mut landings = BTreeSet::new();

    for entry in &matched {
        // What is to come in is something this project does not hold.
        if config
            .mapping
            .iter()
            .any(|mapping| mapping.from == entry.id)
        {
            return Err(Refusal::Known(entry.id));
        }

        let path = entry.path.as_str();
        let mapped = config
            .mapping
            .iter()
            .any(|mapping| mapping.path().is_ok_and(|held| held.as_str() == path));
        let locked = lock
            .materials()
            .iter()
            .any(|material| material.path().as_str() == path);

        if mapped || locked {
            return Err(Refusal::PathKnown(path.to_owned()));
        }

        // Where it lands is somewhere nothing is: not where an earlier reference of this run is to
        // land, and not a path the work tree already holds.
        if !landings.insert(path.to_owned()) || landing_taken(root, &entry.path, entry.kind) {
            return Err(Refusal::Taken(path.to_owned()));
        }
    }

    Ok(matched)
}

/// Where the references are to land here, read from the operands the way `mv` reads its target.
enum Destination {
    /// No destination was written: each reference lands at the path the Vault names it by.
    Named,
    /// One reference lands at exactly this path.
    Exact(LayoutPath),
    /// Every reference lands under this directory, joined with the name the Vault gives it; `""`
    /// names the project's root, the one directory with no name of its own.
    Under(String),
}

/// Everything the references name, each entry once and in the order the references name them, with
/// where each of them lands.
///
/// A reference that names a folder of the copy brings what stands under it as well: bringing a
/// folder in is bringing in the tree it is, and a path that names only the directory, no entry
/// stands there, names what is under it rather than nothing. An entry several references name, a
/// glob and a path that reaches into what it matched, is one request rather than a clash, so it is
/// brought in once.
fn expand(
    remote: &Layout,
    refs: &[String],
    destination: &Destination,
    by_name: bool,
    vault: &str,
) -> Result<Vec<Planned>, Refusal> {
    let mut paths: Vec<LayoutPath> = remote.paths().into_iter().map(|(path, _)| path).collect();
    paths.sort_unstable();

    let mut wanted = Vec::new();
    let mut seen = BTreeSet::new();

    for reference in refs {
        let (base, seeds) = seeds(remote, &paths, reference, by_name, vault)?;

        for seed in seeds {
            let mut candidates = vec![seed.clone()];
            candidates.extend(
                paths
                    .iter()
                    .filter(|path| under(path.as_str(), seed.as_str()))
                    .cloned(),
            );

            for path in candidates {
                let Some(id) = remote.id_of(&path) else {
                    continue;
                };

                if !seen.insert(id) {
                    continue;
                }

                let Some(data) = remote.entry(id) else {
                    continue;
                };

                let Some(landing) = landing(destination, &path, &base) else {
                    return Err(Refusal::Unnamed {
                        id,
                        vault: vault.to_owned(),
                    });
                };

                wanted.push(Planned {
                    id,
                    kind: data.kind(),
                    path: landing,
                });
            }
        }
    }

    Ok(wanted)
}

/// What one reference names: the directory a glob selects from, and the paths it names.
///
/// # Errors
///
/// Returns [`Refusal::Unknown`] when the reference names nothing, a glob that matches nothing
/// included, since a run that quietly did part of what was asked is worse than one that stops.
fn seeds(
    remote: &Layout,
    paths: &[LayoutPath],
    reference: &str,
    by_name: bool,
    vault: &str,
) -> Result<(String, Vec<LayoutPath>), Refusal> {
    let unknown = || Refusal::Unknown {
        reference: reference.to_owned(),
        vault: vault.to_owned(),
    };

    if let Ok(id) = Uuid::from_str(reference) {
        return remote.path_of(id).map_or_else(
            || Err(unknown()),
            |path| {
                let base = if by_name {
                    parent(path.as_str())
                } else {
                    String::new()
                };

                Ok((base, vec![path]))
            },
        );
    }

    if is_pattern(reference) {
        let matched = matching(reference, paths, vault)?;

        if matched.is_empty() {
            return Err(unknown());
        }

        return Ok((pattern_base(reference), matched));
    }

    let Ok(path) = LayoutPath::new(reference) else {
        return Err(unknown());
    };

    if remote.id_of(&path).is_none()
        && !paths
            .iter()
            .any(|candidate| under(candidate.as_str(), path.as_str()))
    {
        return Err(unknown());
    }

    let base = if by_name {
        parent(path.as_str())
    } else {
        String::new()
    };

    Ok((base, vec![path]))
}

/// Whether a reference is to be read as a glob over the Vault's Layout paths.
///
/// A reference with none of the pattern characters names one path exactly, so a path holding a `[`
/// or `{` is still nameable; one with them is a pattern, and a literal occurrence has to be written
/// as a class the way [`globset`] spells one.
fn is_pattern(reference: &str) -> bool {
    reference
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | '{'))
}

/// The paths among `paths` that the glob `reference` names, keeping the order they came in.
///
/// Separators are literal, the way a shell reads a pattern: `*` names what is directly under a
/// directory and `**` what is anywhere below it, rather than `*` swallowing a whole tree.
///
/// # Errors
///
/// Returns [`Refusal::Unknown`] when the reference does not read as a pattern.
fn matching(
    reference: &str,
    paths: &[LayoutPath],
    vault: &str,
) -> Result<Vec<LayoutPath>, Refusal> {
    let glob = GlobBuilder::new(reference)
        .literal_separator(true)
        .build()
        .map_err(|_| Refusal::Unknown {
            reference: reference.to_owned(),
            vault: vault.to_owned(),
        })?;
    let matcher = glob.compile_matcher();

    Ok(paths
        .iter()
        .filter(|path| matcher.is_match(path.as_str()))
        .cloned()
        .collect())
}

/// The directory a path stands in, which is what a name is read against.
fn parent(path: &str) -> String {
    path.rsplit_once('/')
        .map_or_else(String::new, |(directory, _)| directory.to_owned())
}

/// The directory a glob selects from, which what it matches is rebased on.
fn pattern_base(reference: &str) -> String {
    let cut = reference
        .find(['*', '?', '[', '{'])
        .unwrap_or(reference.len());
    let head = &reference[..cut];

    head.rfind('/')
        .map_or_else(String::new, |at| head[..at].to_owned())
}

/// Whether `path` names something below `of`, and not `of` itself.
fn under(path: &str, of: &str) -> bool {
    path.len() > of.len() && path.starts_with(of) && path.as_bytes()[of.len()] == b'/'
}

/// Splits the operands the way `mv` reads its own: everything but the last word is what is to come
/// in, and the last, when there is more than one word, is where it is to go.
///
/// `None` is a run that named nothing at all.
fn split(mut words: Vec<String>) -> Option<(Vec<String>, Option<String>)> {
    if words.is_empty() {
        return None;
    }

    if words.len() == 1 {
        return Some((words, None));
    }

    let to = words.pop();

    Some((words, to))
}

/// Where the operands say the references are to land.
///
/// A destination is read where the run was made, the way `restore` reads a name: `.` is the
/// directory the run is in and `..` climbs from there. It has to be somewhere the project holds,
/// since a lock names nothing outside its own root.
fn destination(root: &Path, cwd: &Path, to: Option<&str>) -> Result<Destination, Refusal> {
    let Some(to) = to else {
        return Ok(Destination::Named);
    };

    let absolute = normalize(&resolve_against(cwd, to));

    let Ok(relative) = absolute.strip_prefix(normalize(root)) else {
        return Err(Refusal::Outside(to.to_owned()));
    };

    if absolute.is_dir() {
        return Ok(Destination::Under(prefix(relative)));
    }

    LayoutPath::from_relative(relative)
        .map(Destination::Exact)
        .map_err(|_| Refusal::Outside(to.to_owned()))
}

/// The path one reference lands at: the path the Vault gives it, joined into the destination when
/// that is a directory, or the destination itself when that is the whole name.
///
/// What a path names under the Vault's unplaced mark, `@/new/a.fbx`, lands without that mark: the
/// mark is the Vault's bookkeeping, not a directory of the work.
fn landing(destination: &Destination, named: &LayoutPath, base: &str) -> Option<LockPath> {
    match destination {
        Destination::Named => {
            placeable(named.as_str()).and_then(|placed| LockPath::new(placed).ok())
        }
        Destination::Exact(path) => LockPath::new(path.as_str()).ok(),
        Destination::Under(directory) => {
            let rest = if base.is_empty() {
                named.as_str()
            } else {
                named
                    .as_str()
                    .strip_prefix(base)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .unwrap_or(named.as_str())
            };
            let rest = placeable(rest)?;
            let joined = if directory.is_empty() {
                rest.to_owned()
            } else {
                format!("{directory}/{rest}")
            };

            LockPath::new(&joined).ok()
        }
    }
}

/// A path as it is to be written here, with the Vault's unplaced mark taken off the front.
///
/// `None` is a path that is nothing but the mark: there is no name in it to place.
fn placeable(path: &str) -> Option<&str> {
    match path.strip_prefix("@/") {
        Some("") => None,
        Some(rest) => Some(rest),
        None => Some(path),
    }
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve_against(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// The prefix the paths under `relative` share, or `""` for the project's root itself.
fn prefix(relative: &Path) -> String {
    LayoutPath::from_relative(relative)
        .map_or_else(|_| String::new(), |path| path.as_str().to_owned())
}

/// Whether where an entry lands is already taken.
///
/// A tree may land on a directory that is there and empty: a folder made for it, with nothing in
/// it, is the same as nothing being there for a thing that is a directory. A file is only ever a
/// landing where nothing is.
fn landing_taken(root: &Path, path: &LockPath, kind: EntryKind) -> bool {
    let disk = root.join(disk_path(path));

    if !disk.exists() {
        return false;
    }

    if kind != EntryKind::Tree || !disk.is_dir() {
        return true;
    }

    std::fs::read_dir(&disk).map_or(true, |mut entries| entries.next().is_some())
}

/// A lock's path as a path on disk, `/`-separated as the lock writes it.
fn disk_path(path: &LockPath) -> PathBuf {
    path.as_str().split('/').collect()
}

/// Reads the references and writes what they name, or says why it cannot.
fn signed_in(state: StateBring) -> Result<ResultBrought, BringFailure> {
    let StateBring {
        refs,
        to,
        vault,
        by_name,
    } = state;

    let cwd = std::env::current_dir().map_err(|error| {
        BringFailure::Fatal(t!("git_rola.err_cwd", error = error.to_string()).to_string())
    })?;

    let Some(session) =
        Session::open(&cwd).map_err(|error| BringFailure::Fatal(error.to_string()))?
    else {
        return Err(BringFailure::Fatal(
            t!("git_rola.err_no_project").to_string(),
        ));
    };

    let project = session.project();
    let config = project
        .config()
        .map_err(|error| BringFailure::Fatal(error.to_string()))?;

    // The lock is read before anything is reached for: a project with none is told so without a
    // Vault being asked, and a run that cannot append to it does not start.
    let stored = session.lock().map_err(|error| {
        BringFailure::Fatal(t!("git_rola.err_lock", error = error.to_string()).to_string())
    })?;

    let name = vault_name(&config, vault.as_deref())?;
    let source = upstream::VaultSource::named(&config, &name).map_err(BringFailure::Fatal)?;
    let account = upstream::account(project).map_err(BringFailure::Fatal)?;

    // The Layout is fetched once and read twice: once to decide the landings, and once below for the
    // version each landing is at.
    let fetch = upstream::Fetch::new(project).map_err(BringFailure::Fatal)?;
    let workspace = upstream::Workarea::new(project).map_err(BringFailure::Fatal)?;
    let remote =
        upstream::fetch_layout(&fetch.workspace, &account, &source).map_err(BringFailure::Fatal)?;

    let planned = plan(
        &remote,
        &config,
        &stored,
        project.root(),
        &cwd,
        &refs,
        to.as_deref(),
        by_name,
        &name,
    )
    .map_err(BringFailure::Refused)?;

    // What each landing holds, and the content it holds it in. Everything is resolved before
    // either project file is written, so a Vault that cannot answer leaves both as they were.
    let mut materials = Vec::with_capacity(planned.len());
    let mut contents = Vec::with_capacity(planned.len());
    let mut trees = Vec::new();

    for entry in &planned {
        let held = remote.entry(entry.id).ok_or_else(|| {
            BringFailure::Fatal(
                t!(
                    "update.err_not_in_layout",
                    material = entry.id.to_string(),
                    vault = name.as_str()
                )
                .to_string(),
            )
        })?;

        let version =
            upstream::version_object(&workspace.value, &account, &source, &held.version())
                .map_err(BringFailure::Fatal)?;
        let content = upstream::content_of(&workspace.value, &account, &source, &version)
            .map_err(BringFailure::Fatal)?;

        contents.push(content);

        if entry.kind == EntryKind::Tree {
            trees.push((entry.id.to_string(), content));
        }

        materials.push(Material::new(
            entry.path.clone(),
            content,
            kind_of(entry.kind),
        ));
    }

    let store = upstream::project_stores(project).map_err(BringFailure::Fatal)?;
    upstream::bring_contents(
        &store,
        &workspace.value,
        &account,
        std::slice::from_ref(&source),
        &contents,
    )
    .map_err(BringFailure::Fatal)?;

    if !trees.is_empty() {
        let inside = upstream::tree_contents(&store, &trees).map_err(BringFailure::Fatal)?;

        upstream::bring_contents(
            &store,
            &workspace.value,
            &account,
            std::slice::from_ref(&source),
            &inside,
        )
        .map_err(BringFailure::Fatal)?;
    }

    // The mapping first: a run that cannot write the lock then leaves a `rola.toml` naming what it
    // asked for, which is the state a hand edit leaves and the next `update` applies.
    let pin = pin_for(&config, &name);
    let mut block = String::new();

    for entry in &planned {
        block.push_str(&mapping_text(entry.id, &entry.path, &pin));
    }

    let config_path = project.config_path();
    let original = std::fs::read_to_string(&config_path).map_err(|error| {
        BringFailure::Fatal(
            t!(
                "bring.err_config",
                path = config_path.display().to_string(),
                error = error.to_string()
            )
            .to_string(),
        )
    })?;

    upstream::write_atomically(&config_path, &appended(&original, &block))
        .map_err(BringFailure::Fatal)?;

    // Then the lock, with this run's materials appended and the Vault they came from.
    let vault = Vault::new(
        VaultName::new(&name).map_err(|error| BringFailure::Fatal(error.to_string()))?,
        &source.address,
    )
    .map_err(|error| BringFailure::Fatal(error.to_string()))?;
    let merged = merged(&stored, &vault, materials).map_err(BringFailure::Fatal)?;

    upstream::write_atomically(&project.lock_path(), &merged.encode())
        .map_err(BringFailure::Fatal)?;

    // And last the work tree: only what this run brought is placed, so a material that was already
    // there is not taken out and put back for this.
    let placed = crate::source::of(project).map_err(BringFailure::Fatal)?;
    let paths: Vec<LockPath> = planned.iter().map(|entry| entry.path.clone()).collect();
    let failures = session
        .restore(&merged, &paths, &placed)
        .map_err(|error| BringFailure::Fatal(error.to_string()))?;
    session
        .exclude(&merged)
        .map_err(|error| BringFailure::Fatal(error.to_string()))?;

    Ok(ResultBrought {
        failures: failures
            .into_iter()
            .map(|(path, error)| (path, error.to_string()))
            .collect(),
    })
}

/// The Vault a bring reads, when the run named none.
///
/// `origin` is the name the project template writes, so it is preferred over whatever a map happens
/// to hold first; a project that names neither still gets the default its mappings use.
fn vault_name(config: &ProjectConfig, asked: Option<&str>) -> Result<String, BringFailure> {
    upstream::vault_name(config, asked)
        .ok_or_else(|| BringFailure::Fatal(t!("update.err_no_vault").to_string()))
}

/// The pin a mapping written by this run carries.
///
/// A Vault that is the project's default is what a `latest` pin would resolve through, so it is
/// written as `latest`; a Vault named by the run is written by name, so a later `update` goes back
/// to the same one.
fn pin_for(config: &ProjectConfig, name: &str) -> Pin {
    if config.default_vault() == Some(name) {
        Pin::Latest
    } else {
        Pin::Vault(name.to_owned())
    }
}

/// The mapping this run appends to `rola.toml`.
fn mapping_text(id: Uuid, path: &LockPath, pin: &Pin) -> String {
    format!(
        "[[mapping]]\nfrom = \"{id}\"\nto = \"/{}\"\npin = \"{pin}\"\n",
        path.as_str()
    )
}

/// `block` appended to `original`, one blank line between them.
///
/// The file is a person's, so only the end of it changes: what is there, comments and order
/// included, is kept as it is.
fn appended(original: &str, block: &str) -> String {
    let mut text = original.trim_end_matches(['\r', '\n']).to_owned();

    if !text.is_empty() {
        text.push('\n');
        text.push('\n');
    }

    text.push_str(block);

    text
}

/// The lock the project carries with `materials` appended and `vault` named.
///
/// # Errors
///
/// Returns a message when two materials share a path or two Vaults share a name, which the plan has
/// already refused, so this is the lock disagreeing with itself rather than a run's mistake.
fn merged(stored: &Lock, vault: &Vault, materials: Vec<Material>) -> Result<Lock, String> {
    let mut vaults = stored.vaults().to_vec();

    if !vaults.iter().any(|held| held.name() == vault.name()) {
        vaults.push(vault.clone());
    }

    let mut all: Vec<Material> = stored.materials().to_vec();
    all.extend(materials);

    Lock::new(vaults, all).map_err(|error| error.to_string())
}

/// What the Vault's kind is placed as.
fn kind_of(kind: EntryKind) -> MaterialKind {
    match kind {
        EntryKind::File => MaterialKind::File,
        EntryKind::Tree => MaterialKind::Tree,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use librorolala::layout::{EntryKind, Layout, LayoutPath, MutableData};
    use rola_define_format::{ContentHash, Lock, LockPath, Material, MaterialKind};
    use rola_git_support::{Defaults, Features, Mapping, Pin, ProjectConfig};
    use uuid::Uuid;

    use super::{Refusal, appended, mapping_text, merged, plan, split, vault_name};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-bring-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A `Uuid` from a number, so a test can name what it expects.
    fn id(number: u8) -> Uuid {
        Uuid::from_u128(u128::from(number))
    }

    /// A Layout holding `entries`, each at its path.
    fn layout(dir: &Path, entries: &[(&str, Uuid, EntryKind)]) -> Layout {
        let layout = Layout::open(dir).unwrap();

        for (path, id, kind) in entries {
            layout
                .create_entry(
                    *id,
                    MutableData::new(None, [7; 32], "an entry").with_kind(*kind),
                )
                .unwrap();
            layout
                .create_path(&LayoutPath::new(path).unwrap(), *id)
                .unwrap();
        }

        layout
    }

    /// A project file mapping `mappings`.
    fn config(mappings: &[(Uuid, &str)]) -> ProjectConfig {
        ProjectConfig {
            vaults: BTreeMap::new(),
            features: Features { git_support: true },
            defaults: Defaults::default(),
            storages: Vec::new(),
            mapping: mappings
                .iter()
                .map(|(from, to)| Mapping {
                    from: *from,
                    to: (*to).to_owned(),
                    pin: Pin::Latest,
                })
                .collect(),
        }
    }

    /// A lock naming `paths` at one content each, or one holding nothing.
    fn lock(paths: &[&str]) -> Lock {
        let materials = paths
            .iter()
            .map(|path| {
                Material::new(
                    LockPath::new(path).unwrap(),
                    ContentHash::from_digest([1; 32]),
                    MaterialKind::File,
                )
            })
            .collect();

        Lock::new(vec![], materials).unwrap()
    }

    /// The paths `refs` plan to land at.
    fn landings(
        remote: &Layout,
        config: &ProjectConfig,
        lock: &Lock,
        root: &Path,
        refs: &[&str],
        to: Option<&str>,
        by_name: bool,
    ) -> Result<Vec<String>, Refusal> {
        let refs: Vec<String> = refs
            .iter()
            .map(|reference| (*reference).to_owned())
            .collect();

        plan(
            remote, config, lock, root, root, &refs, to, by_name, "origin",
        )
        .map(|planned| {
            planned
                .iter()
                .map(|entry| entry.path.as_str().to_owned())
                .collect()
        })
    }

    #[test]
    fn a_name_lands_where_the_vault_has_it() {
        let root = scratch("named");
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);

        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Assets/hero.psd"],
                None,
                false
            ),
            Ok(vec!["Assets/hero.psd".to_owned()])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_destination_is_a_prefix() {
        let root = scratch("prefix");
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);

        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Assets/hero.psd"],
                Some("lib"),
                false
            ),
            Ok(vec!["lib/Assets/hero.psd".to_owned()])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn by_name_takes_the_parent_for_the_base() {
        let root = scratch("by-name");
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);

        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Assets/hero.psd"],
                Some("lib"),
                true
            ),
            Ok(vec!["lib/hero.psd".to_owned()])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_star_names_one_level_and_two_stars_go_further() {
        let root = scratch("glob");
        let remote = layout(
            &root,
            &[
                ("Assets/a.psd", id(1), EntryKind::File),
                ("Assets/b.psd", id(2), EntryKind::File),
                ("Assets/sub/c.psd", id(3), EntryKind::File),
            ],
        );
        let empty = lock(&[]);

        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Assets/*"],
                None,
                false
            ),
            Ok(vec!["Assets/a.psd".to_owned(), "Assets/b.psd".to_owned()])
        );
        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Assets/**"],
                None,
                false
            ),
            Ok(vec![
                "Assets/a.psd".to_owned(),
                "Assets/b.psd".to_owned(),
                "Assets/sub/c.psd".to_owned(),
            ])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_reference_brings_what_is_under_it() {
        let root = scratch("folder");
        let remote = layout(
            &root,
            &[
                ("Assets/a.psd", id(1), EntryKind::File),
                ("Assets/sub/c.psd", id(2), EntryKind::File),
            ],
        );
        let empty = lock(&[]);

        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Assets"],
                None,
                false
            ),
            Ok(vec![
                "Assets/a.psd".to_owned(),
                "Assets/sub/c.psd".to_owned()
            ])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_uuid_names_what_the_vault_holds() {
        let root = scratch("uuid");
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);
        let reference = id(1).to_string();

        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &[&reference],
                None,
                false
            ),
            Ok(vec!["Assets/hero.psd".to_owned()])
        );

        // A `Uuid` the Vault's Layout names nothing by cannot land anywhere on its own.
        assert!(matches!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &[&id(2).to_string()],
                None,
                false
            ),
            Err(Refusal::Unknown { .. })
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_uuid_with_no_path_names_nothing() {
        let root = scratch("unnamed");
        let remote = Layout::open(&root).unwrap();
        remote
            .create_entry(id(1), MutableData::new(None, [7; 32], "an entry"))
            .unwrap();
        let empty = lock(&[]);
        let reference = id(1).to_string();

        // The entry is in the Layout, but no path names it, so there is nothing to land.
        assert!(matches!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &[&reference],
                None,
                false
            ),
            Err(Refusal::Unknown { .. })
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_vaults_unplaced_mark_is_not_part_of_the_landing() {
        assert_eq!(super::placeable("@/new/a.fbx"), Some("new/a.fbx"));
        assert_eq!(super::placeable("a.fbx"), Some("a.fbx"));
        assert_eq!(super::placeable("@/"), None);
    }

    #[test]
    fn a_uuid_already_mapped_is_refused() {
        let root = scratch("known");
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);
        let mine = config(&[(id(1), "Assets/elsewhere.psd")]);

        assert!(matches!(
            landings(&remote, &mine, &empty, &root, &["Assets/hero.psd"], None, false),
            Err(Refusal::Known(seen)) if seen == id(1)
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_place_already_taken_by_a_mapping_or_a_material_is_refused() {
        let root = scratch("path-known");
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);
        let mine = config(&[(id(9), "Assets/hero.psd")]);

        assert!(matches!(
            landings(&remote, &mine, &empty, &root, &["Assets/hero.psd"], None, false),
            Err(Refusal::PathKnown(path)) if path == "Assets/hero.psd"
        ));

        let held = lock(&["Assets/hero.psd"]);
        assert!(matches!(
            landings(&remote, &config(&[]), &held, &root, &["Assets/hero.psd"], None, false),
            Err(Refusal::PathKnown(path)) if path == "Assets/hero.psd"
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_place_the_work_tree_holds_is_refused() {
        let root = scratch("taken");
        std::fs::create_dir_all(root.join("Assets")).unwrap();
        std::fs::write(root.join("Assets/hero.psd"), b"something").unwrap();
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);

        assert!(matches!(
            landings(&remote, &config(&[]), &empty, &root, &["Assets/hero.psd"], None, false),
            Err(Refusal::Taken(path)) if path == "Assets/hero.psd"
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_empty_directory_takes_a_tree() {
        let root = scratch("tree");
        std::fs::create_dir_all(root.join("Textures")).unwrap();
        let remote = layout(&root, &[("Textures", id(1), EntryKind::Tree)]);
        let empty = lock(&[]);

        assert_eq!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Textures"],
                None,
                false
            ),
            Ok(vec!["Textures".to_owned()])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn two_references_cannot_land_on_one_place() {
        let root = scratch("clash");
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let remote = layout(
            &root,
            &[
                ("A/x.psd", id(1), EntryKind::File),
                ("B/x.psd", id(2), EntryKind::File),
            ],
        );
        let empty = lock(&[]);

        assert!(matches!(
            landings(&remote, &config(&[]), &empty, &root, &["A/x.psd", "B/x.psd"], Some("lib"), true),
            Err(Refusal::Taken(path)) if path == "lib/x.psd"
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_exact_destination_takes_one_reference() {
        let root = scratch("exact");
        let remote = layout(
            &root,
            &[
                ("A/x.psd", id(1), EntryKind::File),
                ("B/x.psd", id(2), EntryKind::File),
            ],
        );
        let empty = lock(&[]);

        assert!(matches!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["A/x.psd", "B/x.psd"],
                Some("one.psd"),
                false
            ),
            Err(Refusal::Destination)
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_destination_outside_the_project_is_refused() {
        let root = scratch("outside");
        let remote = layout(&root, &[("Assets/hero.psd", id(1), EntryKind::File)]);
        let empty = lock(&[]);

        assert!(matches!(
            landings(
                &remote,
                &config(&[]),
                &empty,
                &root,
                &["Assets/hero.psd"],
                Some("../elsewhere"),
                false
            ),
            Err(Refusal::Outside(_))
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_operand_line_is_split_the_way_mv_reads_it() {
        assert!(split(Vec::new()).is_none());
        assert_eq!(
            split(vec!["a".to_owned()]),
            Some((vec!["a".to_owned()], None))
        );
        assert_eq!(
            split(vec!["a".to_owned(), "b".to_owned()]),
            Some((vec!["a".to_owned()], Some("b".to_owned())))
        );
    }

    #[test]
    fn the_mapping_written_is_the_one_the_file_takes() {
        let text = mapping_text(
            id(1),
            &LockPath::new("Assets/hero.psd").unwrap(),
            &Pin::Latest,
        );

        assert_eq!(
            text,
            format!(
                "[[mapping]]\nfrom = \"{}\"\nto = \"/Assets/hero.psd\"\npin = \"latest\"\n",
                id(1)
            )
        );

        let named = mapping_text(
            id(1),
            &LockPath::new("Assets/hero.psd").unwrap(),
            &Pin::Vault("mirror".to_owned()),
        );
        assert!(named.contains("pin = \"mirror\""));
    }

    #[test]
    fn a_block_is_appended_with_one_blank_line_before_it() {
        let block = "[[mapping]]\nfrom = \"x\"\n";

        assert_eq!(appended("", block), block);
        assert_eq!(appended("\n", block), block);
        assert_eq!(
            appended("[features]\ngit_support = true", block),
            format!("[features]\ngit_support = true\n\n{block}")
        );
        assert_eq!(
            appended("[features]\ngit_support = true\n\n\n", block),
            format!("[features]\ngit_support = true\n\n{block}")
        );
    }

    #[test]
    fn a_lock_takes_the_new_material_and_names_its_vault_once() {
        use rola_define_format::{Vault, VaultName};

        let held = lock(&["Assets/old.psd"]);
        let vault = Vault::new(VaultName::new("origin").unwrap(), "rola://host/vault").unwrap();
        let material = Material::new(
            LockPath::new("Assets/hero.psd").unwrap(),
            ContentHash::from_digest([2; 32]),
            MaterialKind::File,
        );

        let once = merged(&held, &vault, vec![material]).unwrap();
        assert_eq!(once.vaults().len(), 1);
        assert_eq!(once.materials().len(), 2);

        // Asking again with the same Vault adds no second name.
        let twice = merged(&once, &vault, Vec::new()).unwrap();
        assert_eq!(twice.vaults().len(), 1);
    }

    #[test]
    fn the_vault_named_by_the_run_wins_over_the_project_default() {
        let mut mine = config(&[]);
        mine.vaults
            .insert("origin".to_owned(), "rola://a/one".to_owned());
        mine.vaults
            .insert("mirror".to_owned(), "rola://b/two".to_owned());
        mine.defaults.vault = Some("mirror".to_owned());

        assert_eq!(vault_name(&mine, Some("origin")).unwrap(), "origin");
        assert_eq!(vault_name(&mine, None).unwrap(), "mirror");
        assert_eq!(super::pin_for(&mine, "mirror"), Pin::Latest);
        assert_eq!(
            super::pin_for(&mine, "origin"),
            Pin::Vault("origin".to_owned())
        );
    }
}
