//! Build script: collect the git commit hash, the rustc version, the commit
//! date, and `workspace.package.version` from Cargo.toml, then write them out
//! to `.cache/rs-target/ref.json`.
//!
//! It also generates the C header for the workspace's `#[lazyffi]` surface into
//! `{target_dir}/{profile}/ffi_bindings/` through `rorolala-dev-bindgen`.
//!
//! It also makes the icons the descriptions under `res/icons` ask for, beside them: a description is
//! what turns one picture into the format a program that draws it needs, and the format is not the
//! one an editor keeps.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::ImageEncoder;
use image::codecs::ico::IcoEncoder;
use image::imageops::FilterType;

/// Path to the manifest file, relative to the manifest directory.
const MANIFEST_PATH: &str = "Cargo.toml";

/// Directory, relative to the manifest directory, holding the generated files.
const OUTPUT_DIR: &str = ".cache/rs-target";

/// Name of the generated JSON file inside [`OUTPUT_DIR`].
const OUTPUT_FILE: &str = "ref.json";

/// Git ref files to watch so the build script re-runs on new commits.
const GIT_RERUN_PATHS: &[&str] = &[".git/HEAD", ".git/refs"];

/// Directory, inside `{target_dir}/{profile}`, collecting the FFI bindings.
const BINDINGS_DIR: &str = "ffi_bindings";

/// Source trees scanned for `#[lazyffi]` items, relative to the manifest directory.
const BINDING_SOURCE_ROOTS: &[&str] = &["src", "modules", "utils"];

/// Directory, relative to the manifest directory, of the icons and the descriptions that make them.
const ICON_DIR: &str = "res/icons";

/// The target format an icon description may ask for.
///
/// Only the one is understood. Another is an error rather than something skipped: a description
/// nothing acts on is a description whose author believes it did.
const ICON_FORMAT: &str = "ico";

/// The scaling that takes the nearest source pixel.
const SCALING_NEAREST: &str = "nearest";

/// The scaling that reads the whole of the source into every target pixel.
const SCALING_LANCZOS3: &str = "lanczos3";

/// The largest side an ICO frame may have, which is what the format's directory can state.
const ICO_MAX_SIDE: u32 = 256;

fn main() {
    // Re-run the build script whenever one of the input files changes.
    println!("cargo:rerun-if-changed={MANIFEST_PATH}");
    println!("cargo:rerun-if-changed=build.rs");
    for path in GIT_RERUN_PATHS {
        println!("cargo:rerun-if-changed={path}");
    }

    let commit_hash = git_output(&["rev-parse", "HEAD"]).unwrap_or_default();
    let commit_date = git_output(&["show", "-s", "--format=%cI", "HEAD"]).unwrap_or_default();
    let rustc_version = rustc_output(&["--version"]).unwrap_or_default();
    let version = workspace_version();

    let mut reference: BTreeMap<&str, String> = BTreeMap::new();
    reference.insert("commit_hash", commit_hash);
    reference.insert("commit_date", commit_date);
    reference.insert("rustc_version", rustc_version);
    reference.insert("version", version);

    let json = serde_json::to_string_pretty(&reference)
        .expect("failed to serialize reference info to JSON");

    let out_dir = manifest_dir().join(OUTPUT_DIR);
    std::fs::create_dir_all(&out_dir).expect("failed to create output directory");

    let out_file = out_dir.join(OUTPUT_FILE);
    std::fs::write(&out_file, json).expect("failed to write output JSON file");

    println!("cargo:rerun-if-changed={}", out_file.display());

    generate_c_header();
    generate_icons();
}

/// Makes the icons the descriptions under `res/icons` ask for, beside them.
///
/// A description sits beside the image it describes, named `<image>.<extension>.toml`, and says what
/// to make of it: the format, the size, and how the source is sampled down to that size. What it
/// makes is written beside both and is deliberately not kept in the repository — see
/// `res/icons/.gitignore` — so this is where a fresh tree gets it.
///
/// A description that cannot be acted on fails the build rather than being skipped. What a generated
/// file is for is a description that was carried out, so one that was not is one that would go
/// unnoticed until something looked for the file and did not find it.
///
/// Nothing here runs when only the .NET side is built: `dotnet build` reads the generated files, and
/// reaching them means the workspace has been built first.
fn generate_icons() {
    // Watched as a directory rather than file by file, so that a description being added or removed
    // is noticed without this list having to be kept in step by hand.
    println!("cargo:rerun-if-changed={ICON_DIR}");

    let dir = manifest_dir().join(ICON_DIR);

    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) => {
            println!("cargo:warning=could not read {ICON_DIR}: {error}; no icon is generated");
            return;
        }
    };

    let mut descriptions = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let path = entry.path();

        if path
            .extension()
            .is_some_and(|extension| extension == "toml")
        {
            descriptions.push(path);
        }
    }

    // Sorted so that the same tree makes the same files in the same order, whatever the filesystem
    // hands back: a build script that acts in the order it happens to read is not reproducible.
    descriptions.sort();

    let mut failed = false;

    for description in descriptions {
        if let Err(error) = generate_icon(&description) {
            println!("cargo:warning={error}");
            failed = true;
        }
    }

    if failed {
        std::process::exit(1);
    }
}

/// Makes the one icon a description asks for.
///
/// The error is the line to print rather than a type: what a caller does with it is say it, and one
/// line is the whole of what a build script can say.
///
/// # Errors
///
/// Fails when the description cannot be read or does not state what it has to, when the source
/// picture cannot be read, or when the icon cannot be written.
fn generate_icon(description: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(description)
        .map_err(|error| format!("{}: {error}", description.display()))?;

    let stated: toml::Value = text
        .parse()
        .map_err(|error| format!("{}: {error}", description.display()))?;

    let format = text_setting(&stated, description, "target-format")?;

    if format != ICON_FORMAT {
        return Err(format!(
            "{}: target-format is `{format}`, and `{ICON_FORMAT}` is the only one understood",
            description.display()
        ));
    }

    let filter = match text_setting(&stated, description, "scaling")?.as_str() {
        SCALING_NEAREST => FilterType::Nearest,
        SCALING_LANCZOS3 => FilterType::Lanczos3,
        other => {
            return Err(format!(
                "{}: scaling is `{other}`, and `{SCALING_NEAREST}` and `{SCALING_LANCZOS3}` are the ones understood",
                description.display()
            ));
        }
    };

    let requested = size_setting(&stated, description)?;

    // A description is named after what it describes, so the source is its own name without the
    // `.toml`, and what is made of the source is that name under the target format.
    let source = description.with_extension("");
    let output = source.with_extension(ICON_FORMAT);

    let picture = image::open(&source)
        .map_err(|error| format!("{}: {error}", source.display()))?
        .into_rgba8();

    // Worked out from the picture rather than from the description alone, because a size that leaves
    // a side out asks for the shape this picture has, which is not something the description can say.
    let (width, height) = requested.resolve((picture.width(), picture.height()), description)?;

    let resized = image::imageops::resize(&picture, width, height, filter);

    let mut encoded = Vec::new();
    IcoEncoder::new(&mut encoded)
        .write_image(
            resized.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("{}: {error}", output.display()))?;

    // Written only when it differs, because this directory is watched: writing the same bytes again
    // would be a build script that changes its own input and so runs again on every build after it.
    if std::fs::read(&output).is_ok_and(|written| written == encoded) {
        return Ok(());
    }

    std::fs::write(&output, &encoded).map_err(|error| format!("{}: {error}", output.display()))?;

    Ok(())
}

/// Reads one setting a description states as text.
///
/// # Errors
///
/// Fails when the setting is not stated, or is not stated as text.
fn text_setting(stated: &toml::Value, description: &Path, key: &str) -> Result<String, String> {
    stated
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{}: {key} is not stated as text", description.display()))
}

/// The size a description asks for.
///
/// A side that is left out is what keeps the picture's shape: a description saying `256x` states a
/// width and leaves the height to the picture, which is what a caller who knows one dimension of the
/// place the picture goes wants to say. That is also why a size is not two numbers here — the one it
/// leaves out is only known once the picture it describes has been read.
enum TargetSize {
    /// Both sides are stated.
    Both(u32, u32),

    /// The width is stated and the height follows the picture.
    Width(u32),

    /// The height is stated and the width follows the picture.
    Height(u32),
}

impl TargetSize {
    /// Works out the two sides, the one that was left out following the source's shape.
    ///
    /// # Errors
    ///
    /// Fails when the sides do not fit an ICO frame — either because the description asked for a size
    /// it cannot have, or because following this picture's shape worked out to one.
    fn resolve(self, source: (u32, u32), description: &Path) -> Result<(u32, u32), String> {
        // Worked out in 64 bits, so that following a shape cannot overflow before it is known to be
        // too large for an icon to be drawn at anyway.
        let (width, height) = match self {
            Self::Both(width, height) => (u64::from(width), u64::from(height)),
            Self::Width(width) => (u64::from(width), following(width, source.0, source.1)),
            Self::Height(height) => (following(height, source.1, source.0), u64::from(height)),
        };

        let limit = u64::from(ICO_MAX_SIDE);

        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(format!(
                "{}: target-size works out to `{width}x{height}`, and an ICO frame is between 1 and {ICO_MAX_SIDE} on a side",
                description.display()
            ));
        }

        Ok((
            u32::try_from(width).expect("a side within the limit is a u32"),
            u32::try_from(height).expect("a side within the limit is a u32"),
        ))
    }
}

/// Works out the side that was left out: `side` scaled by the shape `known` and `other` describe.
///
/// Rounded to the nearest whole pixel rather than truncated, and never down to nothing: a side of
/// zero is a picture with no picture in it, and one pixel is the least a side can be.
fn following(side: u32, known: u32, other: u32) -> u64 {
    if known == 0 {
        return u64::from(side);
    }

    let scaled = (u64::from(side) * u64::from(other) + u64::from(known) / 2) / u64::from(known);

    scaled.max(1)
}

/// Reads the size a description asks for.
///
/// # Errors
///
/// Fails when the size is not stated as text, is not written `WxH`, `Wx` or `xH`, states neither
/// side, or states a side that is not a number.
fn size_setting(stated: &toml::Value, description: &Path) -> Result<TargetSize, String> {
    let size = text_setting(stated, description, "target-size")?;

    let Some((width, height)) = size.split_once('x') else {
        return Err(format!(
            "{}: target-size is `{size}`, and a size is written `WxH`, `Wx` or `xH`",
            description.display()
        ));
    };

    let side = |value: &str, name: &str| -> Result<Option<u32>, String> {
        if value.is_empty() {
            return Ok(None);
        }

        value.parse::<u32>().map(Some).map_err(|error| {
            format!(
                "{}: target-size is `{size}`, whose {name} `{value}` is not a number: {error}",
                description.display()
            )
        })
    };

    match (side(width, "width")?, side(height, "height")?) {
        (Some(width), Some(height)) => Ok(TargetSize::Both(width, height)),
        (Some(width), None) => Ok(TargetSize::Width(width)),
        (None, Some(height)) => Ok(TargetSize::Height(height)),
        (None, None) => Err(format!(
            "{}: target-size is `{size}`, which states neither side",
            description.display()
        )),
    }
}

/// Generates the C header for the workspace's `#[lazyffi]` surface.
///
/// Failures stop the build: a build script can only speak to cargo through its
/// directives, so the diagnostic is forwarded line by line and the script then
/// exits non-zero. An incomplete header must never be produced silently.
fn generate_c_header() {
    for root in BINDING_SOURCE_ROOTS {
        println!("cargo:rerun-if-changed={root}");
    }

    let Some(output_dir) = bindings_dir() else {
        println!("cargo:warning=could not resolve the bindings directory; skipping");
        return;
    };

    let manifest = manifest_dir();
    let source_roots = BINDING_SOURCE_ROOTS
        .iter()
        .map(|root| manifest.join(root))
        .collect::<Vec<_>>();

    let config = rorolala_dev_bindgen::Config {
        source_roots: &source_roots,
        output_dir: &output_dir,
    };

    if let Err(error) = rorolala_dev_bindgen::generate(&config) {
        for line in error.to_string().lines() {
            println!("cargo:warning={line}");
        }
        std::process::exit(1);
    }
}

/// Resolves `{target_dir}/{profile}/ffi_bindings` from `OUT_DIR`.
///
/// `OUT_DIR` is `{target_dir}/{profile}/build/{package}-{hash}/out`, so three
/// levels up is `{target_dir}/{profile}`.
fn bindings_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").ok()?);
    let profile_dir = out_dir.ancestors().nth(3)?;
    Some(profile_dir.join(BINDINGS_DIR))
}

/// Returns the manifest directory, falling back to the current directory.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()))
}

/// Runs a git command and returns its trimmed stdout.
fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

/// Runs a rustc command (honouring the `RUSTC` environment variable) and
/// returns its stdout.
fn rustc_output(args: &[&str]) -> Option<String> {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = Command::new(rustc).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Parses `version` from the `[workspace.package]` section of Cargo.toml.
fn workspace_version() -> String {
    let manifest = manifest_dir().join(MANIFEST_PATH);

    let content = match std::fs::read_to_string(&manifest) {
        Ok(content) => content,
        Err(_) => return String::new(),
    };

    let mut in_workspace_package = false;
    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
            continue;
        }

        if in_workspace_package
            && let Some(rest) = trimmed.strip_prefix("version")
            && let Some(value) = rest.trim_start().strip_prefix('=')
        {
            return value.trim().trim_matches(['"', '\'']).to_string();
        }
    }

    String::new()
}
