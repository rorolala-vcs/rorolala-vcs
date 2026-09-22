//! Reading and writing one entry, and the file-level cares that come with it.
//!
//! An entry is what a store lays down for one key: a frame and a payload. Getting one onto the disk
//! is done the same way whatever it holds — written beside where it belongs and moved into place —
//! and what holds a store together is that every one of them arrived whole.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::atomic::{AtomicU64, Ordering};

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, OBJECTS_DIR, PACKED_DIR, TEMPORARY_SUFFIX};
use crate::{Error, Frame, Key};

impl RorolalaStorage {
    /// Writes an entry — its frame and its payload — where `path` says.
    pub(super) async fn write_entry(
        &self,
        path: &Path,
        frame: &Frame,
        payload: &[u8],
    ) -> Result<(), Error> {
        let parent = path.parent().ok_or(Error::Malformed)?;
        tokio::fs::create_dir_all(parent).await?;

        let mut bytes = frame.encode();
        bytes.extend_from_slice(payload);

        // Written beside where it belongs and moved into place, so a write that is cut short
        // leaves a store that is what it was rather than one holding half an entry. The temporary
        // name is one no other write is using, so two writers of one path never write through each
        // other's file — see `temporary_beside`.
        let temporary = temporary_beside(path)?;
        tokio::fs::write(&temporary, &bytes).await?;
        tokio::fs::rename(&temporary, path).await?;

        Ok(())
    }

    /// Drops the entry `path` names, if there is one there.
    pub(super) async fn drop_entry(&self, path: &Path) -> Result<(), Error> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Reads the entry `path` names, if there is one there.
    pub(super) async fn read_entry(&self, path: &Path) -> Result<Option<(Frame, Vec<u8>)>, Error> {
        let bytes = match tokio::fs::read(path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };

        let (frame, header) = Frame::decode(&bytes)?;
        let payload = bytes.get(header..).ok_or(Error::Malformed)?.to_vec();

        Ok(Some((frame, payload)))
    }

    /// Removes the temporary files a write that was cut short left behind, and answers how many.
    ///
    /// A write lays its bytes down beside where they belong and moves them into place, so a run that
    /// stopped in the middle leaves a `.tmp` file nothing will ever read. Such a file costs space and
    /// nothing else — it is not part of the store and no key names it — so taking it back is a
    /// caller's to do when it likes, and there is nowhere it has to be done.
    ///
    /// A temporary another write is still using is removed too, if it is removed while that write is
    /// running: the write then fails to move its file into place, which is a failed write and not a
    /// store that does not hold together. Clearing a store nothing else is writing to is the plain
    /// case.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the store cannot be walked or a file cannot be removed.
    pub async fn clear_temporaries(&self) -> Result<usize, Error> {
        let mut removed = 0;

        for directory in [OBJECTS_DIR, MANIFEST_DIR, PACKED_DIR] {
            sweep(&self.root.join(directory), &mut removed).await?;
        }

        Ok(removed)
    }
}

/// The key content is stored under: the hash of the content itself.
pub(super) fn key_of(content: &[u8]) -> Key {
    Key::new(*blake3::hash(content).as_bytes())
}

/// The frame's plain length, as this machine counts bytes.
pub(super) fn plain_len_of(frame: &Frame) -> Result<usize, Error> {
    usize::try_from(frame.plain_len).map_err(|_| Error::Malformed)
}

/// Fails unless `content` hashes to `key`.
pub(super) fn verify(key: &Key, content: &[u8]) -> Result<(), Error> {
    if key_of(content) == *key {
        Ok(())
    } else {
        Err(Error::Corrupt(*key))
    }
}

/// Whether there is something at `path`.
pub(super) async fn exists(path: &Path) -> Result<bool, Error> {
    match tokio::fs::metadata(path).await {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// The paths directly inside `directory`, or none when there is no directory there.
pub(super) async fn entries(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut entries = match tokio::fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };

    let mut paths = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        paths.push(entry.path());
    }

    Ok(paths)
}

/// Every key named by a file under `directory`, which is laid out two levels deep by digest.
pub(super) async fn collect(directory: &Path, keys: &mut BTreeSet<Key>) -> Result<(), Error> {
    for first in entries(directory).await? {
        for second in entries(&first).await? {
            for file in entries(&second).await? {
                if let Some(name) = file.file_name().and_then(|name| name.to_str())
                    && let Some(key) = key_from_hex(name)
                {
                    keys.insert(key);
                }
            }
        }
    }

    Ok(())
}

/// The key a name of the digest's own width stands for, if it is one.
fn key_from_hex(name: &str) -> Option<Key> {
    Key::from_str(name).ok()
}

/// Removes the temporary files under `directory`, counting them in `removed`.
async fn sweep(directory: &Path, removed: &mut usize) -> Result<(), Error> {
    for path in entries(directory).await? {
        if tokio::fs::metadata(&path)
            .await
            .is_ok_and(|data| data.is_dir())
        {
            Box::pin(sweep(&path, removed)).await?;

            continue;
        }

        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(TEMPORARY_SUFFIX))
        {
            tokio::fs::remove_file(&path).await?;
            *removed += 1;
        }
    }

    Ok(())
}

/// The temporary name a write to `path` is laid down under before it is moved into place.
///
/// The name is unique among every write in this process, so two of them writing one path — two
/// threads, two tasks, or one task's two writes — never write through each other's file rather than
/// only across processes. It hangs off the whole name rather than replacing the extension, so a pack
/// and the index beside it never share a temporary.
fn temporary_beside(path: &Path) -> Result<PathBuf, Error> {
    /// How many temporaries this process has named, so that no two are the same.
    static NAMED: AtomicU64 = AtomicU64::new(0);

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(Error::Malformed)?;
    let unique = NAMED.fetch_add(1, Ordering::Relaxed);

    Ok(path.with_file_name(format!(
        "{name}.{}.{unique}{TEMPORARY_SUFFIX}",
        std::process::id()
    )))
}

/// Writes `bytes` to `path` whole, and makes it outlive a crash before returning.
///
/// The temporary is written, synced and moved into place, and the directory it landed in is synced
/// too — a rename is durable only once its directory is, since the file's bytes may be on the disk
/// while the name pointing at them is not. This is for the files whose loss would be the loss of
/// content: a pack's index names a pack that may be the only copy of what it holds, so an index a
/// reader can find after a crash has to name a pack that is all there.
pub(super) async fn write_whole_durably(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    use tokio::io::AsyncWriteExt as _;

    let parent = path.parent().ok_or(Error::Malformed)?;
    tokio::fs::create_dir_all(parent).await?;

    let temporary = temporary_beside(path)?;
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(bytes).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temporary, path).await?;

    sync_directory(parent).await
}

/// Makes the names in `directory` durable, so that a file moved into or out of it survives a crash.
///
/// A directory is synced by opening it and syncing the handle, which is how the systems this store is
/// kept on do it. Where that is not how it is done, nothing is synced and nothing is claimed: the
/// writes are still atomic, they are just not promised to outlive the machine.
pub(super) async fn sync_directory(directory: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        let handle = tokio::fs::File::open(directory).await?;
        handle.sync_all().await?;
    }

    let _ = directory;

    Ok(())
}

/// The `len` bytes of the file at `path` starting `offset` in.
///
/// A packed entry is read straight out of the pack it sits in rather than by reading the pack
/// whole: a pack is as big as everything in it, and a read that wanted one small object should not
/// pay for the rest.
pub(super) async fn read_range(path: &Path, offset: u64, len: u64) -> Result<Vec<u8>, Error> {
    use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _};

    let len = usize::try_from(len).map_err(|_| Error::Malformed)?;
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(io::SeekFrom::Start(offset)).await?;

    let mut bytes = vec![0_u8; len];
    file.read_exact(&mut bytes).await?;

    Ok(bytes)
}
