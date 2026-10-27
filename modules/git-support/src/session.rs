//! One project's materials and the git work tree they are inside, kept in step.

use std::path::Path;

use rola_define_format::{Lock, LockPath};

use crate::content::ContentSource;
use crate::error::Error;
use crate::exclude;
use crate::git::Repository;
use crate::project::Project;
use crate::staging::{self, Private, State};

/// A project's materials and the git work tree they are inside.
///
/// A session is read from disk rather than held open: every method reads the lock or the state it
/// needs, so a run takes the files as they are when it acts rather than as they were when it
/// started.
#[derive(Debug, Clone)]
pub struct Session {
    /// The project the run is inside.
    project: Project,
    /// The git work tree the project is inside.
    repository: Repository,
    /// Where the plugin keeps what is not in the work.
    private: Private,
}

impl Session {
    /// The project `start` is inside, if it is inside one and that project uses git support.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Disabled`] when the project does not use git support, [`Error::NoRepository`]
    /// when it is inside no git work tree, and what reading the project's file fails with.
    pub fn open(start: &Path) -> Result<Option<Self>, Error> {
        let Some(project) = Project::locate(start) else {
            return Ok(None);
        };

        if !project.config()?.git_support() {
            return Err(Error::Disabled);
        }

        let Some(repository) = project.repository()? else {
            return Err(Error::NoRepository);
        };

        // The exclude file is the repository's one and git reads it in every work tree of it, so a
        // linked work tree's materials would fight the main work tree's for the block that keeps
        // them out of git. Until a work tree's materials can be kept apart, the run is refused
        // rather than allowed to make one work tree's materials visible to git in the other.
        if repository.is_linked_work_tree() {
            return Err(Error::LinkedWorkTree);
        }

        let private = Private::of(project.root());

        Ok(Some(Self {
            project,
            repository,
            private,
        }))
    }

    /// The project the run is inside.
    #[must_use]
    pub const fn project(&self) -> &Project {
        &self.project
    }

    /// The git work tree the project is inside.
    #[must_use]
    pub const fn repository(&self) -> &Repository {
        &self.repository
    }

    /// The lock the project carries, read.
    ///
    /// # Errors
    ///
    /// Returns what [`Project::lock`] fails with.
    pub fn lock(&self) -> Result<Lock, Error> {
        self.project.lock()
    }

    /// What the plugin last put into the working tree.
    ///
    /// # Errors
    ///
    /// Returns what [`State::read`] fails with.
    pub fn state(&self) -> Result<State, Error> {
        State::read(&self.private.state_path())
    }

    /// The project as the work tree names it, which is how its exclude block is told apart.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfigValue`] when the project is outside the work tree.
    pub fn name(&self) -> Result<String, Error> {
        Ok(exclude::name(
            &self.repository.relative(self.project.root())?,
        ))
    }

    /// Rewrites the project's block of the repository's exclude file from `lock`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the exclude file cannot be written.
    pub fn exclude(&self, lock: &Lock) -> Result<(), Error> {
        exclude::rewrite(&self.repository.exclude_path()?, &self.name()?, lock)
    }

    /// Takes every material last attached out of the working tree.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Content`] when a material was changed after it was attached, and what
    /// moving or recording fails with.
    pub fn detach(&self) -> Result<(), Error> {
        let mut state = self.state()?;

        staging::detach(&self.private, self.project.root(), &mut state)
    }

    /// Puts `lock`'s materials into the working tree, recording only what was placed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the state cannot be written.
    pub fn attach(
        &self,
        lock: &Lock,
        source: &dyn ContentSource,
    ) -> Result<Vec<(String, Error)>, Error> {
        let mut state = self.state()?;

        staging::attach(&self.private, self.project.root(), lock, source, &mut state)
    }

    /// Puts `only` of `lock`'s materials back, replacing whatever is there.
    ///
    /// This is the run that throws a change away: a material changed in the working tree is removed
    /// and placed again from the lock, which is what `git restore` means.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the state cannot be written.
    pub fn restore(
        &self,
        lock: &Lock,
        only: &[LockPath],
        source: &dyn ContentSource,
    ) -> Result<Vec<(String, Error)>, Error> {
        let mut state = self.state()?;
        let failures = staging::restore(
            &self.private,
            self.project.root(),
            lock,
            only,
            source,
            &mut state,
        )?;

        state.write(&self.private.state_path())?;

        Ok(failures)
    }

    /// Detaches what was attached, attaches `lock`, and keeps the exclusions in step.
    ///
    /// # Errors
    ///
    /// Returns what [`detach`](Self::detach) or [`attach`](Self::attach) fails with.
    pub fn settle(
        &self,
        lock: &Lock,
        source: &dyn ContentSource,
    ) -> Result<Vec<(String, Error)>, Error> {
        self.detach()?;
        let failures = self.attach(lock, source)?;
        self.exclude(lock)?;

        Ok(failures)
    }
}
