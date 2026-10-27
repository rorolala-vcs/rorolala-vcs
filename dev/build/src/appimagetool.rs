//! The tool the `AppImage` is made with.
//!
//! `AppImageKit` publishes the tool as an `AppImage` under a release named `continuous`, so there is no
//! version tag to pin. The hash is what pins it instead: the release may be rebuilt, and a build
//! that found different bytes would stop rather than package something nobody reviewed. A machine
//! that keeps its own `appimagetool` on the path is used as it is; nothing is fetched here.

use std::path::PathBuf;

use crate::Error;
use crate::paths::Workspace;
use crate::toolchain::Tool;

/// The hash the tool must have.
const SHA256: &str = "95cbe7cce9717fce90c484e34052ee7c7f1d7635b33c12525b4776826a7d29b6";

/// Where the tool is fetched from.
const URL: &str = "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage";

/// The directory under the project's own cache the toolchain lives in.
const DIRECTORY: &str = "appimagetool";

/// The program itself.
const BINARY: &str = "appimagetool-x86_64.AppImage";

/// The `AppImage` tool: the one under the project's own cache, or the one this machine has on its
/// path.
///
/// # Errors
///
/// Reports [`Error::Missing`] with the source, the hash and the commands that lay it down when
/// neither is there.
pub fn require(workspace: &Workspace) -> Result<PathBuf, Error> {
    let laid_out = workspace.toolchain_dir()?.join(DIRECTORY).join(BINARY);
    if laid_out.is_file() {
        return Ok(laid_out);
    }

    if let Some(found) = Tool::on_path(BINARY).or_else(|| Tool::on_path("appimagetool")) {
        eprintln!(
            "appimagetool is not laid out under {}; using the one on this machine's path: {}",
            laid_out.display(),
            found.display()
        );
        return Ok(found);
    }

    let directory = workspace.toolchain_dir()?.join(DIRECTORY);

    Err(Error::Missing {
        summary: format!(
            "appimagetool is neither laid out in {} nor on this machine's path, and the AppImage is made with it",
            laid_out.display()
        ),
        advice: vec![
            format!("It is fetched from:  {URL}"),
            format!("It hashes to:        {SHA256}  (sha256)"),
            String::from("Lay it out where this project looks for it first, under its own cache:"),
            format!("  mkdir -p \"{}\"", directory.display()),
            format!("  curl -L -o \"{}\" {URL}", laid_out.display()),
            format!(
                "  echo \"{SHA256}  {}\" | sha256sum -c -",
                laid_out.display()
            ),
            format!("  chmod +x \"{}\"", laid_out.display()),
            String::from(
                "It is an `AppImage` itself, so a run needs FUSE or the extract-and-run fallback the build sets for it.",
            ),
        ],
    })
}
