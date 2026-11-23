#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};

use rorolala_utils_constants::WORKSPACE_KEYS_DIR;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate;
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;

use rorolala_auth::{Account, Accounts, KeyLocateRule, Member, Members};

pub mod address;
pub mod chunking_read;
pub mod config_read;
pub mod existence;
pub mod fetch;
pub mod gc;
pub mod keys;
pub mod layout_read;
pub mod ownership_read;
pub mod remote;
pub mod status;
pub mod storage_read;
pub mod storage_set;
pub mod write;

pub use address::*;
pub use chunking_read::*;
pub use config_read::*;
pub use existence::*;
pub use gc::*;
pub use layout_read::*;
pub use ownership_read::*;
pub use status::*;
pub use storage_read::*;
pub use storage_set::*;
pub use write::*;

/// Authentication
pub use rorolala_auth as auth;

/// Locating and normalizing local paths
pub use rorolala_utils_location as location;

/// Searching the built-in book: the articles, their descriptions and the fragments a query matched
pub use rorolala_book_search as book_search;

/// Background daemon
pub use rorolala_daemon as daemon;

/// Errors across the C ABI
pub use rorolala_errors as errors;

/// The ignore rules a Workspace or a Vault is managed by
pub use rorolala_ignore as ignore;

/// The reverse dependency index over a version control index
pub use rorolala_inverse_index as inverse_index;

/// The Layout a Workspace and a Vault keep: what each names, and what it holds
pub use rorolala_layout as layout;

/// Reading a Workspace's tree against the Layout it works from
pub use rorolala_workspace_analyze as workspace_analyze;

/// Transport and wire protocol
pub use rorolala_protocol as protocol;

/// Storage of file data, and the layout that places it
pub use rorolala_storage as storage;

/// A directory versioned as one file: its structure, and what a tree version names
pub use rorolala_tree as tree;

/// Client-side workspace
pub use rorolala_workspace as workspace;

/// Server-side vault
pub use rorolala_vault as vault;

/// Version control
pub use rorolala_vcs as vcs;

/// A terminal's other half: the pseudo-terminal a shell is run in
pub use rola_desktop_pty as desktop_pty;

/// The local key roots the current directory sits inside, highest priority first.
///
/// The Workspace comes before the Vault, since it is the copy being worked in, so a key
/// kept there shadows an equally named one in the Vault. The Vault then brings every scope
/// it is admitted to, see [`Vault::key_scopes`], its own first and the Vaults holding it
/// after, so a member kept in a sub-vault is looked for there before the root, and is not
/// found at all by a search that starts above it. The keyring itself does not know what
/// any of those are: this is where their directories are turned into the plain roots it
/// searches.
fn local_roots(vault: Option<&Vault>, workspace: Option<&Workspace>) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    roots.extend(workspace.map(|held| held.get_root().join(WORKSPACE_KEYS_DIR)));
    if let Some(held) = vault {
        roots.extend(held.key_scopes());
    }

    roots
}

/// The Vault the current directory is inside, if it is inside one.
fn current_vault() -> Option<Vault> {
    Vault::locate(&std::env::current_dir().ok()?)
}

/// The Workspace the current directory is inside, if it is inside one.
fn current_workspace() -> Option<Workspace> {
    Workspace::locate(&std::env::current_dir().ok()?)
}

/// The local roots beside a Vault that was already found, and the Workspace the call is
/// made from.
fn roots_beside(vault: &Vault) -> Vec<PathBuf> {
    local_roots(Some(vault), current_workspace().as_ref())
}

/// The local roots the current directory sits inside.
fn roots_here() -> Vec<PathBuf> {
    let vault = current_vault();
    let workspace = current_workspace();

    local_roots(vault.as_ref(), workspace.as_ref())
}

/// Every member whose public key `rule` finds, highest priority first.
///
/// A member is named by a public key, which is meant to be shared, so it is looked for in
/// every scope the rule turns on, the local roots first, then the user's local data
/// directory, the filesystem root, and `ROLA_HOME`, with a name found higher up shadowing
/// the same name below it.
///
/// The local roots are the keys of the Workspace the call is made from and of `vault`, in
/// that order. The members cross as a set, read one index at a time; see [`Members`].
#[must_use]
#[lazyffi(export = rola_member_locate)]
pub fn member_locate(vault: &Vault, rule: &KeyLocateRule) -> Members {
    auth::locate_members(&roots_beside(vault), rule)
}

/// Every member `rule` finds beside the current Workspace, with no Vault to search beside.
///
/// [`member_locate`] takes a Vault because the Vault's own keys are one of the local roots it reads.
/// A caller that has no Vault used to pass a null pointer there, which the export cannot receive;
/// this is the same search with that root left out, and the rule's other scopes still answer. The
/// Workspace is found from the current directory.
///
/// # FFI
///
/// A `RolaMembers` the caller releases with `free_rola_members`.
#[must_use]
#[lazyffi(export = rola_member_locate_local)]
pub fn member_locate_local(rule: &KeyLocateRule) -> Members {
    let workspace = current_workspace();

    auth::locate_members(&local_roots(None, workspace.as_ref()), rule)
}

/// The member named `member_name`, if any scope the rule turns on holds its public key.
///
/// The lazy counterpart of [`member_locate`]: it stops at the first directory that has the
/// key rather than listing every one. The Vault and the Workspace are both looked for from
/// the current directory, since neither is given.
#[must_use]
#[lazyffi(export = rola_member_find)]
pub fn member_find(member_name: &str, rule: &KeyLocateRule) -> Option<Member> {
    auth::find_member(member_name, &roots_here(), rule)
}

/// Every account whose private key `rule` finds, highest priority first.
///
/// An account is named by a private key, which is not shared, so it is only ever looked
/// for in the local roots (beside the Workspace and `vault`, in that order) whatever the
/// rule says about the other scopes. The accounts cross as a set, read one index at a
/// time; see [`Accounts`].
#[must_use]
#[lazyffi(export = rola_account_locate)]
pub fn account_locate(vault: &Vault, rule: &KeyLocateRule) -> Accounts {
    auth::locate_accounts(&roots_beside(vault), rule)
}

/// Every account `rule` finds beside the current Workspace, with no Vault to search beside.
///
/// [`account_locate`] takes a Vault because the Vault's own keys are one of the local roots it reads.
/// A caller that has no Vault used to pass a null pointer there, which the export cannot receive;
/// this is the same search with that root left out, and the rule's other scopes still answer. The
/// Workspace is found from the current directory.
///
/// # FFI
///
/// A `RolaAccounts` the caller releases with `free_rola_accounts`.
#[must_use]
#[lazyffi(export = rola_account_locate_local)]
pub fn account_locate_local(rule: &KeyLocateRule) -> Accounts {
    let workspace = current_workspace();

    auth::locate_accounts(&local_roots(None, workspace.as_ref()), rule)
}

/// Every account whose private key is directly inside `keys`.
///
/// It is the user-scoped counterpart of [`account_locate`]: the caller names the directory that
/// holds the accounts, usually the user's own `rola/<name>/keys`, rather than a Workspace or a
/// Vault. The
/// rule still says which scopes to search, but an account is only ever a private key, so only the
/// local root named here can answer.
#[must_use]
#[lazyffi(export = rola_user_accounts)]
pub fn user_accounts(keys: &Path, rule: &KeyLocateRule) -> Accounts {
    auth::locate_accounts(&[keys.to_path_buf()], rule)
}

/// Every member whose public key is directly inside `keys`.
///
/// It is the user-scoped counterpart of [`member_locate`]. The rule's other scopes are still
/// searched, since a public key is meant to be shared; `keys` is the local root the caller names.
#[must_use]
#[lazyffi(export = rola_user_members)]
pub fn user_members(keys: &Path, rule: &KeyLocateRule) -> Members {
    auth::locate_members(&[keys.to_path_buf()], rule)
}

/// The account named `account_name`, if any scope the rule turns on holds its private key.
///
/// The lazy counterpart of [`account_locate`], which also only looks in the local roots.
/// The Vault and the Workspace are both looked for from the current directory.
#[must_use]
#[lazyffi(export = rola_account_find)]
pub fn account_find(account_name: &str, rule: &KeyLocateRule) -> Option<Account> {
    auth::find_account(account_name, &roots_here(), rule)
}

/// The entries of a tree's structure.
///
/// A tree's structure crosses as its bytes, and the format behind those bytes is this crate's own.
/// A caller that read them back would be keeping a second copy of the format; it hands them here
/// instead, as hex, and reads the entries as data.
#[lazyffi(export = RolaTreeEntries)]
#[derive(Debug, Clone, Default)]
pub struct TreeEntries {
    /// The entries, in the order the structure writes them.
    entries: Vec<TreeEntry>,
}

#[lazyffi(export = rola_)]
impl TreeEntries {
    /// How many entries the structure holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.entries.len()
    }

    /// The entry at `index`, or null past the end.
    #[must_use]
    pub fn at(&self, index: usize) -> Option<TreeEntry> {
        self.entries.get(index).cloned()
    }
}

impl TreeEntries {
    /// The entries of `bytes`, when they decode as a structure this build reads.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let structure = tree::TreeStructure::decode(bytes).ok()?;

        Some(Self {
            entries: structure
                .entries()
                .iter()
                .cloned()
                .map(|entry| TreeEntry { entry })
                .collect(),
        })
    }
}

/// One entry of a tree's structure.
#[lazyffi(export = RolaTreeEntry)]
#[derive(Debug, Clone)]
pub struct TreeEntry {
    /// The entry itself, which holds the path, kind, mode, storage and target.
    entry: tree::TreeEntry,
}

#[lazyffi(export = rola_)]
impl TreeEntry {
    /// The path, tree-root relative and `/` separated.
    #[must_use]
    pub fn path(&self) -> String {
        self.entry.path().to_owned()
    }

    /// What the path is: `file`, `directory` or `symlink`.
    #[must_use]
    pub fn kind(&self) -> String {
        match self.entry.kind() {
            tree::EntryKind::File => "file",
            tree::EntryKind::Directory => "directory",
            tree::EntryKind::Symlink => "symlink",
        }
        .to_owned()
    }

    /// The mode, when the entry is a file: `regular` or `executable`.
    #[must_use]
    pub fn mode(&self) -> String {
        match self.entry.mode() {
            Some(tree::FileMode::Regular) => "regular",
            Some(tree::FileMode::Executable) => "executable",
            None => "",
        }
        .to_owned()
    }

    /// The key a file's content is stored under, or an empty string.
    #[must_use]
    pub fn storage(&self) -> String {
        self.entry
            .storage()
            .map_or_else(String::new, ToString::to_string)
    }

    /// What a symbolic link points at, or an empty string.
    #[must_use]
    pub fn target(&self) -> String {
        self.entry.target().unwrap_or_default().to_owned()
    }
}

/// The entries of a tree's structure, decoded from the hex a caller holds.
///
/// # FFI
///
/// A `RolaTreeEntries *` the caller releases with `free_rola_tree_entries`, or null when the text is
/// not hex or the bytes are not a structure this build reads.
#[must_use]
#[lazyffi(export = rola_tree_entries)]
pub fn tree_entries(structure_hex: &str) -> Option<TreeEntries> {
    let bytes = unhex(structure_hex)?;

    TreeEntries::decode(&bytes)
}

/// The bytes `text` writes as hex, when every pair is a hex digit.
fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }

    let digits = text.as_bytes();
    let mut bytes = Vec::with_capacity(digits.len() / 2);

    for pair in digits.windows(2).step_by(2) {
        let high = char::from(pair[0]).to_digit(16)?;
        let low = char::from(pair[1]).to_digit(16)?;
        bytes.push(u8::try_from(high * 16 + low).ok()?);
    }

    Some(bytes)
}
