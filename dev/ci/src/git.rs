//! The git work behind the lock pair: pinning `HEAD`, packing a dirty tree, and putting it back.
//!
//! It lives apart from the two commands because what a marker means and what a commit subject is
//! are questions that can be answered without a repository, and are tested as such. Nothing here
//! decides when to run: that is the commands'.
//!
//! Every git invocation runs with the workspace root as its working directory, so a run started in
//! a crate directory pins and restores the same tree as one started at the top.

use std::ffi::OsString;
use std::path::Path;

use rola_build::Error;
use rola_build::toolchain::Tool;

/// The branch the locked `HEAD` is pinned to, created or force-reset by `git-lock`.
pub const BACKUP_BRANCH: &str = "rola/bkup";

/// The file `git-lock` leaves at the workspace root; its content says whether the tree was dirty
/// when it was locked.
pub const MARKER_FILE: &str = "ROLAA-CI-CHECKING";

/// The first temporary commit: the dirty tree packed so that it can be put back afterwards. Only
/// made when the tree was dirty.
pub const TEMP_COMMIT_MESSAGE: &str = "[DO NOT PUSH] TEMP [DO NOT PUSH]";

/// The second temporary commit: the one carrying the marker. Its subject is what `git-unlock`
/// matches before it touches anything.
pub const CI_TEMP_COMMIT_MESSAGE: &str = "[DO NOT PUSH] CI TEMP [DO NOT PUSH]";

/// The words in a commit subject that mark the commit `git-lock` made.
///
/// They are matched case-sensitively, because a commit that says `ci temp` is not one this
/// program wrote.
const CI_TEMP_MARK: &str = "CI TEMP";

/// How a locked workspace is put back, as the marker of its lock says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestorePath {
    /// The tree was clean when it was locked: the marker commit is all that was added, so one hard
    /// reset lands on the tree as it was.
    Clean,
    /// The tree was dirty when it was locked: a plain `TEMP` commit of the user's changes sits
    /// below the marker commit, and has to be unwrapped back into the working tree.
    Dirty,
}

/// The restore path a marker names, or `None` for a marker this program did not write.
///
/// A lock whose marker cannot be read is a lock this program does not know how to undo, and that
/// is a refusal rather than something to guess at.
pub fn restore_path(marker: &str) -> Option<RestorePath> {
    match marker.trim() {
        "true" => Some(RestorePath::Dirty),
        "false" => Some(RestorePath::Clean),
        _ => None,
    }
}

/// The marker content that stands for `dirty`.
pub fn marker_content(dirty: bool) -> &'static str {
    if dirty { "true" } else { "false" }
}

/// Whether `subject` is the subject of the commit `git-lock` leaves at `HEAD`.
pub fn is_ci_temporary_commit(subject: &str) -> bool {
    subject.contains(CI_TEMP_MARK)
}

/// Whether porcelain status output describes a tree with changes in it.
///
/// A line is one changed path, so a line that is not blank is a change. Untracked files count: the
/// lock packs them into its temporary commit like any other change, so a tree whose only change is
/// a new file takes the restore path that puts it back rather than the one that discards it.
pub fn is_dirty(porcelain: &str) -> bool {
    porcelain.lines().any(|line| !line.trim().is_empty())
}

/// The failure of a request this program refuses rather than guesses at.
pub fn refusal(reason: String) -> Error {
    Error::Missing {
        summary: reason,
        advice: vec!["lock a workspace with `cargo ci git-lock` before unlocking it".to_owned()],
    }
}

/// Runs `git` with `args` in the workspace, and reports a status that was not a success.
///
/// # Errors
///
/// Reports [`Error::Tool`] when `git` cannot be started, and [`Error::Status`] when it ran and
/// failed; what it said is inherited, so the reader sees git's own words rather than a copy.
pub fn run(root: &Path, args: impl IntoIterator<Item = &'static str>) -> Result<(), Error> {
    Tool::git().run_in(root, args)
}

/// Runs `git` with `args` in the workspace and answers what it wrote to standard output.
///
/// `capture` takes no directory, so the working directory is given to git itself: `-C` is read
/// before the subcommand and is what makes this run on the workspace the caller named.
///
/// # Errors
///
/// Reports [`Error::Tool`] when `git` cannot be started, and [`Error::Status`] when it ran and
/// failed.
pub fn output(root: &Path, args: impl IntoIterator<Item = &'static str>) -> Result<String, Error> {
    let program = Tool::git();
    let mut words: Vec<OsString> = vec![OsString::from("-C"), root.as_os_str().to_owned()];
    words.extend(args.into_iter().map(OsString::from));

    let output = program.capture(words)?;

    if !output.status.success() {
        return Err(Error::Status {
            program: program.program().to_string_lossy().into_owned(),
            code: output.status.code().unwrap_or(1),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The subject line of the commit `HEAD` names.
///
/// # Errors
///
/// Reports whatever [`output`] reported.
pub fn head_subject(root: &Path) -> Result<String, Error> {
    output(root, ["log", "-1", "--pretty=%s"]).map(|subject| subject.trim().to_owned())
}

/// The working tree's changes, one porcelain line per path.
///
/// Porcelain rather than a plain `diff HEAD`: it also names untracked files, which the lock
/// commits, and it refreshes the index first, so a build that merely touched a file's timestamp
/// is not read as a change to its content.
///
/// # Errors
///
/// Reports whatever [`output`] reported.
pub fn porcelain(root: &Path) -> Result<String, Error> {
    output(root, ["status", "--porcelain"])
}

/// Writes the marker that says how this lock has to be undone.
///
/// # Errors
///
/// Reports [`Error::Io`] when the marker cannot be written.
pub fn write_marker(root: &Path, dirty: bool) -> Result<(), Error> {
    let path = root.join(MARKER_FILE);

    std::fs::write(&path, marker_content(dirty)).map_err(|error| Error::Io {
        doing: "writing the workspace lock marker",
        detail: format!("{}: {error}", path.display()),
    })
}

/// Reads the restore path out of the marker the lock left.
///
/// # Errors
///
/// Reports a refusal when the marker is missing or holds something no lock writes, and
/// [`Error::Io`] when it cannot be read.
pub fn read_restore_path(root: &Path) -> Result<RestorePath, Error> {
    let path = root.join(MARKER_FILE);

    let marker = match std::fs::read_to_string(&path) {
        Ok(marker) => marker,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(refusal(format!(
                "no `{MARKER_FILE}` at the workspace root: the workspace is not locked"
            )));
        }
        Err(error) => {
            return Err(Error::Io {
                doing: "reading the workspace lock marker",
                detail: format!("{}: {error}", path.display()),
            });
        }
    };

    restore_path(&marker).ok_or_else(|| {
        refusal(format!(
            "`{MARKER_FILE}` holds `{}`, which is not a marker a lock writes",
            marker.trim()
        ))
    })
}

/// Removes the marker the lock left.
///
/// The reset that undoes the lock usually takes the marker with it, since it is committed only in
/// the commit being dropped, so a marker that is already gone is not a failure.
///
/// # Errors
///
/// Reports [`Error::Io`] when the marker is there and cannot be removed.
pub fn remove_marker(root: &Path) -> Result<(), Error> {
    let path = root.join(MARKER_FILE);

    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Io {
            doing: "removing the workspace lock marker",
            detail: format!("{}: {error}", path.display()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{RestorePath, is_ci_temporary_commit, is_dirty, marker_content, restore_path};

    #[test]
    fn a_marker_is_read_as_the_restore_path_it_names() {
        assert_eq!(restore_path("true"), Some(RestorePath::Dirty));
        assert_eq!(restore_path("false"), Some(RestorePath::Clean));
        // The marker is read as a line, since a file written by an editor may end in one.
        assert_eq!(restore_path(" true\n"), Some(RestorePath::Dirty));
        assert_eq!(restore_path("false\r\n"), Some(RestorePath::Clean));
    }

    #[test]
    fn a_marker_no_lock_writes_names_no_restore_path() {
        for marker in ["", "TRUE", "1", "yes", "true false", "maybe"] {
            assert_eq!(restore_path(marker), None, "marker `{marker}`");
        }
    }

    #[test]
    fn the_marker_a_lock_writes_says_whether_the_tree_was_dirty() {
        assert_eq!(marker_content(true), "true");
        assert_eq!(marker_content(false), "false");
    }

    #[test]
    fn only_the_commit_that_says_ci_temp_is_the_locks_marker_commit() {
        assert!(is_ci_temporary_commit(
            "[DO NOT PUSH] CI TEMP [DO NOT PUSH]"
        ));
        assert!(is_ci_temporary_commit("CI TEMP"));
        // The plain commit of the user's changes sits below the marker one and is not it.
        assert!(!is_ci_temporary_commit("[DO NOT PUSH] TEMP [DO NOT PUSH]"));
        // The match is case-sensitive, as the subject this program writes is.
        assert!(!is_ci_temporary_commit(
            "[DO NOT PUSH] ci temp [DO NOT PUSH]"
        ));
        assert!(!is_ci_temporary_commit("a normal commit"));
    }

    #[test]
    fn each_porcelain_line_is_a_change_and_no_line_is_a_clean_tree() {
        assert!(!is_dirty(""));
        assert!(!is_dirty("  \n\n"));
        assert!(is_dirty(" M dev/ci/src/git.rs\n"));
        assert!(is_dirty("?? dev/ci/src/new.rs\n"));
        // A line of nothing is not a path.
        assert!(!is_dirty(" \n\t\n"));
    }
}
