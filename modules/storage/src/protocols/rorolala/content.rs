//! What a store keeps, and how it decides to keep it.
//!
//! Content goes down as one object or as a manifest naming chunks, and it is put back together the
//! same way whatever it was when it went in. Which of the two it becomes is settled by
//! [`choice`](super::choice) and carried in as an [`AlgorithmChoice`], so what is here is the laying
//! down of content that has already been decided about.

use std::collections::BTreeSet;

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, PackKind};
use super::entry::{collect, exists, key_of, plain_len_of, read_range, verify};
use crate::{
    AlgorithmChoice, Chunk, Chunker as _, Codec, Error, FRAME_HEADER_LEN, FRAME_VERSION, Frame,
    Key, Layout, Manifest,
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
    ///
    /// The content a manifest makes is checked against the key before it is handed over: a manifest
    /// is not covered by the key, so it may name the right chunks in the wrong order — or chunks of
    /// something else — and what a key promises is the content, not the reading of a manifest.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] if no manifest and no object is stored under `key`,
    /// [`Error::Corrupt`] if what is stored does not make the content the key names,
    /// [`Error::Malformed`] if a manifest will not read and there is no object to answer with, and
    /// [`Error::Io`] if the store cannot be read at all.
    pub(super) async fn read_content(&self, key: &Key) -> Result<Vec<u8>, Error> {
        let manifest = match self.read_manifest(key).await {
            Ok(Some(manifest)) => manifest,
            // No manifest at all: the content is whatever is under the key as an object.
            Ok(None) => return self.read_object(key).await,
            // A manifest that will not read is one nothing can be put back together from; the object
            // under the key is the same content, so it is answered with. What the fallback fails
            // with decides what is reported: only a key with nothing under it leaves the manifest's
            // own failure as the account, while a store that cannot be read and an object that does
            // not hash are both more telling than "malformed".
            Err(Error::Malformed) => return self.whole_or_malformed(key).await,
            Err(error) => return Err(error),
        };

        match self.assemble(&manifest).await {
            // Every chunk is whole in itself — `read_object` checked that — but the manifest that
            // put them in order is not covered by the key, so the whole is checked on its own. A
            // per-chunk check that passed is not the check a key promises.
            Ok(content) if verify(key, &content).is_ok() => Ok(content),
            Ok(_) => self.whole_or_corrupt(key).await,
            // The chunks cannot be put back together — one is gone, or longer than the manifest says.
            // The whole content may still be under the key, beside the manifest (see `write_content`),
            // and a key is the hash of the whole content, so what is there is the same content the
            // manifest would have made. If it is not there, what went wrong with the manifest is what
            // is reported — unless the fallback itself failed in a way that says more: a store that
            // cannot be read is not content that does not add up, and an object that does not hash is
            // not a manifest that reads wrong.
            Err(error) => match self.read_object(key).await {
                Ok(content) => Ok(content),
                Err(Error::Io(failure)) => Err(Error::Io(failure)),
                Err(Error::Corrupt(corrupt)) => Err(Error::Corrupt(corrupt)),
                Err(_) => Err(error),
            },
        }
    }

    /// The object stored under `key`, or the malformed manifest there is nothing else to report.
    ///
    /// This is what a manifest that will not read is answered with. The object beside it is the same
    /// content, so a read that works answers the key; a read that fails for its own reason is the
    /// better account of the store — an unreadable disk, or an object that does not hash — and only
    /// a key with no object under it leaves the manifest's failure as what is reported.
    async fn whole_or_malformed(&self, key: &Key) -> Result<Vec<u8>, Error> {
        match self.read_object(key).await {
            Ok(content) => Ok(content),
            Err(Error::NotFound(_)) => Err(Error::Malformed),
            Err(error) => Err(error),
        }
    }

    /// The object stored under `key`, or a corruption of it when no such object is there.
    ///
    /// This is what a manifest that does not make the content the key names is answered with: the
    /// content may still be under the key as one object, and a whole write leaves it there beside a
    /// manifest — see `write_content`. If it is not, what the key promises is not in the store, and
    /// the key is corrupt rather than missing: a manifest for it is there, and says it was stored.
    async fn whole_or_corrupt(&self, key: &Key) -> Result<Vec<u8>, Error> {
        match self.read_object(key).await {
            Ok(content) => Ok(content),
            // The fallback's own failure is the more telling one for an I/O problem: the store is
            // failing rather than holding content that does not add up.
            Err(Error::Io(error)) => Err(Error::Io(error)),
            Err(_) => Err(Error::Corrupt(*key)),
        }
    }

    /// The content a manifest names, put back together from the objects it names.
    ///
    /// What the manifest says each chunk should be long is checked against what comes back: a
    /// manifest is the only place that is written down, and one that disagrees with the objects it
    /// names is a store that does not hold together rather than content to be handed on.
    async fn assemble(&self, manifest: &Manifest) -> Result<Vec<u8>, Error> {
        // Nothing is reserved from the total the manifest claims: a length is the manifest's to say
        // and this is not a number an allocator should be handed on trust. A manifest that reads is
        // bounded by `MAX_CONTENT_LEN` — see `Manifest::decode` — and what grows the buffer here is
        // the bytes that really came back, one chunk at a time.
        let mut content = Vec::new();
        for chunk in manifest.chunks() {
            let piece = self.read_object(&chunk.key()).await?;
            if piece.len() as u64 != chunk.len() {
                return Err(Error::Malformed);
            }

            content.extend_from_slice(&piece);
        }

        Ok(content)
    }

    /// How many bytes the content stored under `key` is carried to another store as, read without
    /// taking the content apart.
    ///
    /// A content kept as chunks is carried as its manifest and then the chunks it names, whose
    /// lengths the manifest already says; one kept whole is carried as the object itself, whose
    /// length the frame in front of it records. Neither path reads the content, so a caller that
    /// only wants to say how much is going to move does not pay for the moving.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] if nothing is stored under `key`, [`Error::Malformed`] if what
    /// is stored is not framed as this build can read, and [`Error::Io`] if it cannot be read.
    pub async fn carried_len(&self, key: &Key) -> Result<u64, Error> {
        if let Some(manifest) = self.read_manifest(key).await? {
            // The manifest and the content are carried one after the other, so the sum is checked
            // rather than left to wrap: a manifest read here has lengths that add up, but what is
            // being added to them is the manifest's own encoding, which is on top of that.
            let content = manifest.checked_content_len().ok_or(Error::Malformed)?;
            let encoded = u64::try_from(manifest.encode().len()).map_err(|_| Error::Malformed)?;

            return content.checked_add(encoded).ok_or(Error::Malformed);
        }

        let Some(head) = self.entry_head(PackKind::Object, key).await? else {
            return Err(Error::NotFound(*key));
        };
        let (frame, _) = Frame::decode(&head)?;

        Ok(frame.plain_len)
    }

    /// The front of the entry of `kind` stored under `key`: enough of its frame to read its length.
    ///
    /// The entry is read loose where it is loose and out of a pack where it was packed, and only as
    /// much of it as a frame takes — see [`FRAME_HEADER_LEN`] — so an object too small to hold one is
    /// the whole of what is read.
    async fn entry_head(&self, kind: PackKind, key: &Key) -> Result<Option<Vec<u8>>, Error> {
        let path = self.entry_path(kind, key);

        match tokio::fs::metadata(&path).await {
            Ok(metadata) => {
                let take = metadata.len().min(FRAME_HEADER_LEN as u64);

                Ok(Some(read_range(&path, 0, take).await?))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.packed_head(kind, key, FRAME_HEADER_LEN as u64).await
            }
            Err(error) => Err(error.into()),
        }
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

    /// The manifest kept under `key`, if the content is kept as chunks.
    ///
    /// A manifest is what says how a cut content is put back together, so this is what a caller
    /// needs to carry a content that is kept as chunks to another store, rather than carrying the
    /// content and letting the other end cut it all over again. What it names are the chunks, which
    /// are objects like any other and are read with `read_object`.
    ///
    /// [`put_manifest`](Self::put_manifest) is the other half: a manifest that came from another
    /// store is written down with it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the manifest cannot be looked for, and [`Error::Malformed`] if what
    /// is there does not read as a manifest.
    pub async fn manifest_of(&self, key: &Key) -> Result<Option<Manifest>, Error> {
        self.read_manifest(key).await
    }

    /// Keeps `manifest` as how the content under `key` is put back together.
    ///
    /// This is how a manifest that arrived from another store is written down: the chunks it names
    /// are the other store's to send, and until they are here the store does not hold the content
    /// the key promises. What is written is framed the store's own way, so a manifest from a store
    /// that was told to cut differently still reads here as a manifest.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the manifest cannot be written.
    pub async fn put_manifest(&self, key: &Key, manifest: &Manifest) -> Result<(), Error> {
        self.write_manifest(key, manifest, self.codec).await
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
