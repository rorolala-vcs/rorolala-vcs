//! Locking a place, so that one run changes it at a time.
//!
//! A Workspace, a Vault and the store either of them keeps are each a place one run changes and
//! another must not, and what holds them apart is a file: a place is locked while its lock file
//! is there, and unlocked while it is not. The file is the whole of the lock — nothing reads its
//! contents, and nothing consults anything else — which is what lets a person see the lock, and
//! take it away, without a tool.
//!
//! What a caller holds while it works is a [`LockingGuard`], and it is the guard holding the
//! lock that says so: the lock is taken when the guard is made and given back when the guard
//! goes, so a change that is made through a guard is made under the lock, and one made without
//! one cannot be. What is held is the change itself — the store's packs, say — so a guard is
//! also where a change that must not be made twice at once is put rather than on the place.
//!
//! A lock is not a lease: it is held until it is given back, and a run that dies holding one
//! leaves the place locked. That is deliberate. The alternative is a lock that times out, which
//! means a change that is still going is one another run may start over — and a change cut short
//! by another run is worse than a place a person has to unlock by hand.

use std::fmt;
use std::future::Future;
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use tokio::fs::OpenOptions;

/// What locking a place failed with.
#[derive(Debug)]
#[non_exhaustive]
pub enum LockError {
    /// The place carries a lock already.
    ///
    /// What holds it is another run, or one that died and left the file behind; the two are the
    /// same thing from here, since a lock is a file being there.
    Locked,
    /// The lock file could not be made.
    Io(io::Error),
}

impl fmt::Display for LockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locked => formatter.write_str("a lock on it is already there"),
            Self::Io(source) => write!(formatter, "the lock file could not be made: {source}"),
        }
    }
}

impl std::error::Error for LockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Locked => None,
        }
    }
}

impl From<io::Error> for LockError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

/// A lock on a place, kept until this goes.
///
/// The lock is the lock file being there, so making one is what the guard does and taking it away
/// is what dropping it does. Until then the place is locked, whether or not anything is done with
/// it — a guard held for no reason still holds the lock.
///
/// It reads as the place it is held over, so what a caller has while it works is the place
/// itself, and the lock is something it never has to name.
#[must_use = "the lock is given back as soon as the guard goes"]
pub struct LockingGuard<T> {
    /// What the lock is held over.
    held: T,
    /// The file whose presence says the lock is held.
    lock: PathBuf,
}

impl<T> LockingGuard<T> {
    /// Locks `lock`, holding `held` for as long as the guard lives.
    ///
    /// Making the lock file is the whole of locking, and it is made to fail unless it is new: a
    /// place that carries one already is refused rather than joined, since a lock that two
    /// runs hold is not a lock.
    ///
    /// # Errors
    ///
    /// Returns [`LockError::Locked`] when the place is locked already, and [`LockError::Io`] when
    /// the lock file cannot be made — a directory that is not there, or one that is not writable.
    pub async fn acquire(held: T, lock: impl Into<PathBuf>) -> Result<Self, LockError> {
        let lock = lock.into();

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .await
        {
            Ok(_) => Ok(Self { held, lock }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(LockError::Locked),
            Err(error) => Err(LockError::Io(error)),
        }
    }

    /// The file whose presence is the lock.
    #[must_use]
    pub fn lock_path(&self) -> &Path {
        self.lock.as_path()
    }
}

impl<T> Deref for LockingGuard<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.held
    }
}

impl<T> DerefMut for LockingGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.held
    }
}

impl<T> Drop for LockingGuard<T> {
    fn drop(&mut self) {
        // Giving the lock back is best effort, and failing silently is the whole of what is done
        // about it: a `drop` has nowhere to report to, and what a failure means is that the lock
        // file is still there while nothing holds it. That leaves the place locked until someone
        // takes the file away by hand, which is the same state a run that died holding the lock
        // leaves behind — see the [module docs](self).
        let _ = std::fs::remove_file(&self.lock);
    }
}

/// A place that can be locked, so that what changes it does so one run at a time.
///
/// A place is locked for as long as a [`LockingGuard`] over it lives, and what is asked of an
/// implementor is only where its lock file sits: everything else — how the lock is taken, what
/// taking it fails with, what holding it looks like — is the same for every place, and is here.
pub trait Lockable: Sized + Clone {
    /// The file whose presence says this place is locked.
    fn lock_path(&self) -> PathBuf;

    /// Whether this place is locked right now.
    ///
    /// The lock is a file being there, so this answers what anything else looking at the
    /// directory sees. A run that died holding the lock is therefore reported as locking, which
    /// is the truth of the directory rather than a guess at who is there.
    #[must_use]
    fn is_locking(&self) -> bool {
        self.lock_path().is_file()
    }

    /// Locks this place, answering the guard the lock is held by.
    ///
    /// The place is copied out rather than borrowed, so that the guard is a value a caller may
    /// keep, hand over, or hold across an await without saying how long it lives. All three
    /// places in Rorolala are cheap to copy — a directory, or a directory and two choices.
    ///
    /// # Errors
    ///
    /// Returns [`LockError::Locked`] when the place is locked already, and [`LockError::Io`] when
    /// its lock file cannot be made.
    fn lock(&self) -> impl Future<Output = Result<LockingGuard<Self>, LockError>> + Send;
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{LockError, Lockable, LockingGuard};

    /// A place of this test's own, so that a lock is read without a store around it.
    #[derive(Clone)]
    struct Place {
        /// The file the lock is taken as.
        lock: PathBuf,
    }

    impl Lockable for Place {
        fn lock_path(&self) -> PathBuf {
            self.lock.clone()
        }

        async fn lock(&self) -> Result<LockingGuard<Self>, LockError> {
            LockingGuard::acquire(self.clone(), self.lock_path()).await
        }
    }

    /// A place of its own, in a directory of its own, emptied first so a rerun starts clean.
    fn place(label: &str) -> (PathBuf, Place) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let parent = std::env::temp_dir().join(format!(
            "rorolala-locking-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();

        let place = Place {
            lock: parent.join("lock"),
        };

        (parent, place)
    }

    #[tokio::test]
    async fn a_lock_is_held_until_the_guard_goes() {
        let (parent, place) = place("held");

        assert!(!place.is_locking());

        let guard = place.lock().await.unwrap();

        // The lock is the file, so a place a guard is held over is one anything else sees as
        // locked — the guard holds it by being the one that has the file.
        assert!(place.is_locking());
        assert!(place.lock_path().is_file());
        assert_eq!(guard.lock_path(), place.lock_path().as_path());

        // And giving the guard back gives the lock back, which is what lets the place be locked
        // again. Nothing else does: the file is not a lease and does not time out.
        drop(guard);

        assert!(!place.is_locking());
        assert!(!place.lock_path().exists());

        let again = place.lock().await.unwrap();
        drop(again);

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_lock_already_taken_is_refused() {
        let (parent, place) = place("refused");

        let held = place.lock().await.unwrap();

        // A second taker is refused rather than joined: a lock two runs hold is not a lock.
        assert!(matches!(place.lock().await, Err(LockError::Locked)));

        drop(held);

        // A lock left behind by a run that did not finish is a lock the way any other is — what
        // says a place is locked is the file, not who made it.
        fs::write(place.lock_path(), b"").unwrap();

        assert!(place.is_locking());
        assert!(matches!(place.lock().await, Err(LockError::Locked)));

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_lock_file_that_cannot_be_made_says_so() {
        let (parent, _) = place("unwritable");

        // A lock is a file where the place says it is, so a place whose directory is gone cannot be
        // locked — as a failure of its own rather than as a lock that is held.
        let absent = Place {
            lock: parent.join("gone").join("lock"),
        };

        assert!(matches!(absent.lock().await, Err(LockError::Io(_))));

        let _ = fs::remove_dir_all(&parent);
    }
}
