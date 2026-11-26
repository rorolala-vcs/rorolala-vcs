//! A store's own layout, for the tests and tools that have to see it.
//!
//! A caller hands over content and keys, and *where* the store puts them, which object sits at which
//! path, how a cut content is laid down, where a pack's index sits, is the store's own business.
//! What is here is that business, and it is here at all because a test that checks the placing has to
//! look at it. It is not the boundary: what a caller wants is [`StorageBackend`](crate::StorageBackend)
//! and [`TransferableBackend`](crate::TransferableBackend).

use std::future::Future;
use std::path::PathBuf;

use crate::{Codec, Error, Key, Manifest, PackIndex, RorolalaStorage};

/// Where [`RorolalaStorage`] puts the things it keeps, and how it lays them down.
///
/// Every method is the store's own: it answers where something landed or how it is kept, rather than
/// anything a caller of storage asks for. See the [module docs](self).
pub trait Internals {
    /// The file the store's configuration is kept in, which is what it is found by.
    #[must_use]
    fn config_path(&self) -> PathBuf;

    /// Where the object stored under `key` sits, once it is loose.
    #[must_use]
    fn object_path(&self, key: &Key) -> PathBuf;

    /// Where the manifest of the content stored under `key` sits.
    #[must_use]
    fn manifest_path(&self, key: &Key) -> PathBuf;

    /// Where the packed file of `index` sits, and where its index sits beside it.
    #[must_use]
    fn pack_paths(&self, index: u64) -> (PathBuf, PathBuf);

    /// The packs the store holds, lowest index first.
    fn packs(&self) -> impl Future<Output = Result<Vec<u64>, Error>> + Send;

    /// The packs whose index does not read.
    fn broken_packs(&self) -> impl Future<Output = Result<Vec<u64>, Error>> + Send;

    /// The directory of the pack of `index`.
    fn read_pack_index(&self, index: u64) -> impl Future<Output = Result<PackIndex, Error>> + Send;

    /// Writes the manifest of the content stored under `key`.
    fn write_manifest(
        &self,
        key: &Key,
        manifest: &Manifest,
        codec: Codec,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Reads the manifest of the content stored under `key`, if it was stored chunked.
    fn read_manifest(
        &self,
        key: &Key,
    ) -> impl Future<Output = Result<Option<Manifest>, Error>> + Send;
}

impl Internals for RorolalaStorage {
    fn config_path(&self) -> PathBuf {
        Self::config_path(self)
    }

    fn object_path(&self, key: &Key) -> PathBuf {
        Self::object_path(self, key)
    }

    fn manifest_path(&self, key: &Key) -> PathBuf {
        Self::manifest_path(self, key)
    }

    fn pack_paths(&self, index: u64) -> (PathBuf, PathBuf) {
        Self::pack_paths(self, index)
    }

    async fn packs(&self) -> Result<Vec<u64>, Error> {
        Self::packs(self).await
    }

    async fn broken_packs(&self) -> Result<Vec<u64>, Error> {
        Self::broken_packs(self).await
    }

    async fn read_pack_index(&self, index: u64) -> Result<PackIndex, Error> {
        Self::read_pack_index(self, index).await
    }

    async fn write_manifest(
        &self,
        key: &Key,
        manifest: &Manifest,
        codec: Codec,
    ) -> Result<(), Error> {
        Self::write_manifest(self, key, manifest, codec).await
    }

    async fn read_manifest(&self, key: &Key) -> Result<Option<Manifest>, Error> {
        Self::read_manifest(self, key).await
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{Codec, Key, Manifest, RorolalaStorage};

    use super::Internals;

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rola-storage-internals-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        dir
    }

    #[test]
    fn a_store_answers_where_it_keeps_what_it_holds() {
        let dir = scratch("layout");
        let store = RorolalaStorage::create(dir.clone());
        let key = Key::new([1; 32]);

        assert!(Internals::config_path(&store).starts_with(&dir));
        assert!(Internals::object_path(&store, &key).starts_with(&dir));
        assert!(Internals::manifest_path(&store, &key).starts_with(&dir));
        let (packed, index) = Internals::pack_paths(&store, 0);
        assert!(packed.starts_with(&dir));
        assert!(index.starts_with(&dir));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            runtime.block_on(Internals::packs(&store)).unwrap(),
            Vec::<u64>::new()
        );
        assert_eq!(
            runtime.block_on(Internals::broken_packs(&store)).unwrap(),
            Vec::<u64>::new()
        );
        let _ = runtime.block_on(Internals::read_pack_index(&store, 0));
        let _ = runtime.block_on(Internals::read_manifest(&store, &key));
        let _ = runtime.block_on(Internals::write_manifest(
            &store,
            &key,
            &Manifest::new(Vec::new()),
            Codec::Raw,
        ));

        let _ = std::fs::remove_dir_all(dir);
    }
}
