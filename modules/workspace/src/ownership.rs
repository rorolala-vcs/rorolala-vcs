//! Who holds what, read from the Vault copy a Workspace has fetched.
//!
//! Ownership is not a thing of its own: it is the `owner` a Layout keeps for one entry, and the
//! Vault's Layout is where it changes. A Workspace holds a read-only copy of that Layout under its
//! cache, and what is read here is that copy — nothing is reached for, so what is answered says
//! what the Vault held when it was last fetched.
//!
//! Reading it is three lookups: the path the file sits at names a `Uuid` in the Layout the
//! Workspace works in, the `Uuid` names a `MutableData` in the fetched copy, and that data names
//! the account that holds it. The three are the same on every read, which is why the Layouts are
//! opened once and the answers are handed out per path.

use std::path::{Path, PathBuf};

use rorolala_layout::{Layout, LayoutPath};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};

use crate::Workspace;

/// What holds one entry of a Workspace's tracked Vault.
///
/// # FFI
///
/// One of the four outcomes, read by tag. `Held` carries the name of the account that holds the
/// entry as an owned C string, which the caller reads and releases with `free_string`; the value
/// itself is returned by value and holds nothing else.
#[lazyffi(export = RolaEntryLock)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryLock {
    /// Nothing says: the Layout names no entry at the path, the Vault's copy was never fetched, or
    /// the copy came from a Vault whose Layout does not name the entry yet.
    Unnamed,

    /// No account holds it.
    Free,

    /// The account reading it holds it.
    Mine,

    /// Another account holds it, named here.
    Held(String),
}

/// A Workspace's ownership answers, prepared once over the copy it tracks.
///
/// # FFI
///
/// Opaque: C takes a pointer from `locate_rola_ownership` and gives it back with
/// `free_rola_ownership`. What it holds — the two Layouts, the account and the root — is never read
/// from outside.
#[lazyffi(export = RolaOwnership)]
pub struct Ownership {
    /// The Workspace root, normalised, so a path under it strips cleanly.
    root: PathBuf,

    /// The account the work acts as, if one is named.
    me: Option<String>,

    /// The Layout the Workspace works in, which names each path by a `Uuid`.
    local: Layout,

    /// The Vault's fetched copy, which names who holds each `Uuid`; nothing when it was never
    /// fetched.
    copy: Option<Layout>,
}

impl Ownership {
    /// Prepares the answers a Workspace can give about who holds what.
    ///
    /// Answers nothing when the Workspace works in no Layout, when that Layout tracks no Vault, or
    /// when the Layout cannot be read: ownership is about an entry of the Vault the work goes to,
    /// and a Workspace without one has none to give. A Vault's copy that was never fetched is not
    /// one of those — the Workspace still has answers, and they say that nothing is known yet.
    #[must_use]
    pub fn open(workspace: &Workspace, me: Option<String>) -> Option<Self> {
        let layouts = workspace.layouts();
        let current = layouts.current().ok().flatten()?;
        let vault = layouts.track(&current).ok().flatten()?;
        let local = layouts.get(&current).ok().flatten()?;

        // Opening a copy makes one where none is there, so a copy that was never fetched is asked
        // about first: what is not a directory is not a copy with nothing in it.
        let dir = workspace.readonly_layout_dir(&vault, VAULT_LAYOUT_NAME);
        let copy = dir.is_dir().then(|| Layout::open(dir).ok()).flatten();

        Some(Self {
            root: normalize(workspace.get_root()),
            me,
            local,
            copy,
        })
    }
}

#[lazyffi(export = rola_ownership_)]
impl Ownership {
    /// The account the work acts as, or an empty string when none is named.
    ///
    /// # FFI
    ///
    /// The account name, or an empty string. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_ownership_account)]
    pub fn account(&self) -> String {
        self.me.clone().unwrap_or_default()
    }

    /// What holds the entry at `path`.
    ///
    /// # FFI
    ///
    /// A [`EntryLock`] read by tag; `Held` carries the holder's name as an owned C string, released
    /// with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_ownership_lock_of)]
    pub fn lock_of(&self, path: &Path) -> EntryLock {
        let Some(copy) = &self.copy else {
            return EntryLock::Unnamed;
        };

        // Normalised on both sides, so a path spelled with a `.` or a `..`, or reached through the
        // Workspace's own root, is the same place rather than a second one.
        let path = normalize(path);
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return EntryLock::Unnamed;
        };
        let Ok(named) = LayoutPath::from_relative(relative) else {
            return EntryLock::Unnamed;
        };
        let Some(data) = self.local.id_of(&named).and_then(|id| copy.entry(id)) else {
            return EntryLock::Unnamed;
        };

        match data.owner() {
            None => EntryLock::Free,
            Some(owner) if self.me.as_deref() == Some(owner) => EntryLock::Mine,
            Some(owner) => EntryLock::Held(owner.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use rorolala_utils_constants::VAULT_LAYOUT_NAME;
    use rorolala_utils_location::Locate as _;
    use uuid::Uuid;

    use crate::{EntryLock, Ownership, Workspace};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-ownership-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);

        dir
    }

    /// A Workspace working in one Layout, tracking a Vault whose fetched copy holds `hero.psd` for
    /// `owner`.
    ///
    /// The Layout the Workspace works in names the path and leaves the entry unowned: who holds it
    /// is the Vault's to say, and the copy is where that is read.
    fn held(label: &str, owner: Option<&str>) -> (PathBuf, Workspace) {
        let dir = scratch(label);

        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        let layouts = workspace.layouts();
        let local = layouts.create("main").unwrap();
        layouts.set_current("main").unwrap();
        layouts.set_track("main", "vault").unwrap();

        let id = Uuid::from_u128(1);
        let path = LayoutPath::new("hero.psd").unwrap();
        local.create_path(&path, id).unwrap();
        local
            .create_entry(id, MutableData::new(None, [1; 32], String::new()))
            .unwrap();

        let copy_dir = workspace.readonly_layout_dir("vault", VAULT_LAYOUT_NAME);
        fs::create_dir_all(&copy_dir).unwrap();
        let copy = Layout::open(&copy_dir).unwrap();
        copy.create_path(&path, id).unwrap();
        copy.create_entry(
            id,
            MutableData::new(owner.map(str::to_owned), [1; 32], String::new()),
        )
        .unwrap();

        (dir, workspace)
    }

    #[test]
    fn an_entry_held_by_the_reader_is_mine() {
        let (dir, workspace) = held("mine", Some("alice"));
        let ownership = Ownership::open(&workspace, Some("alice".to_owned())).unwrap();

        assert_eq!(ownership.account(), "alice");
        assert_eq!(ownership.lock_of(&dir.join("hero.psd")), EntryLock::Mine);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_entry_held_by_another_names_them() {
        let (dir, workspace) = held("held", Some("alice"));
        let ownership = Ownership::open(&workspace, Some("bob".to_owned())).unwrap();

        assert_eq!(
            ownership.lock_of(&dir.join("hero.psd")),
            EntryLock::Held("alice".to_owned())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_entry_held_by_another_is_not_mine_when_nobody_is_bound() {
        let (dir, workspace) = held("unbound", Some("alice"));
        let ownership = Ownership::open(&workspace, None).unwrap();

        assert_eq!(ownership.account(), "");
        assert_eq!(
            ownership.lock_of(&dir.join("hero.psd")),
            EntryLock::Held("alice".to_owned())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_entry_nobody_holds_is_free() {
        let (dir, workspace) = held("free", None);
        let ownership = Ownership::open(&workspace, Some("alice".to_owned())).unwrap();

        assert_eq!(ownership.lock_of(&dir.join("hero.psd")), EntryLock::Free);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_the_layout_does_not_name_is_unnamed() {
        let (dir, workspace) = held("unnamed", Some("alice"));
        let ownership = Ownership::open(&workspace, Some("alice".to_owned())).unwrap();

        assert_eq!(
            ownership.lock_of(&dir.join("other.psd")),
            EntryLock::Unnamed
        );

        // So is a path outside the Workspace altogether, which names no entry of any Layout.
        assert_eq!(
            ownership.lock_of(&dir.join("..").join("outside.psd")),
            EntryLock::Unnamed
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_that_tracks_no_vault_has_no_answers() {
        let (dir, workspace) = held("untracked", Some("alice"));
        workspace.layouts().un_track("main").unwrap();

        assert!(Ownership::open(&workspace, Some("alice".to_owned())).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_whose_copy_was_never_fetched_answers_nothing_is_known() {
        let (dir, workspace) = held("unfetched", Some("alice"));
        fs::remove_dir_all(workspace.readonly_layout_dir("vault", VAULT_LAYOUT_NAME)).unwrap();

        let ownership = Ownership::open(&workspace, Some("alice".to_owned())).unwrap();

        assert_eq!(ownership.lock_of(&dir.join("hero.psd")), EntryLock::Unnamed);

        let _ = fs::remove_dir_all(&dir);
    }
}
