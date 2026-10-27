//! The directory a project is: where `rola.toml` is.

use std::path::{Path, PathBuf};

use rola_define_format::Lock;

use crate::config::ProjectConfig;
use crate::error::Error;
use crate::git::Repository;

/// A project: a directory holding a `rola.toml`.
#[derive(Debug, Clone)]
pub struct Project {
    /// The project's root.
    root: PathBuf,
}

impl Project {
    /// The file a project is found by.
    pub const CONFIG_FILE: &str = "rola.toml";
    /// The lock a project carries, beside the file it is found by.
    pub const LOCK_FILE: &str = "rola.lock";

    /// The project `start` is inside, if it is inside one.
    ///
    /// `start` is read as a directory; a file is read from the directory holding it. The walk
    /// stops at the filesystem root, so a directory outside every project answers with nothing.
    #[must_use]
    pub fn locate(start: &Path) -> Option<Self> {
        let start = if start.is_file() {
            start.parent()?
        } else {
            start
        };

        for directory in start.ancestors() {
            if directory.join(Self::CONFIG_FILE).is_file() {
                return Some(Self {
                    root: directory.to_path_buf(),
                });
            }
        }

        None
    }

    /// The project whose root is `root`, which must hold a `rola.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoProject`] when `root` holds no `rola.toml`.
    pub fn at(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();

        if !root.join(Self::CONFIG_FILE).is_file() {
            return Err(Error::NoProject);
        }

        Ok(Self { root })
    }

    /// The project's root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file the project is described by.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.root.join(Self::CONFIG_FILE)
    }

    /// The lock the project carries.
    #[must_use]
    pub fn lock_path(&self) -> PathBuf {
        self.root.join(Self::LOCK_FILE)
    }

    /// The file the project is described by, read.
    ///
    /// # Errors
    ///
    /// Whatever [`ProjectConfig::read`] fails with, and [`Error::Io`] when there is no file.
    pub fn config(&self) -> Result<ProjectConfig, Error> {
        ProjectConfig::read(&self.config_path())
    }

    /// The lock the project carries, read.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when there is no lock, and [`Error::Format`] when there is one this
    /// build does not read.
    pub fn lock(&self) -> Result<Lock, Error> {
        let text = std::fs::read_to_string(self.lock_path()).map_err(Error::Io)?;

        Lock::decode(&text).map_err(Error::Format)
    }

    /// The git work tree the project is inside, when it is inside one.
    ///
    /// # Errors
    ///
    /// Returns what asking git fails with.
    pub fn repository(&self) -> Result<Option<Repository>, Error> {
        Repository::discover(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Project;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-git-support-project-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_project_is_found_by_walking_up_to_the_file_that_describes_it() {
        let root = scratch("locate");
        let nested = root.join("Assets").join("Characters");

        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join(Project::CONFIG_FILE), b"").unwrap();

        let found = Project::locate(&nested).unwrap();
        assert_eq!(found.root(), root.as_path());
        assert_eq!(found.config_path(), root.join(Project::CONFIG_FILE));
        assert_eq!(found.lock_path(), root.join(Project::LOCK_FILE));

        // A file is read from the directory holding it.
        let file = nested.join("hero.psd");
        std::fs::write(&file, b"content").unwrap();
        assert_eq!(Project::locate(&file).unwrap().root(), root.as_path());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_outside_every_project_is_no_project() {
        let root = scratch("absent");
        let nested = root.join("somewhere");
        std::fs::create_dir_all(&nested).unwrap();

        assert!(Project::locate(&nested).is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_project_at_a_root_needs_the_file_that_describes_it() {
        let root = scratch("at");

        assert!(Project::at(root.join("nothing")).is_err());

        std::fs::write(root.join(Project::CONFIG_FILE), b"").unwrap();
        assert!(Project::at(&root).is_ok());

        let _ = std::fs::remove_dir_all(&root);
    }
}
