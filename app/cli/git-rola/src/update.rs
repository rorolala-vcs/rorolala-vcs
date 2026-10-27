//! The `update` command: resolving `rola.toml` to the content it names, and writing the lock.
//!
//! A mapping names a material by its UUID and says which version to place: the version the Vault's
//! Layout currently points it at (`pin = "latest"`, or a Vault's name), or a number in its chain.
//! What is written down is the content the version names — the mapping and the version are the
//! intent, and the content is what the work tree needs.
//!
//! Resolving is the Vault side, which is this program's to reach and not the git half's: the Vault's
//! Layout is fetched and read for the version each mapping is at, the index is asked for that version
//! and the variant it points at, and the content the variant stored is brought into the project's
//! store. Everything is resolved before the lock is written, and a resolution that fails writes
//! nothing: the lock is only ever the result of a whole pass.
//!
//! The content is brought into the project's own store, `.rola/storage/`, which is the same store the
//! rest of the commands read their materials out of. That is deliberate: the store is
//! content-addressed, so sharing it with the `rola` program is sharing a cache rather than sharing
//! state.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use librorolala::auth::{
    Account, KeyLocateRule, current_account, env_keys_dir, find_account, global_keys_dir,
    user_keys_dir,
};
use librorolala::daemon::{action_fetch_layout, action_read_remote_index, action_sync_hashes};
use librorolala::layout::Layout;
use librorolala::protocol::VaultAddress;
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::{UNKNOWN_VERSION, VCSIndexObject, Variant, Version};
use librorolala::workspace::{Workspace, locate_workspace};
use rola_define_format::{ContentHash, Lock, Material, Vault, VaultName};
use rola_git_support::{Mapping, Pin, Project, ProjectConfig, Session};
use rorolala_utils_constants::{VAULT_LAYOUT_NAME, WORKSPACE_KEYS_DIR};
use rust_i18n::t;

/// The marker a version chain ends at: the variant a first version is based on is the empty hash.
const ROOT_VARIANT: [u8; 32] = [0_u8; 32];

/// How many links of a version chain one resolution may follow before it is taken as endless.
///
/// A chain that long is a broken index rather than a long history, and following it forever would
/// be a run with nothing to report.
const CHAIN_LIMIT: u64 = 100_000;

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

/// What an update came to.
pub struct Outcome {
    /// How many materials the new lock places.
    pub materials: usize,
    /// The materials that could not be placed, each with why.
    pub failures: Vec<(String, String)>,
}

/// Resolves the project the run is inside and writes what it resolves.
///
/// # Errors
///
/// Returns what reading the project fails with, and what resolving its mappings fails with. Nothing
/// is written when resolution fails: the lock is written after the whole of it has been resolved.
pub fn run() -> Result<Outcome, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| t!("git_rola.err_cwd", error = error.to_string()).to_string())?;

    let Some(session) = Session::open(&cwd).map_err(|error| error.to_string())? else {
        return Err(t!("git_rola.err_no_project").to_string());
    };

    let project = session.project();
    let config = project.config().map_err(|error| error.to_string())?;
    let lock = resolve(project, &config)?;

    // What was placed before is taken out first, so a failure here leaves the lock — and so the
    // command — as it was rather than half applied.
    session.detach().map_err(|error| error.to_string())?;

    write_lock(&project.lock_path(), &lock)?;

    let source = crate::source::of(project);
    let failures = session
        .attach(&lock, &source)
        .map_err(|error| error.to_string())?;
    session.exclude(&lock).map_err(|error| error.to_string())?;

    Ok(Outcome {
        materials: lock.materials().len(),
        failures: failures
            .into_iter()
            .map(|(path, error)| (path, error.to_string()))
            .collect(),
    })
}

/// The lock a project's mappings resolve to.
///
/// A project with no mappings places nothing, and resolving it reaches nobody: the empty lock is the
/// answer without a Vault or an account being needed.
///
/// # Errors
///
/// Returns a message naming the mapping, Vault or content that could not be resolved.
fn resolve(project: &Project, config: &ProjectConfig) -> Result<Lock, String> {
    if config.mapping.is_empty() {
        return Ok(Lock::empty());
    }

    let account = account(project)?;
    let vaults = VaultSource::used(config)?;
    let fetch = Fetch::new(project)?;
    let workspace = Workarea::new(project)?;

    // What the run wrote down: every Vault's Layout, fetched once, and the content each mapping
    // resolved to.
    let mut layouts = BTreeMap::new();
    let mut materials = Vec::with_capacity(config.mapping.len());
    let mut contents: Vec<ContentHash> = Vec::with_capacity(config.mapping.len());

    for source in &vaults {
        layouts.insert(
            source.name.clone(),
            fetch_layout(&fetch.workspace, &account, source)?,
        );
    }

    for mapping in &config.mapping {
        let name = vault_for(config, mapping)?;
        let source = vaults
            .iter()
            .find(|source| source.name == name)
            .ok_or_else(|| t!("update.err_undeclared", vault = name.as_str()).to_string())?;

        let layout = layouts
            .get(&name)
            .ok_or_else(|| t!("update.err_no_layout", vault = name.as_str()).to_string())?;

        let held = layout.entry(mapping.from).ok_or_else(|| {
            t!(
                "update.err_not_in_layout",
                material = mapping.from.to_string(),
                vault = name.as_str()
            )
            .to_string()
        })?;
        let tip = held.version();

        let version = match mapping.pin {
            Pin::Latest | Pin::Vault(_) => {
                version_object(&workspace.value, &account, source, &tip)?
            }
            Pin::Version(number) => version_at(&workspace.value, &account, source, &tip, number)?,
        };

        let content = content_of(&workspace.value, &account, source, &version)?;
        contents.push(content);
        materials.push(Material::new(
            mapping.path().map_err(|error| error.to_string())?,
            content,
        ));
    }

    // The content comes to the project's store before the lock says it is placed, so a lock written
    // here is one every command after it can act on.
    bring(&workspace.value, project, &account, &vaults, &contents)?;

    let vaults = vaults
        .into_iter()
        .map(|source| {
            VaultName::new(&source.name)
                .map_err(|error| error.to_string())
                .and_then(|name| {
                    Vault::new(name, &source.address).map_err(|error| error.to_string())
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Lock::new(vaults, materials).map_err(|error| error.to_string())
}

/// The Vault a mapping is resolved against.
///
/// A version number is a place in the material's chain, which is the index's rather than a Vault's,
/// so a numbered pin resolves through the project's default Vault the same way a `latest` pin does.
///
/// # Errors
///
/// Returns a message when a `latest` or numbered pin has no default Vault to resolve through.
fn vault_for(config: &ProjectConfig, mapping: &Mapping) -> Result<String, String> {
    match &mapping.pin {
        Pin::Vault(name) => Ok(name.clone()),
        Pin::Latest | Pin::Version(_) => config
            .default_vault()
            .map(str::to_owned)
            .ok_or_else(|| t!("update.err_no_vault").to_string()),
    }
}

/// The Vaults a project's mappings are resolved against, by name and address.
struct VaultSource {
    /// The name the project knows the Vault by.
    name: String,
    /// The address it answers at, as the project wrote it.
    address: String,
    /// Where the daemon is asked, as the actions name it.
    target: String,
}

impl VaultSource {
    /// The Vault a project's mappings are to be resolved against, sorted by name.
    ///
    /// Only the Vaults some mapping resolves through are read: a project that declares a Vault no
    /// mapping uses does not reach it.
    ///
    /// # Errors
    ///
    /// Returns a message when a mapping names a Vault the project does not declare, or when a
    /// declared address does not read as one.
    fn used(config: &ProjectConfig) -> Result<Vec<Self>, String> {
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
            let Some(address) = config.vault(&name) else {
                return Err(t!("update.err_undeclared", vault = name.as_str()).to_string());
            };

            let target = VaultAddress::parse(address)
                .map_err(|error| {
                    t!(
                        "update.err_address",
                        vault = name.as_str(),
                        error = error.to_string()
                    )
                    .to_string()
                })?
                .to_string();

            sources.push(Self {
                name,
                address: address.to_owned(),
                target,
            });
        }

        Ok(sources)
    }
}

/// The account a run acts as, found the way the rest of rola finds it.
///
/// The name is the one the user's own file carries, and the key is looked for where rola looks: the
/// project's own keys first, then the user's key directories.
///
/// # Errors
///
/// Returns a message when no account is named, or when no scope holds a key by that name.
fn account(project: &Project) -> Result<Account, String> {
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

/// A place to fetch a Vault's Layout into, taken away when the update is done with it.
///
/// The fetched copy is a Vault's Layout as it stands, and `fetch-layout` writes it under a
/// workspace's cache. The whole of `.rola/` belongs to this project's own commands, so the copy is
/// fetched under the git half's private area — `.rola/git/vault/` — rather than into the cache the
/// `rola` program keeps for the same purpose: two programs answering for one project should not be
/// sharing the shape of a directory neither of them owns.
struct Fetch {
    /// The directory the copy lives under, which is removed when the update is done.
    root: PathBuf,
    /// The workspace the copy is written into.
    workspace: Workspace,
}

impl Fetch {
    /// A workspace under the project's private area, made for this update.
    ///
    /// # Errors
    ///
    /// Returns a message when the directory cannot be made or found again.
    fn new(project: &Project) -> Result<Self, String> {
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
struct Workarea {
    /// The project's own workspace, which is where the store is.
    value: Workspace,
}

impl Workarea {
    /// The workspace rooted at the project.
    ///
    /// # Errors
    ///
    /// Returns a message when the project's data directory cannot be made or found.
    fn new(project: &Project) -> Result<Self, String> {
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

/// The Vault's Layout, fetched into the workspace this update keeps for it.
///
/// # Errors
///
/// Returns a message naming the Vault when it cannot be reached, or when what it answered is not a
/// Layout this build reads.
fn fetch_layout(
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
fn version_object(
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
fn variant_object(
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
fn content_of(
    workspace: &Workspace,
    account: &Account,
    source: &VaultSource,
    version: &Version,
) -> Result<ContentHash, String> {
    let variant = variant_object(workspace, account, source, version.variant())?;

    Ok(ContentHash::from_digest(*variant.storage_hash()))
}

/// The version at `number` in the chain that ends at `tip`.
///
/// A version number is not written into a version: it is traced by following the variant each
/// version points at and the version that variant was based on. The Vault traces it as it hands a
/// version over, so each step here is one read of the index.
///
/// # Errors
///
/// Returns a message when the chain does not reach the number, or cannot be read.
fn version_at(
    workspace: &Workspace,
    account: &Account,
    source: &VaultSource,
    tip: &[u8; 32],
    number: u64,
) -> Result<Version, String> {
    let mut version = version_object(workspace, account, source, tip)?;
    let mut followed = 0_u64;

    loop {
        let at = version.version_num();

        if at == UNKNOWN_VERSION {
            return Err(t!("update.err_chain", material = hex(tip), number = number).to_string());
        }

        if at == number {
            return Ok(version);
        }

        // Numbers are one per link, so a chain that is already below the number will not reach it
        // by going further back.
        if at < number {
            return Err(t!("update.err_version", number = number, material = hex(tip)).to_string());
        }

        let variant = variant_object(workspace, account, source, version.variant())?;
        let base = *variant.base_version();

        if base == ROOT_VARIANT {
            return Err(t!("update.err_version", number = number, material = hex(tip)).to_string());
        }

        version = version_object(workspace, account, source, &base)?;

        followed += 1;
        if followed > CHAIN_LIMIT {
            return Err(t!("update.err_chain", material = hex(tip), number = number).to_string());
        }
    }
}

/// Brings the content the resolved materials name into the project's store.
///
/// A key is the hash of its content, so any Vault that holds it hands back the same object: the
/// content is asked of every Vault the mappings resolve through, and what none of them holds is a
/// resolution that cannot be applied — which is refused here, before the lock says it is placed.
///
/// # Errors
///
/// Returns a message naming the Vault when one cannot be reached, and the content when it cannot be
/// had at all.
fn bring(
    workspace: &Workspace,
    project: &Project,
    account: &Account,
    sources: &[VaultSource],
    contents: &[ContentHash],
) -> Result<(), String> {
    let root = project.root().join(".rola").join("storage");
    let store = RorolalaStorage::at(root.clone()).unwrap_or_else(|| RorolalaStorage::create(root));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|error| error.to_string())?;

    let mut keys: Vec<Key> = contents
        .iter()
        .map(|content| Key::new(*content.digest()))
        .collect();
    keys.sort_unstable_by_key(Key::hex);
    keys.dedup_by(|one, other| one.hex() == other.hex());

    let mut missing = absent(&runtime, &store, &keys)?;

    if missing.is_empty() {
        return Ok(());
    }

    for source in sources {
        let units = missing.iter().map(Key::hex).collect::<Vec<_>>().join(" ");

        action_sync_hashes(workspace, account, source.target.clone(), units).map_err(|error| {
            t!(
                "update.err_content",
                vault = source.name.as_str(),
                error = error.to_string()
            )
            .to_string()
        })?;

        missing = absent(&runtime, &store, &missing)?;

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
    store: &RorolalaStorage,
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

/// Writes the lock where the project keeps it, whole rather than half way.
///
/// # Errors
///
/// Returns a message when what is written cannot be, naming the path.
fn write_lock(path: &Path, lock: &Lock) -> Result<(), String> {
    let temporary = path.with_file_name(format!(".rola.lock.{}", std::process::id()));

    fs::write(&temporary, lock.encode()).map_err(|error| {
        t!(
            "update.err_write",
            path = path.display().to_string(),
            error = error.to_string()
        )
        .to_string()
    })?;

    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        // WORKAROUND: Windows refuses to rename over a file that is there, so the old lock is
        // removed first. The lock is this project's own file and no other run writes it while a
        // command is holding it.
        Err(error) if path.exists() => {
            fs::remove_file(path).map_err(|error| error.to_string())?;
            fs::rename(&temporary, path).map_err(|_| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// A hash as the index names it.
fn hex(hash: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut text = String::with_capacity(64);

    for byte in hash {
        // UNWRAP: writing into a String cannot fail, so the `Result` this answers is never one.
        let _ = write!(text, "{byte:02x}");
    }

    text
}
