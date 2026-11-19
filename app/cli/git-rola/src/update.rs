//! The `update` command: resolving `rola.toml` to the content it names, and writing the lock.
//!
//! A mapping names a material by its UUID and says which version to place: the version the Vault's
//! Layout currently points it at (`pin = "latest"`, or a Vault's name), or a number in its chain.
//! What is written down is the content the version names, the mapping and the version are the
//! intent, and the content is what the work tree needs.
//!
//! Resolving is the Vault side, which is this program's to reach and not the git half's: the Vault's
//! Layout is fetched and read for the version each mapping is at, the index is asked for that version
//! and the variant it points at, and the content the variant stored is brought into the project's
//! store. Everything is resolved before the lock is written, and a resolution that fails writes
//! nothing: the lock is only ever the result of a whole pass. The reaching itself is
//! [`upstream`]'s, shared with `bring`.
//!
//! The content is brought into the project's own store, `.rola/storage/`, which is the same store the
//! rest of the commands read their materials out of. That is deliberate: the store is
//! content-addressed, so sharing it with the `rola` program is sharing a cache rather than sharing
//! state.

use std::collections::BTreeMap;

use librorolala::auth::Account;
use librorolala::layout::EntryKind;
use librorolala::vcs::{UNKNOWN_VERSION, Version};
use librorolala::workspace::Workspace;
use rola_define_format::{ContentHash, Lock, LockPath, Material, MaterialKind, Vault, VaultName};
use rola_git_support::{Mapping, Pin, Project, ProjectConfig, Session};
use rust_i18n::t;

use crate::upstream;

/// The marker a version chain ends at: the variant a first version is based on is the empty hash.
const ROOT_VARIANT: [u8; 32] = [0_u8; 32];

/// How many links of a version chain one resolution may follow before it is taken as endless.
///
/// A chain that long is a broken index rather than a long history, and following it forever would
/// be a run with nothing to report.
const CHAIN_LIMIT: u64 = 100_000;

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

    // What was placed before is taken out first, so a failure here leaves the lock, and so the
    // command, as it was rather than half applied. A tree is checked against the structure it was
    // placed from, which is the store's, so the source is made before the taking out.
    let source = crate::source::of(project)?;

    session.detach(&source).map_err(|error| error.to_string())?;

    upstream::write_atomically(&project.lock_path(), &lock.encode())?;

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

    let account = upstream::account(project)?;
    let vaults = upstream::VaultSource::used(config)?;
    let fetch = upstream::Fetch::new(project)?;
    let workspace = upstream::Workarea::new(project)?;

    // What the run wrote down: every Vault's Layout, fetched once, and the content each mapping
    // resolved to.
    let mut layouts = BTreeMap::new();
    let mut resolved: Vec<Resolved> = Vec::with_capacity(config.mapping.len());

    for source in &vaults {
        layouts.insert(
            source.name.clone(),
            upstream::fetch_layout(&fetch.workspace, &account, source)?,
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
        let kind = match held.kind() {
            EntryKind::File => MaterialKind::File,
            EntryKind::Tree => MaterialKind::Tree,
        };

        let version = match mapping.pin {
            Pin::Latest | Pin::Vault(_) => {
                upstream::version_object(&workspace.value, &account, source, &tip)?
            }
            Pin::Version(number) => version_at(&workspace.value, &account, source, &tip, number)?,
        };

        let content = upstream::content_of(&workspace.value, &account, source, &version)?;
        resolved.push(Resolved {
            id: mapping.from.to_string(),
            path: mapping.path().map_err(|error| error.to_string())?,
            kind,
            content,
        });
    }

    // The content comes to the project's store before the lock says it is placed, so a lock written
    // here is one every command after it can act on. A tree's structure is the content its version
    // names, and what the structure names is a second ask: the structure is what says which keys
    // the inside is made of, so it crosses before they can be named.
    let store = upstream::project_stores(project)?;
    let contents: Vec<ContentHash> = resolved.iter().map(|entry| entry.content).collect();
    upstream::bring_contents(&store, &workspace.value, &account, &vaults, &contents)?;

    let trees: Vec<(String, ContentHash)> = resolved
        .iter()
        .filter(|entry| entry.kind == MaterialKind::Tree)
        .map(|entry| (entry.id.clone(), entry.content))
        .collect();

    if !trees.is_empty() {
        let inside = upstream::tree_contents(&store, &trees)?;
        upstream::bring_contents(&store, &workspace.value, &account, &vaults, &inside)?;
    }

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

    let materials = resolved
        .into_iter()
        .map(|entry| Material::new(entry.path, entry.content, entry.kind))
        .collect();

    Lock::new(vaults, materials).map_err(|error| error.to_string())
}

/// One mapping, resolved to what the lock places: the `Uuid` it names, where it lands, what lands
/// there, and the content it holds.
struct Resolved {
    /// The `Uuid` the mapping names, as the run says it.
    id: String,
    /// Where the content is placed in the project.
    path: LockPath,
    /// What the content is placed as.
    kind: MaterialKind,
    /// The content the version names.
    content: ContentHash,
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
    source: &upstream::VaultSource,
    tip: &[u8; 32],
    number: u64,
) -> Result<Version, String> {
    let mut version = upstream::version_object(workspace, account, source, tip)?;
    let mut followed = 0_u64;

    loop {
        let at = version.version_num();

        if at == UNKNOWN_VERSION {
            return Err(t!(
                "update.err_chain",
                material = upstream::hex(tip),
                number = number
            )
            .to_string());
        }

        if at == number {
            return Ok(version);
        }

        // Numbers are one per link, so a chain that is already below the number will not reach it
        // by going further back.
        if at < number {
            return Err(t!(
                "update.err_version",
                number = number,
                material = upstream::hex(tip)
            )
            .to_string());
        }

        let variant = upstream::variant_object(workspace, account, source, version.variant())?;
        let base = *variant.base_version();

        if base == ROOT_VARIANT {
            return Err(t!(
                "update.err_version",
                number = number,
                material = upstream::hex(tip)
            )
            .to_string());
        }

        version = upstream::version_object(workspace, account, source, &base)?;

        followed += 1;
        if followed > CHAIN_LIMIT {
            return Err(t!(
                "update.err_chain",
                material = upstream::hex(tip),
                number = number
            )
            .to_string());
        }
    }
}
