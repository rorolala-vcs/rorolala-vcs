//! What a store answers a caller and a peer with.
//!
//! The two traits are the boundary — [`StorageBackend`] for a local caller and
//! [`TransferableBackend`] for a peer — and this is where the store keeps them, so that what a
//! caller sees is one short file rather than the working of the store behind it.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, OBJECTS_DIR, PackKind, SNIFF_LEN};
use super::content::read_magic;
use super::entry::{collect, exists, verify};
use crate::{
    AlgorithmChoice, Error, Key, Presence, ProtocolMagic, StorageBackend, TransferableBackend,
};

impl StorageBackend for RorolalaStorage {
    type Error = Error;

    type AlgorithmChoice = AlgorithmChoice;

    /// Chooses how `file` is written, from what the file in front of it says.
    ///
    /// Three answers, and none of them needs to read the whole content to give one:
    ///
    /// - **A packed container** — anything that starts with a ZIP's signature, which is what a
    ///   `.zip` and every format built on one are — is cut at the boundaries of what it packs. See
    ///   [`Chunking::Zip`](crate::Chunking::Zip): boundaries given rather than searched for, and the
    ///   cut that can tell what is still the same bytes it was.
    /// - **Text** is compressed, and — once there is enough of it to be worth a manifest — cut at
    ///   the ends of its lines, so that what a chunk holds is whole lines. See [`Text`](crate::Text):
    ///   lines are what an edit moves, so a few of them changed costs those chunks rather than the
    ///   file.
    /// - **Anything else** is written the way the store was told to write, which by default is as
    ///   it came in, whole: compressing arbitrary content is not the bargain it is for text, and
    ///   cutting it takes a manifest to say where the pieces are.
    ///
    /// What any answer comes to is not written down anywhere, so a store may be told to write
    /// differently tomorrow without a key changing.
    fn choose_algorithm(&self, file: &Path) -> Self::AlgorithmChoice {
        let mut magic = [0_u8; SNIFF_LEN];
        let filled = read_magic(file, &mut magic);
        let size = fs::metadata(file).map_or(0, |data| data.len());

        self.choose_for(
            &magic[..filled],
            usize::try_from(size).unwrap_or(usize::MAX),
        )
    }

    async fn write_file(
        &self,
        file: &Path,
        choice: Self::AlgorithmChoice,
    ) -> Result<Key, Self::Error> {
        let content = tokio::fs::read(file).await?;

        self.write_content(&content, choice).await
    }

    async fn extract_file(&self, key: &Key, path: &Path) -> Result<(), Self::Error> {
        let content = self.read_content(key).await?;
        verify(key, &content)?;

        tokio::fs::write(path, &content).await?;

        Ok(())
    }

    async fn contains_keys(&self, keys: &[Key]) -> Result<Presence, Self::Error> {
        let mut presence = Presence::all_missing(keys.len());

        for (at, key) in keys.iter().enumerate() {
            if self.holds_content(key).await? {
                presence.set_held(at, true);
            }
        }

        Ok(presence)
    }

    async fn list_all_keys(&self) -> Result<Vec<Key>, Self::Error> {
        let mut keys = BTreeSet::new();

        for directory in [OBJECTS_DIR, MANIFEST_DIR] {
            collect(&self.root.join(directory), &mut keys).await?;
        }

        // What a pack holds is still held, and a caller listing a store wants every key in it
        // whichever way the content happens to be kept. A manifest pack names keys of its own, so
        // both kinds are asked.
        let state = self.root_state();
        for kind in [PackKind::Object, PackKind::Manifest] {
            for index in self.packs_of(kind).await? {
                let Some(directory) = self.pack_index(&state, kind, index).await? else {
                    continue;
                };

                for entry in directory.entries() {
                    keys.insert(entry.key());
                }
            }
        }

        Ok(keys.into_iter().collect())
    }

    async fn list_exist_keys(&self) -> Result<Vec<Key>, Self::Error> {
        let mut exist = Vec::new();

        for key in self.list_all_keys().await? {
            if self.holds_content(&key).await? {
                exist.push(key);
            }
        }

        Ok(exist)
    }

    async fn remove(&self, key: &Key) -> Result<(), Self::Error> {
        // Dropping from a pack changes its index in place, which is what the lock is for — see
        // `root_state`.
        let state = self.root_state();
        let _packing = state.packing.lock().await;

        // What is dropped is what the key names, and nothing is asked about who else might name it:
        // a chunk is an object like any other, so a key another manifest is speaking for would be
        // dropped out from under it. Storage keeps no account of that — finding what is still
        // spoken for is a cleanup's to do — so removing a key that is still wanted is the caller's
        // to know better than.
        //
        // A key may name an object, a manifest, or both, and the two are two entries in two kinds of
        // pack, so both kinds are asked and both loose files are dropped.
        let mut loose = Vec::new();
        for kind in [PackKind::Object, PackKind::Manifest] {
            let path = self.entry_path(kind, key);

            if exists(&path).await? {
                loose.push(path);
            }
        }

        // Every pack that names the key is asked, not just the first: nothing keeps an entry out of
        // two packs, and one left behind would be a key `remove` said it had taken away and a read
        // would still answer with.
        let mut packed = Vec::new();
        for kind in [PackKind::Object, PackKind::Manifest] {
            for index in self.packs_of(kind).await? {
                if self
                    .pack_index(&state, kind, index)
                    .await?
                    .is_some_and(|directory| directory.find(key).is_some())
                {
                    packed.push((kind, index));
                }
            }
        }

        if loose.is_empty() && packed.is_empty() {
            return Err(Error::NotFound(*key));
        }

        for path in loose {
            self.drop_entry(&path).await?;
        }

        for (kind, index) in packed {
            self.remove_from_pack(&state, kind, index, key).await?;
        }

        Ok(())
    }
}

impl TransferableBackend for RorolalaStorage {
    const PROTOCOL_MAGIC: ProtocolMagic = *b"ROLASTOR";

    /// The content stored under `key`, or `None` when there is none.
    ///
    /// Content kept as chunks comes back as the whole of it: what a transfer moves is content, and
    /// how this store keeps it is not something the other end is told.
    async fn content(&self, key: &Key) -> Result<Option<Vec<u8>>, Self::Error> {
        match self.read_content(key).await {
            Ok(content) => Ok(Some(content)),
            Err(Error::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Stores `content` under `key`, the way this store writes anything.
    ///
    /// The key is checked against the content before anything is written, so content that does not
    /// hash to what it was sent under is refused rather than stored under a name that would read
    /// back wrong.
    async fn accept(&self, key: &Key, content: Vec<u8>) -> Result<(), Self::Error> {
        verify(key, &content)?;

        let magic = content.get(..SNIFF_LEN).unwrap_or(&content);
        let choice = self.choose_for(magic, content.len());
        self.write_content(&content, choice).await?;

        Ok(())
    }
}
