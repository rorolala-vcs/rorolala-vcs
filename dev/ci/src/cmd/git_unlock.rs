//! `git-unlock`: the workspace a `git-lock` packed, put back.
//!
//! It refuses on any `HEAD` that is not the lock's marker commit, and refuses when the marker
//! cannot say which restore path to take: a tree put back the wrong way loses a person's work, so
//! nothing is a better answer than a guess.
//!
//! When the restore works, the run is still judged: a tree that was dirty before restoring means
//! the run changed something it should not have, and the non-zero status is what says so. That is
//! the idempotency signal a gate needs, and the reason this command is a utility beside `check`
//! rather than a step of it.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{Error, paths::Workspace};

use crate::Next;
use crate::git::{self, RestorePath};

/// The status of a restore that worked but found the tree changed under the run: a run that
/// contaminates the workspace is a failure even though nothing of it is left behind.
const EC_CONTAMINATED: i32 = 1;

/// Print the tracked changes a run left behind before they are discarded.
const SHOW_DIFF: &str = "--show-diff";

/// Restores the workspace a `git-lock` packed.
// The flag is read from the words directly because this crate takes mingling without its `picker`
// feature: one word does not pay for turning that on, and the words themselves are what the command
// macro is handed either way.
// `#[command]` rewrites the owned first parameter into the entry type, so the words are taken by
// value; the body only reads them to find the one flag this command has.
#[allow(clippy::needless_pass_by_value)]
#[command(node = "git-unlock")]
pub fn git_unlock(args: Vec<String>) -> StateGitUnlock {
    StateGitUnlock {
        show_diff: args.iter().any(|word| word == SHOW_DIFF),
    }
}

/// The state the restore starts in: whether a contaminated run's diff is printed.
///
/// It is `Copy` because it is one flag: the framework hands a state on by value, and a marker for
/// what to print is not something to own.
#[derive(Grouped, Clone, Copy)]
pub struct StateGitUnlock {
    /// Whether the tracked changes a run left behind are printed before they are discarded.
    show_diff: bool,
}

/// Restores the workspace, and answers whether the run left it dirty.
///
/// # Errors
///
/// Reports a refusal when `HEAD` is not the lock's commit or the marker does not name a restore
/// path, and the first failure of discovering the workspace, running git, or removing the marker.
fn unlock_workspace(show_diff: bool) -> Result<bool, Error> {
    let workspace = Workspace::discover()?;
    let root = workspace.root();

    let subject = git::head_subject(root)?;
    if !git::is_ci_temporary_commit(&subject) {
        return Err(git::refusal(format!(
            "HEAD is not a CI temporary commit: `{subject}`"
        )));
    }

    // The marker is read before anything is touched: a refusal changes nothing.
    let restore = git::read_restore_path(root)?;

    // Recorded before the restore, because the restore is what discards it.
    let dirty = git::is_dirty(&git::porcelain(root)?);

    if dirty && show_diff {
        // git prints the diff itself, so what a reader sees is what is about to be discarded.
        git::run(root, ["diff", "HEAD"])?;
    }

    // Both paths drop the marker commit and land on the commit the lock started from.
    git::run(root, ["reset", "--hard", "HEAD~1"])?;

    if restore == RestorePath::Dirty {
        // Unwrap the plain `TEMP` commit into the staging area, then unstage it back into the
        // working tree, which is where the user's changes were before the lock.
        git::run(root, ["reset", "--soft", "HEAD~1"])?;
        git::run(root, ["reset"])?;
    }

    git::remove_marker(root)?;

    Ok(dirty)
}

/// Restores the workspace, and fails the run when it was contaminated.
#[chain]
pub fn handle_git_unlock(state: StateGitUnlock, ec: &mut ResExitCode) -> Next {
    match unlock_workspace(state.show_diff) {
        Ok(true) => {
            println!("unlocked: workspace restored, but the run left the tree dirty");
            ec.exit_code = EC_CONTAMINATED;
        }
        Ok(false) => println!("unlocked: workspace restored"),
        Err(error) => {
            eprintln!("{error}");
            ec.exit_code = error.exit_code();
        }
    }

    empty_result!()
}
