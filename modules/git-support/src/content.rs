//! Where a material's content comes from when the private area does not hold it.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use blake3::Hasher;
use rola_define_format::ContentHash;

use crate::error::Error;

/// A place a material's content can be asked for.
pub trait ContentSource {
    /// Writes the content `content` names to `path`, making the directories above it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unavailable`] when the source does not hold the content, and
    /// [`Error::Io`] when it cannot be written.
    fn write(&self, content: ContentHash, path: &Path) -> Result<(), Error>;
}

/// Loose content on this machine, laid down the way a rola store lays its objects down.
///
/// An object is read from `<root>/obj/<hh>/<hh>/<hex>`, the store's own layout for a loose object.
/// Content that was kept cut is a manifest under `manifest/` naming chunks, which a bare copy
/// cannot put back together: that is the store backend's job, and it belongs to the program rather
/// than here.
#[derive(Debug, Clone)]
pub struct LocalStore {
    /// The store's root.
    root: PathBuf,
}

impl LocalStore {
    /// The store rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Where a loose object of `content` is kept.
    #[must_use]
    pub fn object_path(&self, content: ContentHash) -> PathBuf {
        let token = content.to_token();
        let hex = token.strip_prefix("blake3:").unwrap_or(token.as_str());

        self.root
            .join("obj")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex)
    }
}

impl ContentSource for LocalStore {
    fn write(&self, content: ContentHash, path: &Path) -> Result<(), Error> {
        let object = self.object_path(content);

        if !object.is_file() {
            return Err(Error::Unavailable {
                content: content.to_token(),
            });
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        fs::copy(&object, path).map_err(Error::Io)?;

        Ok(())
    }
}

/// The content the file at `path` holds.
///
/// # Errors
///
/// Returns [`Error::Io`] when the file cannot be read.
pub fn digest_of(path: &Path) -> Result<ContentHash, Error> {
    let mut file = fs::File::open(path).map_err(Error::Io)?;
    let mut hasher = Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(Error::Io)?;
        if read == 0 {
            break;
        }

        hasher.update(&buffer[..read]);
    }

    Ok(ContentHash::from_digest(*hasher.finalize().as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rola_define_format::ContentHash;

    use super::{ContentSource, LocalStore, digest_of};
    use crate::error::Error;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-git-support-content-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_file_is_named_by_the_blake3_of_its_content() {
        let dir = scratch("digest");
        let file = dir.join("content.bin");
        std::fs::write(&file, b"the content a material stands on").unwrap();

        let expected = blake3::hash(b"the content a material stands on");
        let found = digest_of(&file).unwrap();

        assert_eq!(found.digest(), expected.as_bytes());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_loose_object_is_read_out_of_the_store() {
        let dir = scratch("store");
        let store = LocalStore::new(dir.join("store"));

        let held = dir.join("held.bin");
        std::fs::write(&held, b"the content").unwrap();
        let content = digest_of(&held).unwrap();
        let object = store.object_path(content);

        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, b"the content").unwrap();

        let out = dir.join("out").join("hero.psd");
        store.write(content, &out).unwrap();

        assert_eq!(std::fs::read(&out).unwrap(), b"the content");
        assert_eq!(digest_of(&out).unwrap(), content);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_object_the_store_does_not_hold_is_an_error() {
        let dir = scratch("missing");
        let store = LocalStore::new(dir.join("store"));
        let content = ContentHash::from_digest([1; 32]);

        let error = store.write(content, &dir.join("out")).unwrap_err();

        assert!(matches!(error, Error::Unavailable { .. }), "{error}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
