//! What a project asks git: where the work tree is, and where its exclusion file is.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::Error;

/// A git work tree a project is inside.
#[derive(Debug, Clone)]
pub struct Repository {
    /// The root of the work tree.
    root: PathBuf,
    /// The directory git keeps its own files in, which may be outside the work tree.
    git_dir: PathBuf,
    /// The directory every work tree of the repository shares.
    common: PathBuf,
}

impl Repository {
    /// The work tree `start` is inside, if it is inside one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Git`] when git answers a question it should have answered, and
    /// [`Error::Io`] when git cannot be run at all.
    pub fn discover(start: &Path) -> Result<Option<Self>, Error> {
        // `--is-inside-work-tree` is the one question a directory outside every repository is
        // meant to answer "no" to, so a failure here is "no repository" rather than a failure.
        let Some(inside) = say(start, &["rev-parse", "--is-inside-work-tree"])? else {
            return Ok(None);
        };

        if inside != "true" {
            return Ok(None);
        }

        let root = PathBuf::from(must_say(start, &["rev-parse", "--show-toplevel"])?);
        let git_dir = PathBuf::from(must_say(start, &["rev-parse", "--absolute-git-dir"])?);
        let common = common_dir(start)?;

        Ok(Some(Self {
            root,
            git_dir,
            common,
        }))
    }

    /// The root of the work tree.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory git keeps its own files in.
    #[must_use]
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// The directory every work tree of the repository shares.
    #[must_use]
    pub fn common_dir(&self) -> &Path {
        &self.common
    }

    /// Whether this is a linked work tree rather than the repository's main work tree.
    ///
    /// git keeps one exclude file for the whole repository and reads it in every work tree, so two
    /// work trees of one project would both manage the same block with different locks. Until a
    /// work tree's materials can be told apart, git support acts in the main work tree only.
    #[must_use]
    pub fn is_linked_work_tree(&self) -> bool {
        !same_directory(&self.common, &self.git_dir)
    }

    /// The path of the repository's `info/exclude`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Git`] when git answers neither way of asking.
    pub fn exclude_path(&self) -> Result<PathBuf, Error> {
        // git 2.31 and later can answer with the whole path. An older git is asked for the common
        // directory instead, which is where the file is for a linked work tree as well, and the
        // answer is joined by hand.
        if let Some(path) = say(
            &self.root,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "info/exclude",
            ],
        )? {
            return Ok(PathBuf::from(path));
        }

        Ok(self.common.join("info").join("exclude"))
    }

    /// `path` as the work tree names it, which is how an exclude pattern begins.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfigValue`] when `path` is outside the work tree.
    pub fn relative(&self, path: &Path) -> Result<PathBuf, Error> {
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        path.strip_prefix(&root)
            .map(Path::to_path_buf)
            .map_err(|_| {
                Error::ConfigValue(format!(
                    "`{}` is outside the work tree at `{}`",
                    path.display(),
                    self.root.display()
                ))
            })
    }
}

/// The directory every work tree of the repository `start` is in shares.
///
/// `--path-format=absolute` is git 2.31 and later; before it the answer is written relative to
/// where the question was asked, which is `start`.
fn common_dir(start: &Path) -> Result<PathBuf, Error> {
    if let Some(path) = say(
        start,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )? {
        return Ok(PathBuf::from(path));
    }

    let common = PathBuf::from(must_say(start, &["rev-parse", "--git-common-dir"])?);

    Ok(if common.is_absolute() {
        common
    } else {
        start.join(common)
    })
}

/// Whether two paths name the same directory.
///
/// The two questions that answer where a work tree sits are free to spell one directory two ways,
/// and Windows names a directory in either case, so the comparison is made on the directories
/// themselves rather than on the text that reached here.
fn same_directory(one: &Path, other: &Path) -> bool {
    let canonical = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let (one, other) = (canonical(one), canonical(other));

    if cfg!(windows) {
        one.to_string_lossy()
            .eq_ignore_ascii_case(&other.to_string_lossy())
    } else {
        one == other
    }
}

/// Asks git `args` in `cwd`, answering with its output when it succeeds and nothing when it does
/// not.
fn say(cwd: &Path, args: &[&str]) -> Result<Option<String>, Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(Error::Io)?;

    if !output.status.success() {
        return Ok(None);
    }

    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}

/// Asks git `args` in `cwd`, answering with its output or with what it said about failing.
fn must_say(cwd: &Path, args: &[&str]) -> Result<String, Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(Error::Io)?;

    if !output.status.success() {
        return Err(Error::Git {
            command: format!("git {}", args.join(" ")),
            reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
