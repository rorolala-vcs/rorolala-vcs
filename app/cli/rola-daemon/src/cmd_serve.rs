//! `rola-daemon serve`: serve one Vault over this process's standard streams.
//!
//! It is what a daemon behind an SSH session is, and it serves exactly one connection: the far
//! side starts it, `rola` on this side speaks the protocol through the process's standard input
//! and output, and when that connection ends the process ends. No port is bound and nothing is
//! listened on, so a machine can offer a Vault over `ssh` alone.
//!
//! Two things about it are contracts rather than choices. The standard output carries the protocol
//! and nothing else — not a greeting, not a result line — since a word written there is a word the
//! client reads as part of an answer. And the exit code is the daemon's own (see
//! [`rorolala_daemon::exit`]) rather than this program's, because the far side of a session has
//! only that number to tell "the path is not a Vault" from "this machine refuses sessions".

use std::path::Path;

use mingling::{
    Grouped, Routable, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, empty_result, help, metadata, r_eprintln,
        renderer, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResConcurrency;
use rorolala_daemon::{daemon_serve_vault, exit};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::{Next, errors::ErrorArgument, exit_codes::EC_HELP, failure::failure};

#[help(buffer)]
pub fn help_serve(_: EntryServe, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("serve.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryServe)]
pub fn desc_serve() -> Description {
    t!("serve.description").to_string().into()
}

/// Completes what `rola-daemon serve` can be given next.
///
/// The path is the filesystem's to answer, and a program run by a session has nothing of its own
/// to offer: the completion is what a shell would print for a path it was asked about.
#[completion(EntryServe)]
pub fn complete_serve(_ctx: ShellContext) -> Suggest {
    suggest!()
}

#[command(node = "serve")]
pub fn serve(args: EntryServe) -> Next {
    let picked = args
        .pick_or_route(arg![String], || ErrorArgument::missing("PATH").to_chain())
        .to_result();

    let path = match picked {
        Ok(path) => path,
        Err(next) => return next,
    };

    StateServe { path }.into()
}

/// The state of serving one Vault.
#[derive(Grouped)]
pub struct StateServe {
    /// The directory the Vault sits at, as it was written.
    path: String,
}

#[chain]
pub fn handle_serve(state: StateServe, concurrency: &ResConcurrency) -> Next {
    let StateServe { path } = state;

    // How much is carried at once is the run's to name. A session is one transfer, so the bound
    // is what it holds that one to rather than what it shares between several.
    let max_concurrency = concurrency
        .explicit()
        .then(|| u16::try_from(**concurrency).unwrap_or(u16::MAX));

    let stopped = daemon_serve_vault(Path::new(&path), max_concurrency);

    // A session that ended is a session that was served: what it was asked for was answered, and
    // the far side closed the connection. Saying so would be a word on the stream the client is
    // still reading, which is why the ordinary end is the silent one.
    if stopped.code() == exit::STOPPED {
        return empty_result!();
    }

    // What a session ends with is the daemon's own number rather than this program's, since it is
    // the only thing the far side has to tell one reason from another.
    ErrorNotServed::of(stopped.code(), &path).into()
}

/// Error: the Vault this process was asked to serve was not served.
#[derive(Grouped)]
pub struct ErrorNotServed {
    /// What went wrong, in the words this program says it in.
    reason: String,
    /// What a reader can do about it.
    help: String,
    /// The daemon's own reason, which is what the far side of a session branches on.
    code: u8,
}

impl ErrorNotServed {
    /// The wording for a session that stopped for `code`, over the Vault at `path`.
    fn of(code: u8, path: &str) -> Self {
        let (reason, help) = match code {
            exit::NO_IDENTITY => (
                t!("start.err_no_identity").to_string(),
                t!("start.err_no_identity_help").to_string(),
            ),
            exit::NO_VAULT => (
                t!("error.not_a_vault", path = path).to_string(),
                t!("error.not_a_vault_help").to_string(),
            ),
            exit::SSH_DISABLED => (
                t!("serve.err_ssh_disabled").to_string(),
                t!("serve.err_ssh_disabled_help").to_string(),
            ),
            exit::PREFERENCE_UNREADABLE => (
                t!("serve.err_preference_unreadable").to_string(),
                t!("serve.err_preference_unreadable_help").to_string(),
            ),
            _ => (
                t!("serve.err_session_failed").to_string(),
                t!("serve.err_session_failed_help").to_string(),
            ),
        };

        Self { reason, help, code }
    }
}

impl ErrorNotServed {
    /// What a reader can do about it.
    fn help(&self) -> &str {
        &self.help
    }

    /// The daemon's own reason for stopping.
    const fn code(&self) -> u8 {
        self.code
    }
}

impl Failure for ErrorNotServed {
    fn name(&self) -> &'static str {
        "error_not_served"
    }

    fn reason(&self) -> String {
        self.reason.clone()
    }
}

failure!(ErrorNotServed);

#[renderer(buffer)]
pub fn render_error_not_served(error: ErrorNotServed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(error.help().trim()));
    ec.exit_code = i32::from(error.code());
}
