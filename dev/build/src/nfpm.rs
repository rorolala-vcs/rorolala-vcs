//! The packager the Linux packages are built with.
//!
//! It is pinned the way the Windows installer's compiler is pinned, and for the same reason: what a
//! package is built with decides what is in it. One program makes the three formats a Linux
//! distribution asks for — deb, rpm and Arch's package — so a machine that builds one can build the
//! others without the other distributions' tools.
//!
//! Nothing is fetched and nothing is installed, as with the Windows compiler: a build that
//! downloaded a program and ran it silently would be making that decision for whoever ran the build.
//! What is given instead is everything needed to lay one down.

use std::path::PathBuf;

use crate::Error;
use crate::paths::Workspace;
use crate::toolchain::Tool;

/// The nFPM release this project packages with.
const VERSION: &str = "2.48.0";

/// The hash the release's archive must have.
const SHA256: &str = "6d7f9a28b2f397934403950fcbba83f598b08d02edbe79a0815b3b4897341750";

/// Where the release's archive is fetched from.
const URL: &str =
    "https://github.com/goreleaser/nfpm/releases/download/v2.48.0/nfpm_2.48.0_Linux_x86_64.tar.gz";

/// The directory under the project's own cache the toolchain lives in.
const DIRECTORY: &str = "nfpm";

/// The program itself.
const BINARY: &str = "nfpm";

/// The packager the Linux packages are built with: the toolchain under the project's own cache, or
/// the one this machine has on its path.
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

    if let Some(found) = Tool::on_path(BINARY) {
        eprintln!(
            "nFPM is not laid out under {}; using the one on this machine's path: {}",
            laid_out.display(),
            found.display()
        );
        return Ok(found);
    }

    let directory = workspace.toolchain_dir()?.join(DIRECTORY);
    let archive = directory.join("nfpm.tar.gz");

    Err(Error::Missing {
        summary: format!(
            "nFPM {VERSION} is neither laid out in {} nor on this machine's path, and the Linux packages are built with it",
            laid_out.display()
        ),
        advice: vec![
            format!("It is fetched from:  {URL}"),
            format!("It hashes to:        {SHA256}  (sha256)"),
            String::from("Lay it out where this project looks for it first, under its own cache:"),
            format!("  mkdir -p \"{}\"", directory.display()),
            format!("  curl -L -o \"{}\" {URL}", archive.display()),
            format!(
                "  echo \"{SHA256}  {}\" | sha256sum -c -",
                archive.display()
            ),
            format!(
                "  tar -xzf \"{}\" -C \"{}\"",
                archive.display(),
                directory.display()
            ),
            format!("  chmod +x \"{}\"", laid_out.display()),
            String::from(
                "An nFPM already installed on this machine needs neither that nor a copy: the nfpm on the path is found instead.",
            ),
        ],
    })
}
