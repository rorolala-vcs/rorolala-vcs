//! The `refresh` command: applying `rola.lock` to the work tree without asking a Vault.
//!
//! `update` resolves `rola.toml` through the Vaults and writes a new lock, and `restore` puts named
//! materials back from it; this is the command that takes the lock a checkout left and makes the
//! work tree, `.rola/git/state.toml` and `.git/info/exclude` agree with it. It reads neither
//! `rola.toml` nor the Vault, and it writes no lock: a configuration change is `update`'s to
//! apply.
//!
//! The work is [`settle`](crate::settle)'s, shared with `hook`, so what a hook does after git
//! changes the tree and what a person does by asking for it are one run.

// mingling's `#[command]` expands the entry type and the dispatch function beside the command, and
// the expansion is not documented; the items this module writes carry their own docs.
#![allow(missing_docs)]
// A command's words arrive as the value the framework's entry converts to, so the parameter is the
// framework's shape rather than this module's choice: taking them by reference is not an option the
// generated wrapper offers.
#![allow(clippy::needless_pass_by_value)]

use mingling::{
    Grouped,
    macros::{chain, command},
};
use rola_git_support::Session;
use rust_i18n::t;

use crate::Next;
use crate::Refused;
use crate::options::operands;

/// Puts the working tree back in step with `rola.lock`
///
/// The materials the project recorded are taken out, the lock's are placed from the store or the
/// staging area, and the exclude block is rewritten from the lock. Nothing is asked of a Vault and
/// nothing is written to `rola.toml` or `rola.lock`.
///
/// A material that was changed after it was placed stops the run before anything is moved, so a
/// change is never thrown away silently; `restore` is how one is thrown away on purpose.
#[command(node = "refresh", entry = EntryRefresh)]
pub fn refresh(words: Vec<String>) -> Next {
    if !operands(&words).is_empty() {
        return Refused { command: "refresh" }.into();
    }

    StateRefresh.into()
}

/// The state a refresh starts in: the command names nothing.
#[derive(Grouped)]
pub struct StateRefresh;

/// Applies the lock, or says why the project cannot be acted in.
#[chain]
pub fn handle_refresh(_state: StateRefresh) -> Next {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return crate::Fatal::new(
                t!("git_rola.err_cwd", error = error.to_string()).to_string(),
            )
            .into();
        }
    };

    // Unlike a hook, this is a run a person asked for, so a work tree this build does not act in is
    // refused rather than passed over: a checkout still has to work, a command does not.
    let session = match Session::open(&cwd) {
        Ok(Some(session)) => session,
        Ok(None) => {
            return crate::Fatal::new(t!("git_rola.err_no_project").to_string()).into();
        }
        Err(error) => return crate::Fatal::new(error.to_string()).into(),
    };

    match crate::settle::apply(&session) {
        Ok(result) => result.into(),
        Err(message) => crate::Fatal::new(message).into(),
    }
}
