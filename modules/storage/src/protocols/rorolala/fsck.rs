//! Whether a store still holds what it names.
//!
//! The check is read-only. It looks at the layout first, which is what a reader would meet as a
//! missing pack or a manifest that does not read, and only reads content when
//! [`FsckOptions::full`](crate::FsckOptions::full) asks it to. What it finds is a
//! [`FsckReport`](crate::FsckReport); repairing anything is a cleanup's job, not this one's.

use std::collections::BTreeSet;

use super::RorolalaStorage;
use crate::{Chunk, Error, FsckOptions, FsckReport, Key, MissingChunks, StorageBackend as _};

impl RorolalaStorage {
    /// Checks that this store still holds what it names.
    ///
    /// A structural check, the one a run gets without asking for more, answers the three ways a
    /// store stops holding together while its files are still there: a pack whose index does not
    /// read, a pack whose index reads and whose file is gone, and a manifest that does not read or
    /// names chunks the store does not hold. It reads no content, so an object whose bytes changed
    /// is found only by a full check.
    ///
    /// A full check reads every key the store names and checks the content against the key, which is
    /// what a read would do. A key already named as a broken manifest or as a content with missing
    /// chunks is not read again: its content cannot be put back together, and reporting the same
    /// content twice says nothing more.
    ///
    /// Packs are asked about by `packs` and `broken_packs` rather than from what a read
    /// remembers, since a check that can be answered from a memory is not one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the store cannot be read at all. That is a store that is failing
    /// rather than one holding a bad file, and it is returned instead of being reported as a finding
    /// because what it says is that the answer is not known.
    pub async fn fsck(&self, options: FsckOptions) -> Result<FsckReport, Error> {
        let mut report = FsckReport::default();

        let packs = self.packs().await?;
        report.checked_packs = packs.len();

        let broken = self.broken_packs().await?;
        for index in &packs {
            if broken.contains(index) {
                continue;
            }

            // A pack is found by its index, and a lookup answers "not found" for a pack whose file
            // is absent, so this is the one place that says the index outlived its pack.
            if !tokio::fs::try_exists(self.pack_path(*index)).await? {
                report.missing_packs.push(*index);
            }
        }
        report.broken_packs = broken;

        let manifests = self.list_manifest_keys().await?;
        report.checked_manifests = manifests.len();

        // The chunks of every manifest are gathered first and asked about in one batch: one lookup
        // per chunk would walk the packs again for each of them.
        let mut chunked: Vec<(Key, Vec<Key>)> = Vec::with_capacity(manifests.len());

        for key in &manifests {
            match self.manifest_of(key).await {
                Ok(Some(manifest)) => {
                    chunked.push((*key, manifest.chunks().iter().map(Chunk::key).collect()));
                }
                // A manifest listed a moment ago and gone now is one nothing can be said about.
                Ok(None) => {}
                Err(Error::Malformed) => report.broken_manifests.push(*key),
                Err(error) => return Err(error),
            }
        }

        let chunks: Vec<Key> = chunked
            .iter()
            .flat_map(|(_, chunks)| chunks.iter().copied())
            .collect();
        let held = self.contains_keys(&chunks).await?;

        let mut at = 0;
        for (manifest, named) in &chunked {
            let missing: Vec<Key> = named
                .iter()
                .enumerate()
                .filter(|(offset, _)| !held.held(at + offset))
                .map(|(_, chunk)| *chunk)
                .collect();

            if !missing.is_empty() {
                report.missing_chunks.push(MissingChunks {
                    manifest: *manifest,
                    chunks: missing,
                });
            }

            at += named.len();
        }

        if options.full {
            // What a manifest already speaks for is not asked about again, see the docs above.
            let spoken: BTreeSet<Key> = report
                .broken_manifests
                .iter()
                .copied()
                .chain(report.missing_chunks.iter().map(|missing| missing.manifest))
                .collect();

            for key in self.list_all_keys().await? {
                if spoken.contains(&key) {
                    continue;
                }

                report.checked_objects += 1;

                match self.read_content(&key).await {
                    Ok(_) => {}
                    Err(Error::NotFound(_)) => report.missing.push(key),
                    Err(Error::Corrupt(_)) => report.corrupt.push(key),
                    Err(Error::Malformed) => report.malformed.push(key),
                    Err(error) => return Err(error),
                }
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::RorolalaStorage;
    use crate::{
        AlgorithmChoice, Chunk, Chunking, Codec, FsckOptions, PackIndex, StorageBackend as _,
    };

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-fsck-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A store of its own, in a directory of its own.
    fn store(label: &str) -> (PathBuf, RorolalaStorage) {
        let parent = scratch(label);
        let storage = RorolalaStorage::create(parent.join("store"));

        (parent, storage)
    }

    /// Content long enough to be cut, with every byte of it distinct enough for a chunk to be one.
    fn content() -> Vec<u8> {
        (0..4096_u32).map(|value| (value % 251) as u8).collect()
    }

    #[tokio::test]
    async fn a_store_that_holds_together_is_clean() {
        let (parent, store) = store("clean");

        let whole = parent.join("whole");
        fs::write(&whole, b"kept whole").unwrap();
        store
            .write_file(&whole, AlgorithmChoice::new(Codec::Raw, Chunking::Whole))
            .await
            .unwrap();

        let cut = parent.join("cut");
        fs::write(&cut, content()).unwrap();
        store
            .write_file(
                &cut,
                AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 512 }),
            )
            .await
            .unwrap();

        let structural = store.fsck(FsckOptions::default()).await.unwrap();
        assert!(structural.is_clean(), "{structural:?}");
        assert!(structural.checked_manifests > 0);
        assert_eq!(structural.checked_objects, 0);

        let full = store.fsck(FsckOptions { full: true }).await.unwrap();
        assert!(full.is_clean(), "{full:?}");
        assert!(full.checked_objects > 0);

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_chunk_that_is_gone_is_named_by_its_manifest() {
        let (parent, store) = store("missing-chunk");

        let file = parent.join("file");
        fs::write(&file, content()).unwrap();
        let key = store
            .write_file(
                &file,
                AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 512 }),
            )
            .await
            .unwrap();

        let manifest = store.manifest_of(&key).await.unwrap().unwrap();
        let chunks: Vec<_> = manifest.chunks().iter().map(Chunk::key).collect();
        assert!(chunks.len() >= 2);

        // A chunk is an object like any other, so dropping it leaves the manifest naming one that
        // is not there.
        store.remove(&chunks[0]).await.unwrap();

        let report = store.fsck(FsckOptions::default()).await.unwrap();
        assert_eq!(report.missing_chunks.len(), 1, "{report:?}");
        assert_eq!(report.missing_chunks[0].manifest, key);
        assert_eq!(report.missing_chunks[0].chunks, vec![chunks[0]]);

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_pack_index_that_does_not_read_is_named() {
        let (parent, store) = store("broken-pack");

        let (pack, index) = store.pack_paths(0);
        assert!(!pack.exists());
        fs::write(&index, b"not an index").unwrap();

        let report = store.fsck(FsckOptions::default()).await.unwrap();
        assert_eq!(report.broken_packs, vec![0], "{report:?}");
        // A pack whose index does not read is not also said to have no pack file.
        assert!(report.missing_packs.is_empty(), "{report:?}");

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_pack_file_that_is_gone_is_named() {
        let (parent, store) = store("missing-pack");

        let (pack, index) = store.pack_paths(0);
        fs::write(&index, PackIndex::new(Vec::new()).encode()).unwrap();
        assert!(!pack.exists());

        let report = store.fsck(FsckOptions::default()).await.unwrap();
        assert!(report.broken_packs.is_empty(), "{report:?}");
        assert_eq!(report.missing_packs, vec![0], "{report:?}");

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn an_object_whose_bytes_changed_is_found_by_a_full_check() {
        let (parent, store) = store("corrupt");

        let key = store
            .write_object(b"the real content", Codec::Raw)
            .await
            .unwrap();
        let path = store.object_path(&key);
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        fs::write(&path, bytes).unwrap();

        // What is stored is still framed and still the length it says, so nothing about the layout
        // is wrong: only reading it back says the content is not the key.
        let structural = store.fsck(FsckOptions::default()).await.unwrap();
        assert!(structural.is_clean(), "{structural:?}");

        let full = store.fsck(FsckOptions { full: true }).await.unwrap();
        assert_eq!(full.corrupt, vec![key], "{full:?}");

        let _ = fs::remove_dir_all(&parent);
    }
}
