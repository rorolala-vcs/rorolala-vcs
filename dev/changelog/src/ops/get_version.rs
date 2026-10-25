//! `get-version`: the workspace version, on stdout.
//!
//! A tree with no version is not a failure: nothing is printed, and the status is
//! [`crate::EC_NO_VERSION`] so a script can tell "there is none" from "the command went wrong".

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::paths::Workspace;
use crate::{EC_NO_VERSION, Error, Next, manifest};

/// Prints the workspace version, or says there is none by its status.
#[command(node = "get-version")]
pub fn get_version() -> StateGetVersion {
    StateGetVersion
}

/// The state the command starts in: it names nothing.
#[derive(Grouped)]
pub struct StateGetVersion;

/// Prints the version, or exits with [`EC_NO_VERSION`](crate::EC_NO_VERSION) when there is none.
#[chain]
pub fn handle_get_version(_state: StateGetVersion, ec: &mut ResExitCode) -> Next {
    match version() {
        Ok(Some(version)) => println!("{version}"),
        Ok(None) | Err(Error::NotRepository | Error::NoWorkspace { .. }) => {
            ec.exit_code = EC_NO_VERSION;
        }
        Err(error) => {
            eprintln!("{error}");
            ec.exit_code = error.exit_code();
        }
    }

    empty_result!()
}

/// The version the workspace states, if it states one.
fn version() -> Result<Option<String>, Error> {
    let workspace = Workspace::discover()?;
    let manifest = workspace.manifest_path();
    let text = std::fs::read_to_string(&manifest).map_err(|error| Error::Io {
        doing: "reading the workspace manifest",
        detail: format!("{}: {error}", manifest.display()),
    })?;

    manifest::version(&manifest, &text)
}
