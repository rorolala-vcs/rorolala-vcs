//! `git-lock`: the workspace packed into temporary commits a run can happen on.
//!
//! A run must not meet a dirty tree: a step that rewrites generated files would be
//! indistinguishable from a person's change. Locking pins `HEAD` to a backup branch, commits the
//! dirty tree as a plain temporary commit, and puts a marker in a second, named commit; the unlock
//! half reads that marker to know which of the two paths puts the tree back.
//!
//! It is a utility around a run and never a step of `check`: a gate that committed the tree it was
//! handed would be a gate about something other than what was committed.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{Error, paths::Workspace};

use crate::Next;
use crate::git;

/// Pins `HEAD` to a backup branch and commits the workspace into a temporary commit.
#[command(node = "git-lock")]
pub fn git_lock() -> StateGitLock {
    StateGitLock
}

/// The state the command starts in: it names nothing, since the whole tree is what is locked.
#[derive(Grouped)]
pub struct StateGitLock;

/// Locks the workspace, and answers whether its tree was dirty.
///
/// # Errors
///
/// Reports the first failure of discovering the workspace, running git, or writing the marker.
fn lock_workspace() -> Result<bool, Error> {
    let workspace = Workspace::discover()?;
    let root = workspace.root();

    git::run(root, ["branch", "-f", git::BACKUP_BRANCH, "HEAD"])?;

    let dirty = git::is_dirty(&git::porcelain(root)?);

    if dirty {
        git::run(root, ["add", "."])?;
        git::run(root, ["commit", "-m", git::TEMP_COMMIT_MESSAGE])?;
    }

    git::write_marker(root, dirty)?;

    // Everything is committed under the marker commit, not only the marker: whatever a step left in
    // the tree between the two commits is restored by the same path the marker names.
    git::run(root, ["add", "."])?;
    git::run(root, ["commit", "-m", git::CI_TEMP_COMMIT_MESSAGE])?;

    Ok(dirty)
}

/// Locks the workspace for a run.
#[chain]
pub fn handle_git_lock(_state: StateGitLock, ec: &mut ResExitCode) -> Next {
    match lock_workspace() {
        Ok(true) => println!("locked: dirty tree committed as a temporary commit"),
        Ok(false) => println!("locked: clean tree marked for the run"),
        Err(error) => {
            eprintln!("{error}");
            ec.exit_code = error.exit_code();
        }
    }

    empty_result!()
}
