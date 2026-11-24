//! Where a project's materials come from.
//!
//! A project keeps a rola store under `.rola/storage/`, and, when `rola.toml` binds more, reads them
//! in the order it binds them: the first one that holds a content is the one it is read from, and
//! what is written goes to the first one that is there. The stores are asked through their own
//! backend rather than by the path a loose object happens to sit at, so content kept cut or
//! compressed is read back as the file it was.
//!
//! A project that keeps no store at all is one whose materials can only be put back from the
//! staging area. `update` is what makes a store here: it brings the content the Vault's versions
//! name, so the commands that come after it find it, see [`crate::update`].

use std::io;
use std::path::Path;

use librorolala::storage::{Bytes, Key, StorageSet};
use rola_define_format::ContentHash;
use rola_git_support::{ContentSource, Error, Project};

use crate::upstream::project_stores;

/// The source a project's materials come from.
pub enum Source {
    /// The stores the project works on, in the order it reaches for them.
    Stores(StorageSet),
    /// Nothing yet: only what is staged can be put back.
    Nothing,
}

/// The source of `project`'s materials.
///
/// # Errors
///
/// Returns a message when `rola.toml` is there and does not read. A configuration that cannot be
/// read is not a project with no store: one is a project to be fixed, the other is a project whose
/// content is somewhere else.
pub fn of(project: &Project) -> Result<Source, String> {
    let stores = project_stores(project)?;

    Ok(if stores.stores().next().is_none() {
        Source::Nothing
    } else {
        Source::Stores(stores)
    })
}

impl ContentSource for Source {
    fn write(&self, content: ContentHash, path: &Path) -> Result<(), Error> {
        match self {
            Self::Stores(stores) => write_from_stores(stores, content, path),
            Self::Nothing => Err(Error::Unavailable {
                content: content.to_token(),
            }),
        }
    }

    fn read(&self, content: ContentHash) -> Result<Bytes, Error> {
        match self {
            Self::Stores(stores) => read_from_stores(stores, content),
            Self::Nothing => Err(Error::Unavailable {
                content: content.to_token(),
            }),
        }
    }
}

/// The bytes the `stores` hold under `content`.
///
/// A tree material's content is its structure, which has to be read whole to say what the material
/// holds. It meets the stores' runtime the same way a placement does, see [`write_from_stores`].
fn read_from_stores(stores: &StorageSet, content: ContentHash) -> Result<Bytes, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|error| Error::Io(io::Error::other(error.to_string())))?;

    let key = Key::new(*content.digest());

    match runtime.block_on(stores.read_object_bytes(&key)) {
        Ok(bytes) => Ok(bytes),
        // A content no store holds is "no source here has it", which is what a caller reports as an
        // unplaced material rather than as a failed read.
        Err(librorolala::storage::Error::NotFound(_)) => Err(Error::Unavailable {
            content: content.to_token(),
        }),
        Err(error) => Err(Error::Io(io::Error::other(error.to_string()))),
    }
}

/// Writes the content `stores` hold under `content` to `path`.
///
/// A store reads asynchronously and a command does not, so the two meet on a runtime of this run's
/// own. It is made per read rather than held: a command places a handful of materials, and a runtime
/// that lives for the length of one read is cheaper than one kept alive for the length of the
/// command.
fn write_from_stores(stores: &StorageSet, content: ContentHash, path: &Path) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|error| Error::Io(io::Error::other(error.to_string())))?;

    let key = Key::new(*content.digest());

    match runtime.block_on(stores.extract_file(&key, path)) {
        Ok(()) => Ok(()),
        // A content no store holds is "no source here has it", which is what a caller reports as an
        // unplaced material rather than as a failed read.
        Err(librorolala::storage::Error::NotFound(_)) => Err(Error::Unavailable {
            content: content.to_token(),
        }),
        Err(error) => Err(Error::Io(io::Error::other(error.to_string()))),
    }
}
