//! What a store keeps, and how it decides to keep it.
//!
//! Content goes down as one object or as a manifest naming chunks, and it is put back together the
//! same way whatever it was when it went in. Which of the two it becomes is this module's one
//! decision, and it decides from the content alone — so a write from a file and a write from a
//! transfer land the same way.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, PackKind, TEXT_CUT, TEXT_CUT_FROM};
use super::entry::{collect, exists, key_of, plain_len_of, verify};
use crate::{
    AlgorithmChoice, Chunk, Chunker as _, Chunking, Codec, Error, FRAME_VERSION, Frame, Key,
    Layout, Manifest, ZIP_MAGIC,
};

impl RorolalaStorage {
    /// Writes `plain` as an object of its own, encoded with `codec`, and answers with its key.
    ///
    /// The key is the hash of `plain`, so writing the same content twice writes the same object
    /// and answers with the same key either time.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the object cannot be written.
    pub async fn write_object(&self, plain: &[u8], codec: Codec) -> Result<Key, Error> {
        let key = key_of(plain);

        // A key is the hash of what is under it, so an object that is already there — loose or in a
        // pack — is the very content in hand. Writing it again would only put a loose copy beside a
        // packed one: the same bytes in two places, which is work packing would have to undo. A write
        // therefore writes what is not there, and nothing else.
        if self.holds_object(&key).await? {
            return Ok(key);
        }

        let frame = Frame {
            version: FRAME_VERSION,
            codec,
            layout: Layout::Single,
            plain_len: plain.len() as u64,
        };

        self.write_entry(&self.object_path(&key), &frame, &codec.encode(plain)?)
            .await?;

        Ok(key)
    }

    /// Reads the object stored under `key`, loose or in a pack, as it was written in.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] if no object is stored under `key`, [`Error::Malformed`] if
    /// what is stored is not framed as this build can read, and [`Error::Corrupt`] if what comes
    /// back does not hash to `key`.
    pub async fn read_object(&self, key: &Key) -> Result<Vec<u8>, Error> {
        let Some(bytes) = self.object_entry(key).await? else {
            return Err(Error::NotFound(*key));
        };
        let (frame, header) = Frame::decode(&bytes)?;

        // An object is the content itself; a manifest is how content was put together, and it is
        // not stored where an object is.
        if frame.layout != Layout::Single {
            return Err(Error::Malformed);
        }

        let payload = bytes.get(header..).ok_or(Error::Malformed)?;
        let content = frame.codec.decode(payload, plain_len_of(&frame)?)?;
        verify(key, &content)?;

        Ok(content)
    }

    /// Writes the manifest of the content stored under `key`, framed as a chunked entry.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the manifest cannot be written.
    pub(crate) async fn write_manifest(
        &self,
        key: &Key,
        manifest: &Manifest,
        codec: Codec,
    ) -> Result<(), Error> {
        // A manifest that is already held — loose or in a manifest pack — says how this content is put
        // back together, and the content under a key is the content the key names, so it is not written
        // again. Writing it again would only put a loose copy beside a packed one. A manifest that is
        // there but does not read is written over: what this leaves behind is one that reads.
        if self.holds_manifest(key).await? {
            return Ok(());
        }

        let plain = manifest.encode();
        let frame = Frame {
            version: FRAME_VERSION,
            codec,
            layout: Layout::Chunked,
            plain_len: plain.len() as u64,
        };

        self.write_entry(&self.manifest_path(key), &frame, &codec.encode(&plain)?)
            .await
    }

    /// Reads the manifest of the content stored under `key`, if it was stored chunked.
    ///
    /// A manifest is read loose where it is loose and out of a manifest pack where it was packed, and
    /// `None` is a content that was stored as one object rather than a manifest, which is not a failure:
    /// how it was cut is not something the key says.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if what is stored is not framed as this build can read.
    pub(crate) async fn read_manifest(&self, key: &Key) -> Result<Option<Manifest>, Error> {
        let Some(bytes) = self.manifest_entry(key).await? else {
            return Ok(None);
        };
        let (frame, header) = Frame::decode(&bytes)?;

        if frame.layout != Layout::Chunked {
            return Err(Error::Malformed);
        }

        let payload = bytes.get(header..).ok_or(Error::Malformed)?;
        let plain = frame.codec.decode(payload, plain_len_of(&frame)?)?;

        Ok(Some(Manifest::decode(&plain)?))
    }

    /// The content stored under `key`, put back together however it was laid down.
    ///
    /// A manifest is what a cut content has and an object is what a whole one has, and both may be
    /// there at once — a chunk is an object, and a chunk's bytes may be the very content some key
    /// names. The manifest is read first, so which of the two answers is never left to chance; but a
    /// manifest that does not *read* does not decide anything either, and the object under the same
    /// key is the same content, so it is answered with rather than the key being lost to a stray
    /// file beside it.
    pub(super) async fn read_content(&self, key: &Key) -> Result<Vec<u8>, Error> {
        let manifest = match self.read_manifest(key).await {
            Ok(Some(manifest)) => manifest,
            // No manifest at all: the content is whatever is under the key as an object.
            Ok(None) => return self.read_object(key).await,
            // A manifest that will not read is one nothing can be put back together from; the object
            // under the key is the same content, so it is answered with. If none is there, the
            // manifest's failure is what is reported rather than a not-found that hides it.
            Err(Error::Malformed) => {
                return self.read_object(key).await.map_err(|_| Error::Malformed);
            }
            Err(error) => return Err(error),
        };

        match self.assemble(&manifest).await {
            Ok(content) => Ok(content),
            // The chunks cannot be put back together — one is gone, or longer than the manifest says.
            // The whole content may still be under the key, beside the manifest (see `write_content`),
            // and a key is the hash of the whole content, so what is there is the same content the
            // manifest would have made. If it is not there, what went wrong with the manifest is what
            // is reported.
            Err(error) => self.read_object(key).await.map_err(|_| error),
        }
    }

    /// The content a manifest names, put back together from the objects it names.
    ///
    /// What the manifest says each chunk should be long is checked against what comes back: a
    /// manifest is the only place that is written down, and one that disagrees with the objects it
    /// names is a store that does not hold together rather than content to be handed on.
    async fn assemble(&self, manifest: &Manifest) -> Result<Vec<u8>, Error> {
        let mut content = Vec::with_capacity(usize::try_from(manifest.content_len()).unwrap_or(0));
        for chunk in manifest.chunks() {
            let piece = self.read_object(&chunk.key()).await?;
            if piece.len() as u64 != chunk.len() {
                return Err(Error::Malformed);
            }

            content.extend_from_slice(&piece);
        }

        Ok(content)
    }

    /// Chooses how `content` is written, from what its opening says.
    ///
    /// This is the one place a store decides what to do with content, and it decides from the
    /// content alone — so a write from a file and a write from a transfer land the same way. See
    /// [`choose_algorithm`](crate::StorageBackend::choose_algorithm) for what each answer means.
    pub(super) fn choose_for(&self, magic: &[u8], size: usize) -> AlgorithmChoice {
        if starts_a_container(magic) {
            return AlgorithmChoice::new(Codec::Raw, Chunking::Zip);
        }

        if is_text(magic) {
            let chunking = if size < TEXT_CUT_FROM {
                Chunking::Whole
            } else {
                TEXT_CUT
            };

            return AlgorithmChoice::new(Codec::Zstd, chunking);
        }

        AlgorithmChoice::new(self.codec, self.cut)
    }

    /// Stores `content`, written with `choice`, and answers with its key.
    ///
    /// This is a write with the file already in hand: what a caller has is bytes rather than a path
    /// to them, which is what a transfer has.
    pub(super) async fn write_content(
        &self,
        content: &[u8],
        choice: AlgorithmChoice,
    ) -> Result<Key, Error> {
        let key = key_of(content);
        let ranges = choice.chunking().split(content);

        // One chunk is not a cutting: it is the content itself, laid down as a single object.
        if ranges.len() < 2 {
            self.write_object(content, choice.codec()).await?;
            // The object is the entry now, so a manifest that put the content together another way
            // is not. A manifest is safe to drop where an object is not: nothing ever refers to
            // one, while an object may be a chunk some other manifest is speaking for.
            self.drop_entry(&self.manifest_path(&key)).await?;

            return Ok(key);
        }

        let mut manifest = Manifest::default();
        for range in ranges {
            let chunk = &content[range];
            let chunk_key = self.write_object(chunk, choice.codec()).await?;

            manifest.push(Chunk::new(chunk_key, chunk.len() as u64));
        }

        // The object a whole write would have left is deliberately left where it is: it may be a
        // chunk another manifest is speaking for, and a chunk is an object like any other.
        self.write_manifest(&key, &manifest, choice.codec()).await?;

        Ok(key)
    }

    /// Whether the content stored under `key` can be put back together.
    ///
    /// A cut content is held when its chunks are, not when a file with its name is: what a key
    /// promises is the content, and the content is the chunks. A manifest that does not read, or one
    /// whose chunks are not all there, does not decide anything either, so what the key promises
    /// rests on the object beside it — the same answer a read gives, and the same way round.
    pub(super) async fn holds_content(&self, key: &Key) -> Result<bool, Error> {
        let manifest = match self.read_manifest(key).await {
            Ok(manifest) => manifest,
            Err(Error::Malformed) => None,
            Err(error) => return Err(error),
        };

        let Some(manifest) = manifest else {
            return self.holds_object(key).await;
        };

        for chunk in manifest.chunks() {
            if !self.holds_object(&chunk.key()).await? {
                return self.holds_object(key).await;
            }
        }

        Ok(true)
    }

    /// Whether an object is stored under `key`, loose or in a pack.
    pub(super) async fn holds_object(&self, key: &Key) -> Result<bool, Error> {
        if exists(&self.object_path(key)).await? {
            return Ok(true);
        }

        Ok(self.find_packed(key).await?.is_some())
    }

    /// Whether the content stored under `key` is kept as a manifest of chunks.
    ///
    /// A manifest is what a cut content has and an object is what a whole one has, and the two are
    /// stored apart — see [`Manifest`] — so this says which of the two a key is kept as. A manifest
    /// that is there but does not read is not one anything can be put back together from, so it is
    /// answered as an object, the same way round a read takes the two.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the manifest cannot be looked for.
    pub async fn holds_manifest(&self, key: &Key) -> Result<bool, Error> {
        match self.read_manifest(key).await {
            Ok(manifest) => Ok(manifest.is_some()),
            Err(Error::Malformed) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Every key the store keeps a manifest for, and nothing else.
    ///
    /// A manifest is kept apart from the objects — see [`Manifest`] — so what is here is the whole of
    /// what the store was told to cut, whether each manifest is loose or in a pack, and nothing of what
    /// it was told to keep whole. A manifest whose chunks have gone is still listed: it is still what
    /// the store was told to keep, and what is missing is found on the read that asks for it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the manifests cannot be listed.
    pub async fn list_manifest_keys(&self) -> Result<Vec<Key>, Error> {
        let mut keys = BTreeSet::new();
        collect(&self.root.join(MANIFEST_DIR), &mut keys).await?;

        let state = self.root_state();
        for index in self.packs_of(PackKind::Manifest).await? {
            let Some(directory) = self.pack_index(&state, PackKind::Manifest, index).await? else {
                continue;
            };

            for entry in directory.entries() {
                keys.insert(entry.key());
            }
        }

        Ok(keys.into_iter().collect())
    }
}

/// Whether `magic` — the first bytes of some content — starts a signature this store recognises.
///
/// The only one it recognises is a ZIP's, which is what tells a packed container from content of
/// any other kind — see [`Zip`](crate::Zip) for what a container is taken apart by.
pub(super) fn starts_a_container(magic: &[u8]) -> bool {
    magic.starts_with(&ZIP_MAGIC)
}

/// Whether `magic` — the first bytes of some content — reads as text.
///
/// A zero byte is what says content is not text, which is the rule `git` reads a file by: what a
/// file is *for* cannot be told from its bytes, but whether it is something a person wrote can be
/// told well enough by that much of it.
fn is_text(magic: &[u8]) -> bool {
    !magic.contains(&0)
}

/// Fills `magic` with the first bytes of `file`, and says how many of them were there.
pub(super) fn read_magic(file: &Path, magic: &mut [u8]) -> usize {
    use std::io::Read as _;

    let Ok(mut handle) = fs::File::open(file) else {
        return 0;
    };

    let mut filled = 0;
    while filled < magic.len() {
        match handle.read(&mut magic[filled..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => filled += read,
        }
    }

    filled
}
