//! The compiler the Windows installer is built with.
//!
//! It is pinned the way the icon set is pinned, and for the same reason: what a package is built
//! with decides what is in it, so it is named rather than found. The project's own copy is preferred
//! — a package built with a compiler this project names is one that anybody can build again the
//! same way — and a machine's own is taken where there is no other, so a developer who already has
//! Inno Setup installed is not made to lay a second copy down.
//!
//! Nothing is fetched and nothing is installed. Laying a toolchain down is a decision, and a build
//! that downloaded a program and ran it silently would be making that decision for whoever ran the
//! build, in a place they did not choose. What is given instead is everything needed to make one.

use std::path::{Path, PathBuf};

use crate::{Error, fs, paths::Workspace};

/// The Inno Setup release this project packages with.
const VERSION: &str = "6.7.3";

/// The hash the release's installer must have.
const SHA256: &str = "9C73C3BAE7ED48D44112A0F48E66742C00090BDB5BEF71D9D3C056C66E97B732";

/// Where the release's installer is fetched from.
const URL: &str =
    "https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe";

/// The directory under the project's own cache the toolchain lives in.
const DIRECTORY: &str = "innosetup";

/// The compiler itself.
const COMPILER: &str = "ISCC.exe";

/// The compiler the Windows installer is built with: the toolchain under the project's own cache, or
/// the one this machine has on its path.
///
/// # Errors
///
/// Reports [`Error::Missing`] with the version, the source, the hash and the command that lays it
/// down when neither is there.
pub fn require(workspace: &Workspace) -> Result<PathBuf, Error> {
    let laid_out = workspace.toolchain_dir()?.join(DIRECTORY).join(COMPILER);
    if laid_out.is_file() {
        return Ok(laid_out);
    }

    if let Some(found) = on_path(COMPILER) {
        eprintln!(
            "Inno Setup is not laid out under {}; using the one on this machine's path: {}",
            laid_out.display(),
            found.display()
        );
        return Ok(found);
    }

    Err(Error::Missing {
        summary: format!(
            "Inno Setup {VERSION} is neither laid out in {} nor on this machine's path, and the Windows installer is compiled with it",
            laid_out.display()
        ),
        advice: vec![
            format!("It is fetched from:  {URL}"),
            format!("It hashes to:        {SHA256}  (sha256)"),
            String::from("Lay it out where this project looks for it first, under its own cache:"),
            format!(
                "  \"{}\" /VERYSILENT /CURRENTUSER /SUPPRESSMSGBOXES /NORESTART /SP- /NOICONS /DIR=\"{}\"",
                laid_out.with_extension("exe").display(),
                workspace.toolchain_dir()?.join(DIRECTORY).display()
            ),
            String::from(
                "An Inno Setup already installed on this machine needs neither that nor a copy: its directory on the path is found, and what is read from it is ISCC.exe and the files beside it.",
            ),
        ],
    })
}

/// Where a program named `name` is on this machine's path, if it is.
fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;

    // A Windows program's file name carries its extension and the shell's lookup does not, so what a
    // command name is looked for under is stated by the machine rather than assumed here.
    let extensions: Vec<String> = std::env::var("PATHEXT")
        .map(|pathext| pathext.split(';').map(str::to_owned).collect())
        .unwrap_or_default();

    for directory in std::env::split_paths(&path) {
        let exact = directory.join(name);
        if exact.is_file() {
            return Some(exact);
        }

        for extension in &extensions {
            let candidate = directory.join(format!("{name}{extension}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Copies the installer's compiler and the files beside it into the layout, where a package is made
/// from it rather than from where the compiler was found.
///
/// The Inno Setup a machine has is read where it is: this is only for the copy the project lays down
/// itself, which is the one that may be told to.
///
/// # Errors
///
/// Reports [`Error::Io`] when the directory cannot be read or copied.
pub fn copy_toolchain(from: &Path, into: &Path) -> Result<(), Error> {
    fs::copy_directory(from, into)
}
