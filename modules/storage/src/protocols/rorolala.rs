//! Rorolala's own store, and the protocol two of them sync over.
//!
//! A store keeps the objects a Workspace or a Vault handed it and reads them back on request;
//! two stores of the same kind can be pointed at each other to move a batch of them across.
//! That the two ends are the same kind is not something the wire has to be asked about — the
//! type says so — which is why the only thing exchanged before an object moves is
//! [`RorolalaStorage::PROTOCOL_MAGIC`](crate::TransferableBackend::PROTOCOL_MAGIC).
//!
//! What is here is the store itself: the type, where it is made, and the choices it writes with.
//! Everything else is beside it in a file of its own, by what it is about:
//!
//! - [`consts`] — what the layout and the choices are named by.
//! - `paths` — where the things a store keeps sit.
//! - `choice` — how a store decides what to do with the content it is handed.
//! - `config` — what the store is told by the file that makes it a store.
//! - `entry` — reading and writing one entry, and the file-level cares that come with it.
//! - `content` — what a store keeps, and how it decides to keep it.
//! - `pack` — many objects in one file, and the index that says where each one sits.
//! - `backend` — what the store answers a caller and a peer with, the two trait impls.

mod backend;
mod choice;
mod config;
mod consts;
mod content;
mod entry;
mod pack;
mod paths;

#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use rorolala_utils_constants::{LOCK_FILE, STORAGE_CONFIG_PATH};
use rorolala_utils_location::{Locate, LocateHelper};

use crate::{Chunking, Codec, LockError, Lockable, LockingGuard};
use consts::{DEFAULT_CODEC, DEFAULT_CUT, MANIFEST_DIR, OBJECTS_DIR, PACKED_DIR};

/// Rorolala's store of objects, kept under the content addresses they are named by.
///
/// A directory is one once it carries a configuration ([`STORAGE_CONFIG_PATH`]); what lies under
/// that root is the store's own business, and none of it is decided here. A Workspace keeps one
/// inside its data directory and a Vault keeps one under its root, and both hand it back as
/// this.
///
/// # Layout
///
/// ```text
/// <root>/rolast.toml                  what makes the directory a store
/// <root>/obj/<hh>/<hh>/<hex>          loose objects, sharded by the digest
/// <root>/manifest/<hh>/<hh>/<hex>     how a chunked content is put back together
/// <root>/packed/packed_<n>.pack       many entries in one file, of one kind
/// <root>/packed/packed_<n>.idx        where each object sits inside an object pack
/// <root>/manifest/packed_<n>.idx      where each manifest sits inside a manifest pack
/// ```
///
/// A pack holds one kind of entry — objects, or manifests — and a pack number names one pack: the
/// kinds are told apart by where their indexes sit, so an object and a manifest under one key are two
/// entries in two packs, and the manifests can be read without reading every object.
///
/// What an entry holds is not what it *is*: a manifest is storage's own note about how a chunked
/// content was laid out, and nothing here knows a file from anything else.
#[derive(Debug, Clone)]
pub struct RorolalaStorage {
    /// The directory the store is rooted at.
    root: PathBuf,
    /// The cut content is written with once it is big enough to cut.
    cut: Chunking,
    /// The codec content is written with once it is worth compressing.
    codec: Codec,
}

impl Locate for RorolalaStorage {
    /// Finds the store at or above `cwd`.
    ///
    /// The search walks upwards and stops at the first directory carrying a configuration, the
    /// same walk a Vault and a Workspace are found by: the nearest one is the store the caller
    /// is *in*, whatever the directories above it turn out to hold.
    fn locate(cwd: &Path) -> Option<Self> {
        let root = cwd.locate(|dir| dir.join(STORAGE_CONFIG_PATH).exists())?;

        Some(Self::rooted(root))
    }

    fn get_root(&self) -> &Path {
        self.root.as_path()
    }
}

impl Lockable for RorolalaStorage {
    /// The lock sits at the store's root, beside the configuration that makes the directory one.
    ///
    /// A store is one place whether it is reached through a Workspace, a Vault, or on its own, so
    /// there is one lock for it either way — the lock is not per-opener, and a run that holds it
    /// through a Vault holds it against a run that reached the same store directly.
    fn lock_path(&self) -> PathBuf {
        self.root.join(LOCK_FILE)
    }

    async fn lock(&self) -> Result<LockingGuard<Self>, LockError> {
        LockingGuard::acquire(self.clone(), self.lock_path()).await
    }
}

// The store's own layout is written down here, so what makes a directory a store is one thing
// rather than something spread across the entry points below.
impl RorolalaStorage {
    /// The store rooted at `root`, if a configuration is there.
    ///
    /// Nothing is made: this only reads whether the directory is a store already, which is what
    /// a Workspace or a Vault that has one is asked.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Option<Self> {
        let root = root.into();

        root.join(STORAGE_CONFIG_PATH)
            .is_file()
            .then(|| Self::rooted(root))
    }

    /// A store rooted at `root`, writing content the way a store does.
    const fn rooted(root: PathBuf) -> Self {
        Self {
            root,
            cut: DEFAULT_CUT,
            codec: DEFAULT_CODEC,
        }
    }

    /// Makes the store rooted at `root`: the layout, and the configuration that says so.
    ///
    /// ```text
    /// <root>/rolast.toml      the file that makes the directory a store
    /// <root>/obj/             where loose objects go
    /// <root>/manifest/        where the manifests of chunked content go
    /// <root>/packed/          where packs go
    /// ```
    ///
    /// The three directories are made with the store rather than as they fill up, so a store that
    /// exists is one with somewhere to put everything: an empty store is a store with nothing in
    /// it, not one that is half there. What is under them is not guessed at — the sharded
    /// directories under `obj/` and `manifest/` appear as objects are written into them, and the
    /// pack pairs under `packed/` when something is packed.
    ///
    /// What the configuration holds is not decided yet, so what is written is an empty one: what
    /// makes the directory a store is that the file is there at all. Making a store is best
    /// effort — one that could not be made is still handed back, and what failed shows up on the
    /// first read or write that needs it, where there is somewhere to report it.
    #[must_use]
    pub fn create(root: impl Into<PathBuf>) -> Self {
        let root = root.into();

        // The root itself is named by an empty component, so the four are walked as one.
        for directory in ["", OBJECTS_DIR, MANIFEST_DIR, PACKED_DIR] {
            if fs::create_dir_all(root.join(directory)).is_err() {
                return Self::rooted(root);
            }
        }

        let config = root.join(STORAGE_CONFIG_PATH);
        if !config.is_file() {
            let _ = fs::write(config, "");
        }

        Self::rooted(root)
    }

    /// The cut this store writes content with once it is big enough to cut.
    #[must_use]
    pub const fn cut(&self) -> Chunking {
        self.cut
    }

    /// The codec this store writes content with once it is worth compressing.
    #[must_use]
    pub const fn codec(&self) -> Codec {
        self.codec
    }

    /// Writes content with `cut` once it is big enough to cut.
    ///
    /// This is how a store is told to cut one way rather than another: [`Cdc`](crate::Cdc) for
    /// content that is edited, [`Fixed`](crate::Fixed) for content that is written once, and
    /// [`Whole`](crate::Chunking::Whole) to never cut at all.
    #[must_use]
    pub const fn with_cut(mut self, cut: Chunking) -> Self {
        self.cut = cut;

        self
    }

    /// Writes content with `codec` once it is worth compressing.
    #[must_use]
    pub const fn with_codec(mut self, codec: Codec) -> Self {
        self.codec = codec;

        self
    }
}
