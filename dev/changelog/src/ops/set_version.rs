//! `set-version`: the workspace version, and the lock beside it.
//!
//! The version is written into the manifest's `[workspace.package]` table, and a cargo run resolves
//! the workspace again so the member versions in `Cargo.lock` move with it. Both files are put back
//! when that run fails: a version that was not finished being set is worse than one that was not
//! set at all.

use mingling::{
    Grouped,
    consts::UNMATCHED,
    macros::{arg, chain, command, empty_result},
    picker::EntryPicker,
    res::ResExitCode,
};

use super::{ensure_clean, refuse_unmatched};
use crate::paths::Workspace;
use crate::setup::ResAllowDirty;
use crate::tool::Tool;
use crate::version::Version;
use crate::{Error, FailedChangelog, Next, manifest, run_step};

/// The file the manifest is assembled in before it replaces the manifest.
const MANIFEST_TEMP: &str = "__Cargo.toml.tmp";

/// Sets the workspace version and refreshes the lock file.
#[command(node = "set-version")]
pub fn set_version(args: EntrySetVersion) -> Next {
    // UNWRAP: picking the version and the words left over can each fall back, so neither fails.
    let (version, unmatched) = args.pick(arg![Option<String>]).pick(&UNMATCHED).unwrap();

    if let Some(next) = refuse_unmatched(&unmatched, "set-version") {
        return next;
    }

    let Some(version) = version else {
        return FailedChangelog {
            error: Error::VersionNotNamed,
        }
        .into();
    };

    match Version::parse(&version) {
        Ok(version) => StateSetVersion {
            version: version.as_str().to_owned(),
        }
        .into(),
        Err(error) => FailedChangelog { error }.into(),
    }
}

/// The state the command starts in: the version to set.
#[derive(Grouped)]
pub struct StateSetVersion {
    /// The version, already checked.
    version: String,
}

/// Sets the version, and refreshes the lock file with a cargo run.
#[chain]
pub fn handle_set_version(
    state: StateSetVersion,
    allow_dirty: &ResAllowDirty,
    ec: &mut ResExitCode,
) -> Next {
    let StateSetVersion { version } = state;

    run_step(ec, || set_the_version(&version, allow_dirty.allowed()));

    empty_result!()
}

/// Sets the version and runs cargo, restoring both files when the run fails.
fn set_the_version(version: &str, allow_dirty: bool) -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    ensure_clean(&workspace, allow_dirty)?;

    let manifest = workspace.manifest_path();
    let original = std::fs::read_to_string(&manifest).map_err(|error| Error::Io {
        doing: "reading the workspace manifest",
        detail: format!("{}: {error}", manifest.display()),
    })?;
    let updated = manifest::set_version(&manifest, &original, version)?;

    let lock = workspace.root().join("Cargo.lock");
    let original_lock = read_optional(&lock)?;

    write_manifest(&workspace, updated.as_bytes())?;

    if let Err(error) = Tool::cargo().run_in(workspace.root(), ["check", "--workspace", "--quiet"])
    {
        let restored = write_manifest(&workspace, original.as_bytes())
            .and_then(|()| restore_optional(&lock, original_lock));

        if let Err(restore_error) = restored {
            eprintln!(
                "the version was set but the manifest and lock could not be restored: \
                 {restore_error}"
            );
        }

        return Err(error);
    }

    eprintln!("version = {version}");

    Ok(())
}

/// Writes the manifest through a file beside it, so a failure leaves no half-written manifest.
fn write_manifest(workspace: &Workspace, contents: &[u8]) -> Result<(), Error> {
    let temp = workspace.root().join(MANIFEST_TEMP);
    std::fs::write(&temp, contents).map_err(|error| Error::Io {
        doing: "writing the manifest draft",
        detail: format!("{}: {error}", temp.display()),
    })?;

    let manifest = workspace.manifest_path();
    let renamed = std::fs::rename(&temp, &manifest);

    if let Err(error) = renamed {
        std::fs::remove_file(&temp).ok();

        return Err(Error::Io {
            doing: "replacing the manifest",
            detail: format!("{} -> {}: {error}", temp.display(), manifest.display()),
        });
    }

    Ok(())
}

/// A file that may not be there.
fn read_optional(path: &std::path::Path) -> Result<Option<Vec<u8>>, Error> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::Io {
            doing: "reading the lock file",
            detail: format!("{}: {error}", path.display()),
        }),
    }
}

/// Puts a file that may not have been there back.
fn restore_optional(path: &std::path::Path, original: Option<Vec<u8>>) -> Result<(), Error> {
    original.map_or_else(
        || match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Io {
                doing: "removing the lock file the failed run made",
                detail: format!("{}: {error}", path.display()),
            }),
        },
        |bytes| {
            std::fs::write(path, bytes).map_err(|error| Error::Io {
                doing: "restoring the lock file",
                detail: format!("{}: {error}", path.display()),
            })
        },
    )
}
