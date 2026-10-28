//! Where the editor works: the repository root, and the files it reads and writes there.
//!
//! The root is the repository's, not the nearest workspace's: `dev/changelog` is itself a workspace,
//! and a command run from inside it must still set the version of the tree it belongs to. `git`
//! answers where the repository starts; the manifest at that root is then checked for the workspace
//! section the version lives under.

use std::path::{Path, PathBuf};

use crate::{Error, manifest, tool::Tool};

/// The detailed changelog: the file a person writes and `CHANGELOG.md` is generated from.
///
/// The summary at the repository root links to it by this path, which is why the path is stated
/// once here rather than written out at each use.
pub const DETAILS: &str = "dev/CHANGELOG-DETAILS.md";

/// The generated summary of the details, at the repository root.
const CHANGELOG: &str = "CHANGELOG.md";

/// The draft a person fills in before `write`.
const DRAFT: &str = "__CHANGELOG_DRAFT.md";

/// The copy of the summary taken before a rewrite.
const BACKUP: &str = "__CHANGELOG_BACKUP.md";

/// The summary a rewrite is assembled in before it replaces the file.
const REWRITE: &str = "__CHANGELOG_REWRITE.md";

/// The copy of the details taken before a rewrite.
const DETAILS_BACKUP: &str = "__CHANGELOG_DETAILS_BACKUP.md";

/// The details a rewrite is assembled in before it replaces the file.
const DETAILS_REWRITE: &str = "__CHANGELOG_DETAILS_REWRITE.md";

/// The workspace manifest, relative to the repository root.
const MANIFEST: &str = "Cargo.toml";

/// The repository a run works on.
#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// The repository the program was started in.
    ///
    /// # Errors
    ///
    /// Reports [`Error::NotRepository`] when `git` cannot name a repository root,
    /// [`Error::NoWorkspace`] when the root has no manifest declaring a workspace, and
    /// [`Error::Io`] when the root's manifest cannot be read.
    pub fn discover() -> Result<Self, Error> {
        let cwd = std::env::current_dir().map_err(|error| Error::Io {
            doing: "reading the current directory",
            detail: error.to_string(),
        })?;

        let output = Tool::git().capture([
            std::ffi::OsString::from("-C"),
            cwd.into_os_string(),
            std::ffi::OsString::from("rev-parse"),
            std::ffi::OsString::from("--show-toplevel"),
        ])?;

        if !output.status.success() {
            return Err(Error::NotRepository);
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if root.is_empty() {
            return Err(Error::NotRepository);
        }

        let root = PathBuf::from(root);
        let manifest = root.join(MANIFEST);
        let text = std::fs::read_to_string(&manifest).map_err(|error| Error::Io {
            doing: "reading the workspace manifest",
            detail: format!("{}: {error}", manifest.display()),
        })?;

        if !declares_workspace(&text) {
            return Err(Error::NoWorkspace { path: manifest });
        }

        Ok(Self { root })
    }

    /// The repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The workspace manifest the version is set in.
    #[must_use]
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST)
    }

    /// The generated summary of the details.
    #[must_use]
    pub fn changelog(&self) -> PathBuf {
        self.root.join(CHANGELOG)
    }

    /// The details the summary is generated from.
    #[must_use]
    pub fn details(&self) -> PathBuf {
        self.root.join(DETAILS)
    }

    /// The draft `write-reset` creates and `write` consumes.
    #[must_use]
    pub fn draft(&self) -> PathBuf {
        self.root.join(DRAFT)
    }

    /// The copy of the summary taken before a rewrite.
    #[must_use]
    pub fn backup(&self) -> PathBuf {
        self.root.join(BACKUP)
    }

    /// The summary a rewrite is assembled in.
    #[must_use]
    pub fn rewrite(&self) -> PathBuf {
        self.root.join(REWRITE)
    }

    /// The copy of the details taken before a rewrite.
    #[must_use]
    pub fn details_backup(&self) -> PathBuf {
        self.root.join(DETAILS_BACKUP)
    }

    /// The details a rewrite is assembled in.
    #[must_use]
    pub fn details_rewrite(&self) -> PathBuf {
        self.root.join(DETAILS_REWRITE)
    }

    /// The version the workspace states.
    ///
    /// # Errors
    ///
    /// Reports whatever reading the manifest or reading its version reports.
    pub fn version(&self) -> Result<String, Error> {
        let manifest = self.manifest_path();
        let text = std::fs::read_to_string(&manifest).map_err(|error| Error::Io {
            doing: "reading the workspace manifest",
            detail: format!("{}: {error}", manifest.display()),
        })?;

        manifest::version(&manifest, &text)?.ok_or(Error::ManifestVersionMissing { path: manifest })
    }
}

/// Whether a manifest declares a workspace.
fn declares_workspace(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "[workspace]")
}

#[cfg(test)]
mod tests {
    use super::declares_workspace;

    #[test]
    fn a_workspace_is_declared_by_its_own_table() {
        assert!(declares_workspace("[workspace]\nmembers = []\n"));
        assert!(declares_workspace("  [workspace]  \n"));
        assert!(!declares_workspace(
            "[workspace.package]\nversion = \"0.1.0\"\n"
        ));
        assert!(!declares_workspace("[package]\nname = \"rola\"\n"));
        assert!(!declares_workspace(""));
    }
}
