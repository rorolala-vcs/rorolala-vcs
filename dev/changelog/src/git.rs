//! Whether the files a run rewrites have changes of their own.
//!
//! A changelog is rewritten by replacing the whole file, and a version is set by rewriting a
//! manifest line. Both are safe only on files whose changes are the ones being made, so the check
//! is a refusal rather than an attempt to guess which changes were the user's. The rest of the tree
//! is not this check's business: a change to a path a run does not write is not one a rewrite would
//! be mixed into.

use std::ffi::OsString;
use std::path::Path;

use crate::{Error, tool::Tool};

/// The working tree's changes, one porcelain line per path.
///
/// Porcelain rather than a plain `diff HEAD`: it names untracked files as well, which are exactly
/// the ones a first changelog appears as.
///
/// # Errors
///
/// Reports [`Error::Tool`] when `git` cannot be started, and [`Error::Status`] when it ran and
/// failed.
pub fn porcelain(root: &Path) -> Result<String, Error> {
    let program = Tool::git();
    let output = program.capture([
        OsString::from("-C"),
        root.as_os_str().to_owned(),
        OsString::from("status"),
        OsString::from("--porcelain"),
    ])?;

    if !output.status.success() {
        return Err(Error::Status {
            program: program.name(),
            code: output.status.code().unwrap_or(1),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The porcelain lines naming one of `paths`, relative to the repository root.
///
/// A line names one changed path, and a rename names two: the one that left and the one that
/// arrived. Either is a change to a file a run rewrites, so a line matching either end is answered
/// with. A path no line names is not changed.
#[must_use]
pub fn changed<'a>(porcelain: &'a str, paths: &[&str]) -> Vec<&'a str> {
    porcelain
        .lines()
        .filter(|line| named_paths(line).iter().any(|path| paths.contains(path)))
        .collect()
}

/// The paths one porcelain line names, without its status and without git's quoting.
fn named_paths(line: &str) -> Vec<&str> {
    // Porcelain writes `XY path`; the path starts at the third byte. A blank line has none.
    let Some(rest) = line.get(3..) else {
        return Vec::new();
    };

    rest.trim()
        .split(" -> ")
        .map(|path| path.trim().trim_matches('"'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::changed;

    /// The files a changelog rewrite replaces.
    const REWRITTEN: [&str; 2] = ["CHANGELOG.md", "dev/CHANGELOG-DETAILS.md"];

    #[test]
    fn a_clean_tree_has_no_changed_rewrite() {
        for porcelain in ["", "  \n\n", " \n\t\n"] {
            assert_eq!(changed(porcelain, &REWRITTEN), Vec::<&str>::new());
        }
    }

    #[test]
    fn a_rewritten_file_with_changes_is_named() {
        assert_eq!(
            changed(" M CHANGELOG.md\n", &REWRITTEN),
            [" M CHANGELOG.md"]
        );
        assert_eq!(
            changed("?? dev/CHANGELOG-DETAILS.md\n", &REWRITTEN),
            ["?? dev/CHANGELOG-DETAILS.md"]
        );
    }

    #[test]
    fn a_change_to_another_path_is_not_the_runs_business() {
        for porcelain in [
            " M README.md\n?? src/main.rs\n",
            // A file whose name merely ends the same way is not the file itself.
            " M dev/OLD-CHANGELOG-DETAILS.md\n",
        ] {
            assert_eq!(changed(porcelain, &REWRITTEN), Vec::<&str>::new());
        }
    }

    #[test]
    fn a_rename_of_a_rewritten_file_is_a_change() {
        assert_eq!(
            changed(
                "R  dev/CHANGELOG-DETAILS.md -> dev/OLD-DETAILS.md\n",
                &REWRITTEN
            )
            .len(),
            1
        );
    }
}
