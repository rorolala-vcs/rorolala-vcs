//! What the index answers a caller and a peer with.
//!
//! The two traits are the boundary a store is reached through — [`StorageBackend`] for a local
//! caller and [`TransferableBackend`] for a peer — and the index implements both, so that the
//! transfer two ends speak is the very one storage's is rather than a second of its own.
//!
//! The key an index object is kept under is the object's **semantic** hash — the hash of what it
//! says, taken through the salt its kind is hashed with — and not the hash of the bytes filed under
//! it. The two differ on purpose: the key is what a variant or a version points at, so it has to be
//! the object's meaning rather than an accident of how that meaning was written down. What is
//! stored is the object's canonical encoding — see
//! [`StorageBackend::write_file`] — so one key still names exactly one byte string, which is what
//! makes reading it back checkable.

use std::path::Path;

use rorolala_storage::{
    AlgorithmChoice, Chunking, Codec, Key, Presence, ProtocolMagic, StorageBackend,
    TransferableBackend,
};

use super::{VCSIndex, VCSIndexObject, VCSWrite};
use crate::error::VCSIndexError;

impl StorageBackend for VCSIndex {
    type Error = VCSIndexError;

    type AlgorithmChoice = AlgorithmChoice;

    /// Chooses how a file is written: as it is, and never cut.
    ///
    /// An index object is small and structured, so there is nothing in it worth compressing and
    /// nothing worth cutting. The choice is still a choice rather than a constant the caller
    /// cannot reach, since it is what the boundary asks for.
    fn choose_algorithm(&self, _file: &Path) -> Self::AlgorithmChoice {
        AlgorithmChoice::new(Codec::Raw, Chunking::Whole)
    }

    async fn write_file(
        &self,
        file: &Path,
        choice: Self::AlgorithmChoice,
    ) -> Result<Key, Self::Error> {
        // The choice says how the file is to be kept, and an index object is only ever kept whole:
        // there is nowhere to record a cutting of one, so a caller asking for one is refused rather
        // than quietly given a whole object and told nothing.
        if choice.chunking() != Chunking::Whole {
            return Err(VCSIndexError::UnsupportedChoice);
        }

        let plain = tokio::fs::read(file).await?;
        let object = VCSIndexObject::decode(&plain)?;
        let key = object.hash();

        // What is written is the encoding of the object that was read, not the bytes the file
        // happened to hold: the key is the object's meaning, so two spellings of one meaning would
        // otherwise be two byte strings under one key. The decoders are strict enough today that the
        // two are the same bytes, and what makes that a property rather than a coincidence is
        // writing the canonical form here.
        self.write_object(&key, &object.encode(), choice.codec())
            .await?;

        Ok(key)
    }

    async fn extract_file(&self, key: &Key, path: &Path) -> Result<(), Self::Error> {
        let plain = self.read_object(key).await?;
        tokio::fs::write(path, &plain).await?;

        Ok(())
    }

    async fn contains_keys(&self, keys: &[Key]) -> Result<Presence, Self::Error> {
        let mut presence = Presence::all_missing(keys.len());

        for (at, key) in keys.iter().enumerate() {
            if self.holds_object(key).await? {
                presence.set_held(at, true);
            }
        }

        Ok(presence)
    }

    async fn list_all_keys(&self) -> Result<Vec<Key>, Self::Error> {
        self.list_object_keys().await
    }

    async fn list_exist_keys(&self) -> Result<Vec<Key>, Self::Error> {
        // Everything listed is a loose file that is there, so being named and being held are the
        // same answer for the index.
        self.list_object_keys().await
    }

    async fn remove(&self, key: &Key) -> Result<(), Self::Error> {
        self.remove_object(key).await
    }
}

impl TransferableBackend for VCSIndex {
    /// The password an index start of a transfer recognises its peer by.
    ///
    /// It is a password of its own rather than the store's, so a store and an index pointed at
    /// each other turn one another away rather than moving what neither of them holds.
    const PROTOCOL_MAGIC: ProtocolMagic = *b"ROLAIXYZ";

    async fn content(&self, key: &Key) -> Result<Option<Vec<u8>>, Self::Error> {
        match self.read_object(key).await {
            Ok(plain) => Ok(Some(plain)),
            Err(VCSIndexError::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn accept(&self, key: &Key, content: Vec<u8>) -> Result<(), Self::Error> {
        // The key is the object's own hash, so what arrived is decoded and hashed back before
        // anything is written: content that does not hash to the key it was sent under is refused
        // rather than stored under a name that would read back wrong.
        let object = VCSIndexObject::decode(&content)?;
        if object.hash() != *key {
            return Err(VCSIndexError::Corrupt(*key));
        }

        self.write_object(key, &content, Codec::Raw).await
    }
}
