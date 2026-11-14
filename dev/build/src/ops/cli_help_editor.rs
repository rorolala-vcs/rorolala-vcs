//! `cli-help-editor`: the browser editor over the CLI's i18n copy.
//!
//! The editor is a Node.js program beside this crate. This step finds the workspace,
//! checks that `node` is present and on the line the editor needs, and runs the server
//! in the workspace root, where it prints the local address and opens the browser.
//!
//! The server runs in the foreground on purpose: Ctrl+C is how a person stops it, and
//! the terminal that started it is the one that owns it. A status that is not a
//! success, a signal included, is therefore not a build failure the caller should
//! report.

use std::ffi::OsString;
use std::process::ExitStatus;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::paths::Workspace;
use crate::toolchain::Tool;
use crate::{Error, Next, run_step};

/// The oldest Node.js the editor runs on.
const NODE_MAJOR: u32 = 20;

/// Opens the CLI help editor.
#[command(node = "cli-help-editor")]
pub fn cli_help_editor() -> StateCliHelpEditor {
    StateCliHelpEditor
}

/// The state the command starts in: the editor takes no arguments, because the
/// workspace and the i18n trees are what it reads.
#[derive(Grouped)]
pub struct StateCliHelpEditor;

/// Runs the editor server, and waits for the person to stop it.
///
/// # Errors
///
/// Reports a missing or too old Node.js, a server that cannot be started, and a
/// workspace that cannot be found. A server stopped by Ctrl+C is not an error.
#[chain]
pub fn handle_cli_help_editor(_state: StateCliHelpEditor, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        let workspace = Workspace::discover()?;
        let root = workspace.root();
        check_node()?;

        let server = root.join("dev").join("cli-help-editor").join("server.mjs");
        let status = Tool::node().status_in(
            root,
            &[],
            [
                server.into_os_string(),
                OsString::from("--root"),
                root.as_os_str().to_os_string(),
            ],
        )?;

        stopped(status)
    });

    empty_result!()
}

/// Reads what the editor's process ended with.
///
/// A signal is a person stopping the editor rather than a failure, and a plain
/// success is the same. Any other status keeps its number, so a server that refused
/// to start is still reported.
fn stopped(status: ExitStatus) -> Result<(), Error> {
    if status.success() || status.code().is_none() {
        return Ok(());
    }

    Err(Error::Status {
        program: "node".to_owned(),
        code: status.code().unwrap_or(1),
    })
}

/// Checks that the machine has a Node.js the editor can run on.
///
/// The editor uses ES modules and the built-in test runner, both of which are part of
/// Node.js 20. A version older than that is refused here, where the message can say
/// what to install, instead of failing later in a page that never loads.
fn check_node() -> Result<(), Error> {
    let missing = || Error::Missing {
        summary: "Node.js 20 or newer is needed by the CLI help editor".to_owned(),
        advice: vec![
            "Install Node.js 20 or newer.".to_owned(),
            "Run `cargo dev cli-help-editor` again afterwards.".to_owned(),
        ],
    };

    let output = Tool::node().capture(["--version"]).map_err(|_| missing())?;
    if !output.status.success() {
        return Err(missing());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let major = text
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|part| part.parse::<u32>().ok());

    match major {
        Some(version) if version >= NODE_MAJOR => Ok(()),
        _ => Err(Error::Missing {
            summary: format!(
                "Node.js 20 or newer is needed by the CLI help editor, and `node --version` said `{}`",
                text.trim()
            ),
            advice: vec!["Install Node.js 20 or newer.".to_owned()],
        }),
    }
}
