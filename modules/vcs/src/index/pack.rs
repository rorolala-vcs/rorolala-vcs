//! Laying the index's objects out in a pack.
//!
//! A pack is many objects in one file, and the index beside it says where each one sits; the
//! bytes an entry holds are the very bytes it sat loose as — the frame and all — so packing
//! changes where an object lives and nothing else. The index keeps one pack rather than as many
//! as a size limit allows: what it holds are the small, fixed objects the version control logic
//! works in, so there is nothing to weigh one pack against another for.
//!
//! A pack is found by its index, so the index is written before the pack it names and dropped
//! after it; a `.pack` left behind by a write cut short is garbage rather than an index naming a
//! pack that is not there.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;

use rorolala_storage::{PackEntry, PackIndex};

use super::{VCSIndex, entries};
use crate::Hash;
use crate::error::VCSIndexError;

/// The directory packs are kept under, inside an index.
const PACKED_DIR: &str = "packed";

/// Where an object is read from while the index is laid out afresh.
pub enum Source {
    /// A loose file, which holds the whole framed entry.
    Loose(PathBuf),
    /// An entry inside a pack, which is read by its place in that pack.
    Packed(u64, PackEntry),
}

impl VCSIndex {
    /// Where the packed file of `index` sits.
    fn pack_path(&self, index: u64) -> PathBuf {
        self.path
            .join(PACKED_DIR)
            .join(format!("packed_{index}.pack"))
    }

    /// Where the index of the pack of `index` sits, which is what says what the pack holds.
    fn pack_index_path(&self, index: u64) -> PathBuf {
        self.path
            .join(PACKED_DIR)
            .join(format!("packed_{index}.idx"))
    }

    /// The packs the index holds, lowest index first.
    ///
    /// A pack is found by its index rather than by its pack file, so a `.pack` whose index never
    /// arrived is one no reader could reach anyway, and is not counted.
    pub(crate) async fn packs(&self) -> Result<Vec<u64>, VCSIndexError> {
        let mut packs = Vec::new();

        for path in entries(&self.path.join(PACKED_DIR)).await? {
            if let Some(name) = path.file_name().and_then(|name| name.to_str())
                && let Some(index) = index_of_pack(name)
            {
                packs.push(index);
            }
        }

        packs.sort_unstable();

        Ok(packs)
    }

    /// The directory of the pack of `index`.
    pub(crate) async fn read_pack_index(&self, index: u64) -> Result<PackIndex, VCSIndexError> {
        let bytes = tokio::fs::read(self.pack_index_path(index)).await?;

        Ok(PackIndex::decode(&bytes)?)
    }

    /// Where the object stored under `key` sits in a pack, if a pack holds it.
    pub(crate) async fn find_packed(&self, key: &Hash) -> Result<Option<Source>, VCSIndexError> {
        for index in self.packs().await? {
            let directory = self.read_pack_index(index).await?;

            if let Some(entry) = directory.find(key) {
                return Ok(Some(Source::Packed(index, *entry)));
            }
        }

        Ok(None)
    }

    /// The bytes of the entry `source` names, framed as it sits.
    pub(crate) async fn read_source(&self, source: &Source) -> Result<Vec<u8>, VCSIndexError> {
        match source {
            Source::Loose(path) => Ok(tokio::fs::read(path).await?),
            Source::Packed(index, entry) => {
                read_range(&self.pack_path(*index), entry.offset(), entry.len()).await
            }
        }
    }

    /// Every key the index holds loosely.
    pub(crate) async fn list_loose_keys(&self) -> Result<Vec<Hash>, VCSIndexError> {
        let mut keys = BTreeSet::new();
        super::collect(&self.path.join(super::OBJECTS_DIR), &mut keys).await?;

        Ok(keys.into_iter().collect())
    }

    /// Lays every object the index holds out in one pack, answering whether anything changed.
    ///
    /// Everything is gathered up — the entries of every pack, and every loose object — and written
    /// again: what was many packs and a scattering of loose files becomes one pack. What each
    /// object *is* does not change, so every read that worked before works after. An index already
    /// holding one pack and nothing loose is left alone and answered with `false`.
    ///
    /// The new pack is written and the old ones dropped only once it is there, so a reader that
    /// arrives in the middle finds every object in the packs it already knew.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Io`] if an object cannot be read or a pack cannot be written, and
    /// [`VCSIndexError::Malformed`] if a path it needs cannot be named.
    pub async fn repack(&self) -> Result<bool, VCSIndexError> {
        let loose = self.list_loose_keys().await?;
        let packs = self.packs().await?;

        if loose.is_empty() && packs.len() <= 1 {
            return Ok(false);
        }

        // Everything the index holds, under the key it is held by: a loose file, or the place in
        // a pack it sits at. A key both loose and packed is one entry here, and the loose copy is
        // the one read — the two are the same bytes.
        let mut sources: BTreeMap<Hash, Source> = BTreeMap::new();
        for index in &packs {
            for entry in self.read_pack_index(*index).await?.entries() {
                sources.insert(entry.key(), Source::Packed(*index, *entry));
            }
        }
        for key in &loose {
            sources.insert(*key, Source::Loose(self.object_path(key)));
        }

        if sources.is_empty() {
            return Ok(false);
        }

        // The new pack is written at a number no pack holds, so the old ones are not written over
        // while they are still what a reader would find.
        let mut framed = Vec::new();
        let mut directory = PackIndex::default();
        for (key, source) in &sources {
            let bytes = self.read_source(source).await?;
            let offset = framed.len() as u64;
            let len = bytes.len() as u64;

            framed.extend_from_slice(&bytes);
            directory.push(PackEntry::new(*key, offset, len));
        }

        let fresh = packs.last().map_or(0, |last| last + 1);
        write_whole_durably(&self.pack_index_path(fresh), &directory.encode()).await?;
        write_whole_durably(&self.pack_path(fresh), &framed).await?;

        // The objects the pack now holds are not loose any more, and the packs it replaces are
        // gone: what a read finds is the one pack either way.
        for key in &loose {
            let _ = tokio::fs::remove_file(self.object_path(key)).await;
        }
        for index in &packs {
            drop_pack(self, *index).await?;
        }

        // The index is left with one pack, numbered from nothing, so running this over and over
        // does not climb a number that says how often it was packed.
        if fresh != 0 {
            tokio::fs::rename(self.pack_index_path(fresh), self.pack_index_path(0)).await?;
            tokio::fs::rename(self.pack_path(fresh), self.pack_path(0)).await?;
        }

        prune_empty_directories(&self.path.join(super::OBJECTS_DIR)).await;

        Ok(true)
    }

    /// Drops the object stored under `key`, whether it is loose or in a pack.
    ///
    /// A packed entry is taken away by unpacking the pack it sits in without it, and dropping the
    /// pack: the kept entries are written loose first, so nothing is dropped before its bytes are
    /// somewhere else.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::NotFound`] if nothing is stored under `key`.
    pub(crate) async fn remove_object_packed(&self, key: &Hash) -> Result<(), VCSIndexError> {
        if let Some(Source::Packed(index, _)) = self.find_packed(key).await? {
            self.unpack_without(index, key).await?;
            return Ok(());
        }

        Err(VCSIndexError::NotFound(*key))
    }

    /// Writes every entry of the pack of `index` loose, but `removed`, and drops the pack.
    async fn unpack_without(&self, index: u64, removed: &Hash) -> Result<(), VCSIndexError> {
        for entry in self.read_pack_index(index).await?.entries() {
            if entry.key() == *removed {
                continue;
            }

            let bytes = read_range(&self.pack_path(index), entry.offset(), entry.len()).await?;
            write_whole_durably(&self.object_path(&entry.key()), &bytes).await?;
        }

        drop_pack(self, index).await
    }
}

/// The pack number an index's name stands for, if it is one this build writes.
fn index_of_pack(name: &str) -> Option<u64> {
    name.strip_prefix("packed_")
        .and_then(|rest| rest.strip_suffix(".idx"))
        .and_then(|number| number.parse().ok())
        .filter(|index| name == format!("packed_{index}.idx"))
}

/// Drops the pack of `index` and its index, the index first.
async fn drop_pack(index: &VCSIndex, number: u64) -> Result<(), VCSIndexError> {
    for path in [index.pack_index_path(number), index.pack_path(number)] {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}

/// The `len` bytes of the file at `path` starting `offset` in.
///
/// A packed entry is read straight out of its pack rather than by reading the pack whole: a pack
/// is as big as everything in it, and a read that wanted one object should not pay for the rest.
async fn read_range(
    path: &std::path::Path,
    offset: u64,
    len: u64,
) -> Result<Vec<u8>, VCSIndexError> {
    use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _};

    let len = usize::try_from(len).map_err(|_| VCSIndexError::Malformed)?;
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(io::SeekFrom::Start(offset)).await?;

    let mut bytes = vec![0_u8; len];
    file.read_exact(&mut bytes).await?;

    Ok(bytes)
}

/// Writes `bytes` to `path` whole, laid down beside it and moved into place.
///
/// The temporary is named so no two writes in this process share one, so two of them writing a
/// pack and its index never write through each other's file.
async fn write_whole_durably(path: &std::path::Path, bytes: &[u8]) -> Result<(), VCSIndexError> {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// How many temporaries this process has named, so that no two are the same.
    static NAMED: AtomicU64 = AtomicU64::new(0);

    let parent = path.parent().ok_or(VCSIndexError::Malformed)?;
    tokio::fs::create_dir_all(parent).await?;

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(VCSIndexError::Malformed)?;
    let unique = NAMED.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!("{name}.{}.{unique}.tmp", std::process::id()));

    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(&temporary, path).await?;

    Ok(())
}

/// Removes the shard directories under `directory` that no object is left in.
async fn prune_empty_directories(directory: &std::path::Path) {
    let Ok(firsts) = entries(directory).await else {
        return;
    };

    for first in firsts {
        let Ok(seconds) = entries(&first).await else {
            continue;
        };

        for second in seconds {
            let _ = tokio::fs::remove_dir(&second).await;
        }

        let _ = tokio::fs::remove_dir(&first).await;
    }
}
