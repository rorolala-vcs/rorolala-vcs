//! Where a project's materials come from.
//!
//! A project keeps a rola store under `.rola/storage/`, and the store is where a material's content
//! is read from: it is content-addressed, so the content hash a lock names is the same object the
//! store holds, however the store laid it down. The store is asked through its own backend rather
//! than by the path a loose object happens to sit at, so content kept cut or compressed is read back
//! as the file it was.
//!
//! A project that keeps no store is one whose materials can only be put back from the staging area.
//! `update` is what makes a store here: it brings the content the Vault's versions name, so the
//! commands that come after it find it — see [`crate::update`].

use std::io;
use std::path::Path;

use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use rola_define_format::ContentHash;
use rola_git_support::{ContentSource, Error, Project};

/// The source a project's materials come from.
pub enum Source {
    /// The store the project keeps.
    Store(RorolalaStorage),
    /// Nothing yet: only what is staged can be put back.
    Nothing,
}

/// The source of `project`'s materials.
#[must_use]
pub fn of(project: &Project) -> Source {
    let root = project.root().join(".rola").join("storage");

    RorolalaStorage::at(root).map_or(Source::Nothing, Source::Store)
}

impl ContentSource for Source {
    fn write(&self, content: ContentHash, path: &Path) -> Result<(), Error> {
        match self {
            Self::Store(store) => write_from_store(store, content, path),
            Self::Nothing => Err(Error::Unavailable {
                content: content.to_token(),
            }),
        }
    }
}

/// Writes the content `store` holds under `content` to `path`.
///
/// A store reads asynchronously and a command does not, so the two meet on a runtime of this run's
/// own. It is made per read rather than held: a command places a handful of materials, and a runtime
/// that lives for the length of one read is cheaper than one kept alive for the length of the
/// command.
fn write_from_store(
    store: &RorolalaStorage,
    content: ContentHash,
    path: &Path,
) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|error| Error::Io(io::Error::other(error.to_string())))?;

    let key = Key::new(*content.digest());

    match runtime.block_on(store.extract_file(&key, path)) {
        Ok(()) => Ok(()),
        // A content the store does not hold is "no source here has it", which is what a caller
        // reports as an unplaced material rather than as a failed read.
        Err(librorolala::storage::Error::NotFound(_)) => Err(Error::Unavailable {
            content: content.to_token(),
        }),
        Err(error) => Err(Error::Io(io::Error::other(error.to_string()))),
    }
}
