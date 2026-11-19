//! The Vault side of this program: reaching one, reading what it holds, and bringing that here.
//!
//! Both commands that resolve content against a Vault need the same few things: the account a run
//! acts as, a copy of the Vault's Layout fetched under the project's private area, the index read
//! for a version and the variant it points at, and the content the variant stored brought into the
//! project's store. They live here, once, so `update` and `bring` cannot disagree about what a
//! version means or where content is fetched from.
//!
//! A fetch is temporary: the Layout copy is this run's own, made under `.rola/git/vault/` and
//! removed when the run is done, because the copy the `rola` program keeps for the same purpose is
//! not this program's to share.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use librorolala::auth::{
    Account, KeyLocateRule, current_account, env_keys_dir, find_account, global_keys_dir,
    user_keys_dir,
};
use librorolala::daemon::{
    action_fetch_layout, action_read_remote_index, action_sync_hashes_with_stores_async,
};
use librorolala::layout::Layout;
use librorolala::protocol::VaultAddress;
use librorolala::storage::{Key, RorolalaStorage, StorageSet};
use librorolala::tree::{TreeError, TreeStructure};
use librorolala::vcs::{VCSIndexObject, Variant, Version};
use librorolala::workspace::{Workspace, locate_workspace};
use rola_define_format::ContentHash;
use rola_git_support::{Pin, Project, ProjectConfig};
use rorolala_utils_constants::{VAULT_LAYOUT_NAME, WORKSPACE_KEYS_DIR};
use rust_i18n::t;

/// Which of a key's directories an account is looked for in.
///
/// Every directory is named by value where the search is made, so each is searched once: the scopes
/// that would name one of them again are turned off.
const SEARCH: KeyLocateRule = KeyLocateRule {
    find_global: false,
    find_local: true,
    find_user: false,
    find_env: false,
};

/// The content a tree names inside itself, for every tree among `trees`.
///
/// The structure is read from the store the run has just filled, which is what the first
/// [`bring_contents`] leaves there. Both a structure that cannot be read and one that does not
/// decode are the same failure: the mapping resolved to a version whose content is not a tree this
/// build reads.
///
/// # Errors
///
/// Returns a message naming the material whose structure cannot be read, and what reading it failed
/// with.
pub fn tree_contents(
    store: &StorageSet,
    trees: &[(String, ContentHash)],
) -> Result<Vec<ContentHash>, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;

    let mut contents = Vec::new();

    for (id, content) in trees {
        let failed = |error: String| {
            t!(
                "update.err_structure",
                material = id.as_str(),
                error = error
            )
            .to_string()
        };

        let bytes = runtime
            .block_on(store.read_object(&Key::new(*content.digest())))
            .map_err(|error| failed(error.to_string()))?;
        let keys = tree_content_keys(&bytes).map_err(|error| failed(error.to_string()))?;

        contents.extend(
            keys.into_iter()
                .map(|key| ContentHash::from_digest(*key.digest())),
        );
    }

    Ok(contents)
}

/// The content keys a tree's stored bytes name inside it.
///
/// # Errors
///
/// Returns [`TreeError::Malformed`] when the bytes are not a structure this build wrote.
fn tree_content_keys(bytes: &[u8]) -> Result<Vec<Key>, TreeError> {
    TreeStructure::decode(bytes).map(|structure| structure.content_keys())
}

/// The Vaults a project's mappings are resolved against, by name and address.
pub struct VaultSource {
    /// The name the project knows the Vault by.
    pub name: String,
    /// The address it answers at, as the project wrote it.
    pub address: String,
    /// Where the daemon is asked, as the actions name it.
    pub target: String,
}

impl VaultSource {
    /// The Vault a project declares under `name`.
    ///
    /// # Errors
    ///
    /// Returns a message when the project declares no such Vault, or when the address it wrote is
    /// not one this build reads.
    pub fn named(config: &ProjectConfig, name: &str) -> Result<Self, String> {
        let Some(address) = config.vault(name) else {
            return Err(t!("update.err_undeclared", vault = name).to_string());
        };

        let target = VaultAddress::parse(address)
            .map_err(|error| {
                t!(
                    "update.err_address",
                    vault = name,
                    error = error.to_string()
                )
                .to_string()
            })?
            .to_string();

        Ok(Self {
            name: name.to_owned(),
            address: address.to_owned(),
            target,
        })
    }

    /// The Vaults a project's mappings are to be resolved against, sorted by name.
    ///
    /// Only the Vaults some mapping resolves through are read: a project that declares a Vault no
    /// mapping uses does not reach it.
    ///
    /// # Errors
    ///
    /// Returns a message when a mapping names a Vault the project does not declare, or when a
    /// declared address does not read as one.
    pub fn used(config: &ProjectConfig) -> Result<Vec<Self>, String> {
        let mut names = BTreeSet::new();

        for mapping in &config.mapping {
            names.insert(match &mapping.pin {
                Pin::Vault(name) => name.clone(),
                Pin::Latest | Pin::Version(_) => config
                    .default_vault()
                    .map(str::to_owned)
                    .ok_or_else(|| t!("update.err_no_vault").to_string())?,
            });
        }

        let mut sources = Vec::with_capacity(names.len());

        for name in names {
            sources.push(Self::named(config, &name)?);
        }

        Ok(sources)
    }
}

/// The Vault a project's mappings go through when the run names none.
///
/// The answer is the project's `defaults.vault`, then `origin` when the project declares one, then
/// the first Vault it declares. A project declaring none has no answer, which is what a run asking
/// for a Vault has to be told rather than be given one it did not name.
#[must_use]
pub fn vault_name(config: &ProjectConfig, asked: Option<&str>) -> Option<String> {
    asked
        .map(str::to_owned)
        .or_else(|| config.defaults.vault.clone())
        .or_else(|| config.vault("origin").map(|_| "origin".to_owned()))
        .or_else(|| config.default_vault().map(str::to_owned))
}

/// The account a run acts as, found the way the rest of rola finds it.
///
/// The name is the one the user's own file carries, and the key is looked for where rola looks: the
/// project's own keys first, then the user's key directories.
///
/// # Errors
///
/// Returns a message when no account is named, or when no scope holds a key by that name.
pub fn account(project: &Project) -> Result<Account, String> {
    let name = current_account().ok_or_else(|| t!("update.err_no_account").to_string())?;

    let mut roots = vec![
        project
            .root()
            .join(WORKSPACE_KEYS_DIR)
            .components()
            .collect::<PathBuf>(),
    ];
    roots.extend(user_keys_dir());
    roots.extend(global_keys_dir());
    roots.extend(env_keys_dir());

    find_account(&name, &roots, &SEARCH)
        .ok_or_else(|| t!("update.err_no_key", name = name.as_str()).to_string())
}

/// A place to fetch a Vault's Layout into, taken away when the run is done with it.
///
/// The fetched copy is a Vault's Layout as it stands, and `fetch-layout` writes it under a
/// workspace's cache. The whole of `.rola/` belongs to this project's own commands, so the copy is
/// fetched under the git half's private area, `.rola/git/vault/`, rather than into the cache the
/// `rola` program keeps for the same purpose: two programs answering for one project should not be
/// sharing the shape of a directory neither of them owns.
pub struct Fetch {
    /// The directory the copy lives under, which is removed when the run is done.
    root: PathBuf,
    /// The workspace the copy is written into.
    pub workspace: Workspace,
}

impl Fetch {
    /// A workspace under the project's private area, made for this run.
    ///
    /// # Errors
    ///
    /// Returns a message when the directory cannot be made or found again.
    pub fn new(project: &Project) -> Result<Self, String> {
        let root = project.root().join(".rola").join("git").join("vault");
        let error = || t!("update.err_workarea", path = root.display().to_string()).to_string();

        fs::create_dir_all(root.join(".rola")).map_err(|_| error())?;

        let workspace = locate_workspace(&root).ok_or_else(error)?;

        Ok(Self { root, workspace })
    }
}

impl Drop for Fetch {
    fn drop(&mut self) {
        // The copy is this run's own and is fetched again next time; one that cannot be taken away
        // costs space rather than correctness.
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The project as the daemon actions see it: the store they write content into.
pub struct Workarea {
    /// The project's own workspace, which is where the store is.
    pub value: Workspace,
}

impl Workarea {
    /// The workspace rooted at the project.
    ///
    /// # Errors
    ///
    /// Returns a message when the project's data directory cannot be made or found.
    pub fn new(project: &Project) -> Result<Self, String> {
        let error = || {
            t!(
                "update.err_workarea",
                path = project.root().display().to_string()
            )
            .to_string()
        };

        fs::create_dir_all(project.root().join(".rola")).map_err(|_| error())?;

        locate_workspace(project.root())
            .map(|value| Self { value })
            .ok_or_else(error)
    }
}

/// The Vault's Layout, fetched into the workspace this run keeps for it.
///
/// # Errors
///
/// Returns a message naming the Vault when it cannot be reached, or when what it answered is not a
/// Layout this build reads.
pub fn fetch_layout(
    workspace: &Workspace,
    account: &Account,
    source: &VaultSource,
) -> Result<Layout, String> {
    action_fetch_layout(
        workspace,
        account,
        source.target.clone(),
        source.name.clone(),
    )
    .map_err(|error| {
        t!(
            "update.err_fetch",
            vault = source.name.as_str(),
            error = error.to_string()
        )
        .to_string()
    })?;

    let dir = workspace
        .readonly_layout_copy(&source.name, VAULT_LAYOUT_NAME)
        .ok_or_else(|| t!("update.err_no_layout", vault = source.name.as_str()).to_string())?;

    Layout::open_existing(dir).map_err(|error| {
        t!(
            "update.err_layout",
            vault = source.name.as_str(),
            error = error.to_string()
        )
        .to_string()
    })
}

/// One object the Vault's index holds.
///
/// # Errors
///
/// Returns a message naming the Vault when it cannot be read.
fn object(
    workspace: &Workspace,
    account: &Account,
    source: &VaultSource,
    hash: &[u8; 32],
) -> Result<VCSIndexObject, String> {
    action_read_remote_index(workspace, account, source.target.clone(), hex(hash)).map_err(
        |error| {
            t!(
                "update.err_index",
                vault = source.name.as_str(),
                error = error.to_string()
            )
            .to_string()
        },
    )
}

/// The version the index holds under `hash`.
///
/// # Errors
///
/// Returns a message when what is held there is not a version.
pub fn version_object(
    workspace: &Workspace,
    account: &Account,
    source: &VaultSource,
    hash: &[u8; 32],
) -> Result<Version, String> {
    match object(workspace, account, source, hash)? {
        VCSIndexObject::Version(version) => Ok(version),
        other => Err(t!("update.err_not_a_version", object = format!("{other:?}")).to_string()),
    }
}

/// The variant the index holds under `hash`.
///
/// # Errors
///
/// Returns a message when what is held there is not a variant.
pub fn variant_object(
    workspace: &Workspace,
    account: &Account,
    source: &VaultSource,
    hash: &[u8; 32],
) -> Result<Variant, String> {
    match object(workspace, account, source, hash)? {
        VCSIndexObject::Variant(variant) => Ok(variant),
        other => Err(t!("update.err_not_a_version", object = format!("{other:?}")).to_string()),
    }
}

/// The content a version names: the storage key of the variant it points at.
///
/// # Errors
///
/// Returns a message when either object cannot be read.
pub fn content_of(
    workspace: &Workspace,
    account: &Account,
    source: &VaultSource,
    version: &Version,
) -> Result<ContentHash, String> {
    let variant = variant_object(workspace, account, source, version.variant())?;

    Ok(ContentHash::from_digest(*variant.storage_hash()))
}

/// The stores a project works on.
///
/// The project's own `.rola/storage` is the last store it reads; the ones `rola.toml` names under
/// `[[storages]]` come first, in the order it writes them. The set answers with none when the
/// project has neither, which is a project whose materials can only be put back from the staging
/// area; a write through it makes the own store.
///
/// # Errors
///
/// Returns a message when `rola.toml` cannot be read.
pub fn project_stores(project: &Project) -> Result<StorageSet, String> {
    let own_root = project.root().join(".rola").join("storage");
    let own = RorolalaStorage::at(own_root.clone());
    let config = project.config().map_err(|error| error.to_string())?;
    let paths: Vec<String> = config.storage_paths().map(str::to_owned).collect();

    Ok(StorageSet::with_bindings(
        own,
        own_root,
        project.root(),
        paths,
        Some(project.root()),
    ))
}

/// Brings the content the resolved materials name into the project's stores.
///
/// A key is the hash of its content, so any Vault that holds it hands back the same object: the
/// content is asked of every Vault the run resolves through, and what none of them holds is a
/// resolution that cannot be applied, which is refused here, before the lock says it is placed.
///
/// # Errors
///
/// Returns a message naming the Vault when one cannot be reached, and the content when it cannot be
/// had at all.
pub fn bring_contents(
    store: &StorageSet,
    workspace: &Workspace,
    account: &Account,
    sources: &[VaultSource],
    contents: &[ContentHash],
) -> Result<(), String> {
    // The action connects to the Vault over TCP, so the runtime has to carry IO as well as a
    // timer: a current-thread runtime with neither panics inside the socket call.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;

    let mut keys: Vec<Key> = contents
        .iter()
        .map(|content| Key::new(*content.digest()))
        .collect();
    keys.sort_unstable_by_key(Key::hex);
    keys.dedup_by(|one, other| one.hex() == other.hex());

    let mut missing = absent(&runtime, store, &keys)?;

    if missing.is_empty() {
        return Ok(());
    }

    for source in sources {
        let units = missing.iter().map(Key::hex).collect::<Vec<_>>().join(" ");

        runtime
            .block_on(action_sync_hashes_with_stores_async(
                workspace,
                account,
                source.target.clone(),
                units,
                store,
                rorolala_utils_progress::Progress::silent(),
            ))
            .map_err(|error| {
                t!(
                    "update.err_content",
                    vault = source.name.as_str(),
                    error = error.to_string()
                )
                .to_string()
            })?;

        missing = absent(&runtime, store, &missing)?;

        if missing.is_empty() {
            return Ok(());
        }
    }

    let wanted = missing.iter().map(Key::hex).collect::<Vec<_>>().join(", ");

    Err(t!("update.err_missing", content = wanted).to_string())
}

/// Which of `keys` the store does not hold.
/// # Errors
///
/// Returns a message when the store cannot be asked.
fn absent(
    runtime: &tokio::runtime::Runtime,
    store: &StorageSet,
    keys: &[Key],
) -> Result<Vec<Key>, String> {
    let presence = runtime
        .block_on(store.contains_keys(keys))
        .map_err(|error| error.to_string())?;

    Ok(keys
        .iter()
        .zip(presence.iter())
        .filter_map(|(key, held)| (!held).then_some(*key))
        .collect())
}

/// Writes `text` to `path`, whole rather than half way.
///
/// A temporary file beside the target is written first and renamed over it, so a run that cannot
/// write the whole of it leaves what was there as it was.
///
/// # Errors
///
/// Returns a message naming the path when what is written cannot be.
pub fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    let name = path.file_name().map_or_else(
        || "rola".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let temporary = path.with_file_name(format!(".{name}.{}", std::process::id()));

    fs::write(&temporary, text).map_err(|error| {
        t!(
            "update.err_write",
            path = path.display().to_string(),
            error = error.to_string()
        )
        .to_string()
    })?;

    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        // WORKAROUND: Windows refuses to rename over a file that is there, so the old file is
        // removed first. The lock and the project file are this project's own, and no other run
        // writes either while a command is holding it.
        Err(_) if path.exists() => {
            fs::remove_file(path).map_err(|error| error.to_string())?;
            fs::rename(&temporary, path).map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// A hash as the index names it.
pub fn hex(hash: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut text = String::with_capacity(64);

    for byte in hash {
        // UNWRAP: writing into a String cannot fail, so the `Result` this answers is never one.
        let _ = write!(text, "{byte:02x}");
    }

    text
}

#[cfg(test)]
mod tests {
    use librorolala::storage::Key;
    use librorolala::tree::{FileMode, TreeEntry, TreeStructure};

    use super::tree_content_keys;

    /// The keys a structure names are the description's and its files', description first.
    #[test]
    fn the_keys_a_structure_names_are_the_description_and_its_files() {
        let description = Key::new([9; 32]);
        let first = Key::new([1; 32]);
        let second = Key::new([2; 32]);
        let structure = TreeStructure::build(
            Some(description),
            [
                TreeEntry::file("a.png", FileMode::Regular, first),
                TreeEntry::directory("sub"),
                TreeEntry::file("sub/b.psd", FileMode::Regular, second),
            ],
        )
        .unwrap();

        let keys = tree_content_keys(&structure.encode()).unwrap();

        assert_eq!(keys, [description, first, second]);
    }

    /// Bytes that are not a structure are refused rather than read as one.
    #[test]
    fn bytes_that_are_not_a_structure_are_refused() {
        assert!(tree_content_keys(b"not a structure").is_err());
    }
}
