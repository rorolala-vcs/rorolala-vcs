//! Build script: collect the git commit hash, the rustc version, the commit
//! date, and `workspace.package.version` from Cargo.toml, then write them out
//! to `.cache/rs-target/ref.json`.
//!
//! It also generates the C header for the workspace's `#[lazyffi]` surface into
//! `{target_dir}/{profile}/ffi_bindings/` through `rorolala-dev-bindgen`.

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::process::Command;

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
