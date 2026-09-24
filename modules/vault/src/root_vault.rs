//! The Vault a nested Vault sits inside, and the Vaults it holds.
//!
//! A [`Vault`] is the one found at the nearest configuration above a directory; a [`RootVault`]
//! is the outermost one. Which of the two a caller wants is a question of standpoint: the
//! nearest Vault is the one the caller is *in*, while the outermost is the one that *holds* it
//! — and what a root holds is written down, as the directories under its [`VAULTS_DIR`].
//!
//! So with `vault/vault.toml` and `vault/code/vault.toml`, a search from `vault/code` finds
//! `vault/code` as a [`Vault`] and `vault` as a [`RootVault`]. Naming and listing what a root
//! holds is what this adds; everything a [`Vault`] is comes through [`Deref`], since what is
//! wrapped is a Vault and not a copy of one.
//!
//! Nothing here writes: a Vault is one because it is already on disk, and a root holds one
//! because it is already there to be found.

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use just_fmt::fmt_path_str;
use rorolala_storage::{LockError, Lockable, LockingGuard};
use rorolala_utils_location::Locate;
use tokio::fs;

use crate::{CONFIG_PATH, VAULTS_DIR, Vault};

/// A Vault seen from the outside: the root of the Vaults nested inside it.
///
/// It records the outermost [`Vault`] above wherever it was located from, and reads through to
/// it with [`Deref`], [`DerefMut`] and [`From`], so a `RootVault` is a [`Vault`] wherever one is
/// wanted. What it adds is the view from outside: the Vaults under its [`VAULTS_DIR`] are the
/// ones it holds, listed and reached by the methods below.
///
/// A root is not a kind of Vault that is marked as one — there is nothing in a configuration
/// saying "this holds others". It is the Vault that happens to be outermost, and the Vaults it
/// holds are the directories that happen to sit under it.
#[derive(Default, Clone)]
pub struct RootVault {
    /// The outermost Vault: the one whose configuration a search walked furthest to reach.
    vault: Vault,
}

/// Locates the outermost [`Vault`] above the given directory.
///
/// Starting at `vault_dir`, this walks all the way up the directory tree and keeps the last
/// directory holding a Vault configuration ([`CONFIG_PATH`]) — the Vault that holds every other
/// Vault the directory sits inside. Returns `None` if no Vault could be found.
///
/// The nearest configuration is [`Vault::locate`]'s: that answers where a caller is, while this
/// answers what that sits inside.
#[must_use]
pub fn locate_root_vault(vault_dir: &Path) -> Option<RootVault> {
    RootVault::locate(vault_dir)
}

impl Locate for RootVault {
    fn locate(cwd: &Path) -> Option<Self> {
        // Every level holding a configuration is remembered rather than returned, so what comes
        // back is the outermost of them: the Vault that holds the others is the one the search
        // walked furthest to reach.
        let mut outermost = None;
        let mut current = Some(cwd);

        while let Some(path) = current {
            if path.join(CONFIG_PATH).exists() {
                outermost = Some(Vault::at(path.to_path_buf()));
            }

            current = path.parent();
        }

        outermost.map(|vault| Self { vault })
    }

    fn get_root(&self) -> &Path {
        self.vault.get_root()
    }
}

impl Deref for RootVault {
    type Target = Vault;

    fn deref(&self) -> &Self::Target {
        &self.vault
    }
}

impl DerefMut for RootVault {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vault
    }
}

impl From<RootVault> for Vault {
    fn from(root: RootVault) -> Self {
        root.vault
    }
}

impl Lockable for RootVault {
    /// The lock sits at the root's own root — the outermost Vault's, not the one a run is in.
    ///
    /// A root reads through to a [`Vault`], so without this a lock taken through the root would be
    /// the *nearest* Vault's and one run could hold the root while another changed the same Vault
    /// by name. What is locked here is the directory the whole tree hangs from.
    fn lock_path(&self) -> PathBuf {
        self.vault.lock_path()
    }

    async fn lock(&self) -> Result<LockingGuard<Self>, LockError> {
        LockingGuard::acquire(self.clone(), self.lock_path()).await
    }
}

impl RootVault {
    /// Every Vault the root holds, in name order.
    ///
    /// A Vault the root holds is a directory directly under its [`VAULTS_DIR`] carrying a
    /// configuration of its own. The search is one level deep, which is what *holds* means
    /// here: a Vault nested further down is one of the Vaults in between rather than one of the
    /// root's.
    ///
    /// Nothing that cannot be read is an error. A root with no [`VAULTS_DIR`], one that cannot
    /// be listed, and one whose held directories cannot be looked into all come back as holding
    /// nothing, since a list of what is there has nothing to say about why it is not.
    #[must_use]
    pub async fn list_vaults(&self) -> Vec<Vault> {
        self.held_vaults()
            .await
            .into_iter()
            .map(|(_, dir)| Vault::at(dir))
            .collect()
    }

    /// The names of every Vault the root holds, in name order.
    ///
    /// A name is the directory the Vault sits in under [`VAULTS_DIR`], and it is what
    /// [`get_vault_by_path`](Self::get_vault_by_path) takes to reach that Vault again. What is
    /// held is read the way [`list_vaults`](Self::list_vaults) reads it, so a name here and a
    /// Vault there are the same set.
    #[must_use]
    pub async fn list_vault_names(&self) -> Vec<String> {
        self.held_vaults()
            .await
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    /// The Vault `path` names, if the root holds one there.
    ///
    /// `path` is relative to the root, and reaches anywhere under it rather than only into
    /// [`VAULTS_DIR`]: `vaults/alpha` and `code` are both paths, and either names a Vault only
    /// if a configuration sits at the end of it. A path that names the root itself — nothing,
    /// `.`, or `./` — names the root, which is a Vault, and hands back the one this was located
    /// as.
    ///
    /// # Escaping
    ///
    /// The path is normalized before it is joined, the way [`fmt_path_str`] normalizes one, and
    /// it is normalized by walking the *text* of the path rather than the filesystem: `..` is
    /// resolved against the components before it, so a path climbing past the root is clamped to
    /// the root instead of reaching above it, and a leading separator is dropped so an absolute
    /// path reads as the relative path it names. Neither reaches out of the root.
    ///
    /// A **symlink** is a different matter, and is deliberately followed: the path is known here
    /// only as text, so a link under the root that points elsewhere is followed elsewhere, and
    /// the Vault returned may be one the root does not hold at all. A root that must not be left
    /// through is one that must keep symlinks out of itself — nothing here will.
    #[must_use]
    pub async fn get_vault_by_path(&self, path: impl Into<String>) -> Option<Vault> {
        let normalized = fmt_path_str(path.into()).ok()?;
        // What `fmt_path_str` leaves is relative to whatever it is joined onto, so a path that
        // is still absolute — an input that came in as one — is read as the relative path it
        // names instead: `join` would otherwise let an absolute path replace the root outright.
        let relative = normalized.trim_start_matches('/');

        // A path that names the root itself is the root: `fmt_path_str` writes nothing, `.` and
        // `./` all as a path walking to where it started, and what is at the root is the Vault
        // this was located as.
        if relative.is_empty() || relative == "." {
            return Some(self.vault.clone());
        }

        let dir = self.get_root().join(relative);

        if fs::metadata(dir.join(CONFIG_PATH))
            .await
            .is_ok_and(|metadata| metadata.is_file())
        {
            Some(Vault::at(dir))
        } else {
            None
        }
    }

    /// Every Vault the root holds, by the name it is known under and the directory it sits in,
    /// in name order.
    ///
    /// This is where *holds* is decided, so both listing methods read the same thing: the
    /// entries of [`VAULTS_DIR`], kept when a configuration sits directly inside them. What
    /// cannot be read is left out rather than reported — see [`list_vaults`](Self::list_vaults).
    async fn held_vaults(&self) -> Vec<(String, PathBuf)> {
        let directory = self.get_root().join(VAULTS_DIR);
        let mut held = Vec::new();

        // A root with no `vaults/` holds nothing, which is what `read_dir` failing says: there
        // is nothing here to tell apart from a directory holding no Vault.
        let Ok(mut entries) = fs::read_dir(&directory).await else {
            return held;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let dir = entry.path();
            let Some(name) = dir.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            if fs::metadata(dir.join(CONFIG_PATH))
                .await
                .is_ok_and(|metadata| metadata.is_file())
            {
                held.push((name.to_string(), dir));
            }
        }

        // A directory is listed in whatever order the filesystem keeps it, which is no order a
        // reader can rely on: a name is what a held Vault is reached by, so it is what they are
        // held in.
        held.sort();

        held
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate;

    use super::RootVault;
    use crate::{CONFIG_PATH, VAULTS_DIR, Vault};

    /// A parent directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-root-vault-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A Vault at `dir`, made the way a caller makes one.
    fn vault_at(dir: &Path) {
        Vault::create(dir).unwrap();
    }

    #[test]
    fn the_root_vault_is_the_outermost_one_above_a_directory() {
        let parent = scratch("outermost");
        let outer = parent.join("vault");
        let inner = outer.join("code");

        vault_at(&outer);
        vault_at(&inner);

        // The nearest configuration is the Vault the directory sits in...
        assert_eq!(Vault::locate(&inner).unwrap().get_root(), inner.as_path());

        // ...while the outermost is the Vault that holds it.
        assert_eq!(
            RootVault::locate(&inner).unwrap().get_root(),
            outer.as_path()
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn a_root_vault_reads_through_to_the_vault_it_records() {
        let parent = scratch("read-through");
        let dir = parent.join("vault");
        vault_at(&dir);

        let root = RootVault::locate(&dir).unwrap();

        // `Deref` gives everything a Vault gives.
        assert_eq!(root.get_root(), dir.as_path());
        assert_eq!(root.config_path(), dir.join(CONFIG_PATH));

        // `DerefMut` is what hands the Vault over to be changed...
        let mut root = root;
        let as_vault: &mut Vault = &mut root;
        assert_eq!(as_vault.get_root(), dir.as_path());

        // ...and `From` is what hands it over to be kept, with the wrapper gone.
        let taken: Vault = root.into();
        assert_eq!(taken.get_root(), dir.as_path());

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn the_vaults_a_root_holds_are_the_ones_under_its_vaults_directory() {
        let parent = scratch("held");
        let dir = parent.join("vault");
        vault_at(&dir);

        let under = dir.join(VAULTS_DIR);
        vault_at(&under.join("beta"));
        vault_at(&under.join("alpha"));

        // A directory carrying no configuration is not a Vault it holds...
        fs::create_dir_all(under.join("plain")).unwrap();
        // ...nor is a plain file, nor a Vault one level further down.
        fs::write(under.join("loose"), b"").unwrap();
        vault_at(&under.join("deep").join("inner"));

        let root = RootVault::locate(&dir).unwrap();

        // Names come back in name order, and only for the two that are Vaults.
        assert_eq!(root.list_vault_names().await, ["alpha", "beta"]);

        let alpha = under.join("alpha");
        let beta = under.join("beta");
        let listed = root.list_vaults().await;
        let roots: Vec<&Path> = listed.iter().map(Vault::get_root).collect();
        assert_eq!(roots, [alpha.as_path(), beta.as_path()]);

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_vault_the_root_holds_is_reached_by_the_path_it_sits_at() {
        let parent = scratch("by-path");
        let dir = parent.join("vault");
        vault_at(&dir);
        vault_at(&dir.join(VAULTS_DIR).join("alpha"));

        // A Vault nested below the root but outside `vaults/` is reachable too: what the root
        // holds and what is reachable under it are not the same question.
        vault_at(&dir.join("code"));

        let root = RootVault::locate(&dir).unwrap();

        let held = root.get_vault_by_path("vaults/alpha").await.unwrap();
        assert_eq!(
            held.get_root(),
            dir.join(VAULTS_DIR).join("alpha").as_path()
        );

        let nested = root.get_vault_by_path("code").await.unwrap();
        assert_eq!(nested.get_root(), dir.join("code").as_path());

        // A path naming no Vault is no Vault: nothing there, and a file rather than a Vault.
        fs::write(dir.join("file"), b"").unwrap();
        assert!(root.get_vault_by_path("nowhere").await.is_none());
        assert!(root.get_vault_by_path("file").await.is_none());

        // A path naming the root itself is the root, however it is spelled: nothing, `.`, or
        // `./` all come back as the Vault the root is.
        for names_the_root in ["", ".", "./"] {
            assert_eq!(
                root.get_vault_by_path(names_the_root)
                    .await
                    .unwrap()
                    .get_root(),
                dir.as_path()
            );
        }

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_path_that_climbs_out_of_the_root_is_clamped_to_it() {
        let parent = scratch("clamp");
        let dir = parent.join("vault");
        vault_at(&dir);

        // A Vault beside the root: what a path would reach if `..` were taken at its word.
        let beside = parent.join("beside");
        vault_at(&beside);

        let root = RootVault::locate(&dir).unwrap();

        // `..` is resolved over the text of the path, so climbing past the root stops at it and
        // `beside` is looked for inside the root rather than next to it.
        assert!(root.get_vault_by_path("../beside").await.is_none());
        assert!(root.get_vault_by_path("../../beside").await.is_none());

        // An absolute path is read as the relative path it names, for the same reason.
        let absolute = beside.to_str().unwrap().to_string();
        assert!(root.get_vault_by_path(absolute).await.is_none());

        // A path that climbs and comes back stays inside: what it names is read from the root.
        let under = dir.join(VAULTS_DIR);
        vault_at(&under.join("alpha"));
        let alpha = under.join("alpha");
        assert_eq!(
            root.get_vault_by_path("vaults/../vaults/alpha")
                .await
                .unwrap()
                .get_root(),
            alpha.as_path()
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlink_under_the_root_is_followed_out_of_it() {
        use std::os::unix::fs::symlink;

        let parent = scratch("symlink");
        let dir = parent.join("vault");
        vault_at(&dir);

        // A Vault outside the root, reached by a link inside it.
        let outside = parent.join("outside");
        vault_at(&outside);
        symlink(&outside, dir.join("linked")).unwrap();

        let root = RootVault::locate(&dir).unwrap();
        let linked = root.get_vault_by_path("linked").await.unwrap();

        // What was named is what is handed back...
        assert_eq!(linked.get_root(), dir.join("linked").as_path());

        // ...but what it stands for is the Vault kept outside the root. This is how it is meant
        // to behave rather than a hole: see the method's own note on escaping.
        assert!(tokio::fs::metadata(linked.config_path()).await.is_ok());
        assert_eq!(
            tokio::fs::canonicalize(linked.get_root()).await.unwrap(),
            tokio::fs::canonicalize(&outside).await.unwrap()
        );

        // The same holds for what the root holds: a link under `vaults/` is a Vault it holds.
        let under = dir.join(VAULTS_DIR);
        fs::create_dir_all(&under).unwrap();
        symlink(&outside, under.join("linked")).unwrap();
        assert_eq!(root.list_vault_names().await, ["linked"]);

        let _ = fs::remove_dir_all(&parent);
    }
}
