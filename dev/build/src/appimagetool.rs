//! The tool the `AppImage` is made with.
//!
//! `AppImageKit` publishes the tool as an `AppImage` under both a version tag and a release named
//! `continuous` that is rebuilt whenever the tool changes, so only the versioned one has bytes that
//! hold still. The release and the hash pin it together: what a package is built with decides what
//! is in it, and a build that found different bytes would stop rather than package something nobody
//! reviewed. A machine that keeps its own `appimagetool` on the path is used as it is; nothing is
//! fetched here.

use std::path::PathBuf;

use crate::Error;
use crate::paths::Workspace;
use crate::toolchain::Tool;

/// The appimagetool release this project packages with.
const VERSION: &str = "1.9.1";

/// The hash the release's `AppImage` must have.
const SHA256: &str = "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0";

/// Where the release's `AppImage` is fetched from.
const URL: &str =
    "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage";

/// The directory under the project's own cache the toolchain lives in.
const DIRECTORY: &str = "appimagetool";

/// The program itself.
const BINARY: &str = "appimagetool-x86_64.AppImage";

/// The `AppImage` tool: the one under the project's own cache, or the one this machine has on its
/// path.
///
/// # Errors
///
/// Reports [`Error::Missing`] with the version, the source, the hash and the commands that lay it
/// down when neither is there.
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
            "appimagetool {VERSION} is neither laid out in {} nor on this machine's path, and the AppImage is made with it",
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
