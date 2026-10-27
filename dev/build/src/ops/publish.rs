//! `publish`: an export laid out and packaged, which is what a person is handed.
//!
//! What an export is made of is programs, and what someone who is not a developer wants is something
//! that puts them where a shell and a file manager will find them. The platforms do not agree on what
//! that is: Windows is given an installer — a program that copies the layout, writes the two things
//! that make a program reachable, and can be asked to undo all of it — Linux is given the packages a
//! distribution installs and an `AppImage`, and the other Unixes are given an archive and the script
//! that installs it, since what a Unix hands over is a tree and a script.

use std::path::Path;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
#[cfg(not(target_os = "linux"))]
use crate::fs;
#[cfg(not(target_os = "linux"))]
use crate::iscc;
#[cfg(not(target_os = "linux"))]
use crate::ops::export::export_artifacts;
use crate::paths::Workspace;
#[cfg(not(target_os = "linux"))]
use crate::platform::{self, Platform};
use crate::run_step;
#[cfg(not(target_os = "linux"))]
use crate::toolchain::Tool;

/// The Inno Setup script the Windows installer is compiled from.
#[cfg(not(target_os = "linux"))]
const INSTALLER_SCRIPT: &str = "dev/packaging/windows/rorolala.iss";

/// The script a Unix install is made with, laid beside what it installs.
#[cfg(not(target_os = "linux"))]
const INSTALL_SCRIPT: &str = "dev/packaging/unix/install.sh";

/// Lays an export out and packages it.
#[command(node = "publish")]
pub fn publish() -> StatePublish {
    StatePublish
}

/// The state the command starts in: it names nothing, since the whole export is what is packaged.
#[derive(Grouped)]
pub struct StatePublish;

/// The archive a Unix is given, and the installer a Windows is given, are both named for what they
/// are, the version they are of, and the platform they are for, since that is what tells two
/// hand-offs apart on a disk that has both.
pub fn publish_package() -> Result<(), crate::Error> {
    #[cfg(target_os = "linux")]
    {
        let workspace = Workspace::discover()?;

        crate::linux::publish(&workspace)
    }

    #[cfg(not(target_os = "linux"))]
    {
        let workspace = Workspace::discover()?;
        let version = workspace.version()?;
        let publisher = workspace.publisher()?;
        let build = workspace.build_dir()?;
        let platform = Platform::current();
        let setup = format!("rorolala-{version}-setup");

        // The toolchain first, and deliberately: what an export costs is minutes of compiling, and a
        // machine that has no compiler for the installer should say so before one is built for a
        // package that cannot be made.
        let compiler = if matches!(platform, Platform::Windows) {
            Some(iscc::require(&workspace)?)
        } else {
            None
        };

        export_artifacts(false)?;

        if let Some(compiler) = compiler {
            let installer = build.join(format!("{setup}.exe"));

            // The compiler reads its sources out of the layout and writes the installer into the
            // layout, so anything of a previous build left there would be read as part of the
            // payload: the export cleared the directory first, and this makes that true even of a
            // run that did not export.
            fs::remove(&installer)?;

            // The script is told the three things it cannot work out for itself: what version this
            // is, what the installer is to be called, and where the layout it packages was laid and
            // is to be written.
            let status = std::process::Command::new(compiler)
                .arg("/Q")
                .arg(format!("/DVersion={version}"))
                .arg(format!("/DPublisher={publisher}"))
                .arg(format!("/DSetup={setup}"))
                .arg(format!("/DSourceDir={}", build.display()))
                .arg(format!("/DOutputDir={}", build.display()))
                .arg(workspace.root().join(INSTALLER_SCRIPT))
                .status()
                .map_err(|error| crate::Error::Tool {
                    program: "ISCC".to_owned(),
                    detail: error.to_string(),
                })?;

            if !status.success() {
                return Err(crate::Error::Status {
                    program: "ISCC".to_owned(),
                    code: status.code().unwrap_or(1),
                });
            }

            report(&installer)
        } else {
            // The layout and the script that installs it, as one archive: what a Unix is given is
            // the tree as it was laid out, so nothing here changes it beyond adding the script a
            // reader runs.
            let script = build.join("install.sh");
            fs::copy_file(&workspace.root().join(INSTALL_SCRIPT), &script)?;
            make_executable(&script)?;

            let archive = build.join(format!(
                "rorolala-{version}-{}-{}.tar.gz",
                platform::uname_os(std::env::consts::OS),
                platform::uname_arch(std::env::consts::ARCH)
            ));

            Tool::tar().run_in(&build, ["-czf", &archive.to_string_lossy(), "-C", ".", "."])?;

            report(&archive)
        }
    }
}

/// Says where a package was written and how large it is: what a hand-off is, in one line.
pub fn report(path: &Path) -> Result<(), crate::Error> {
    let length = std::fs::metadata(path)
        .map_err(|error| crate::Error::Io {
            doing: "reading the package",
            detail: format!("{}: {error}", path.display()),
        })?
        .len();

    println!("{} = {length} bytes", path.display());

    Ok(())
}

/// Makes the install script a script a reader can run, on the platforms where that is a file mode.
#[cfg(not(target_os = "linux"))]
#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), crate::Error> {
    use std::os::unix::fs::PermissionsExt as _;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| crate::Error::Io {
            doing: "reading the install script's mode",
            detail: format!("{}: {error}", path.display()),
        })?
        .permissions();
    permissions.set_mode(0o755);

    std::fs::set_permissions(path, permissions).map_err(|error| crate::Error::Io {
        doing: "making the install script executable",
        detail: format!("{}: {error}", path.display()),
    })
}

/// Leaves the script as it is: Windows has no file mode for a shell script to be run under.
///
/// The signature is the one the Unix twin has, so the call site reads the same on both platforms;
/// returning a success is the whole of what this side does.
#[cfg(not(target_os = "linux"))]
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn make_executable(_path: &Path) -> Result<(), crate::Error> {
    Ok(())
}

/// Lays the package out.
#[chain]
pub fn handle_publish(_state: StatePublish, ec: &mut ResExitCode) -> Next {
    run_step(ec, publish_package);

    empty_result!()
}
