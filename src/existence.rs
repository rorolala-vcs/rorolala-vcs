//! Which Vaults can provide the content a variant, a version or a Layout entry names.
//!
//! A key is the hash of its content, so any Vault holding it hands back the same content, and which
//! of them holds it is a question only the Vaults answer. This is the asking: every Vault the
//! Workspace has bound is asked what its store holds, and what a target needs is looked for in every
//! answer.
//!
//! It is the network half of the existence reading. The local half is [`rorolala_status`]'s, and it
//! says whether the content is here and, when the content is a tree, which keys that structure
//! names. What a Vault's own index names is read from that Vault only when this side cannot resolve
//! the target: content that a Vault holds but whose version or variant this side never fetched is
//! still reachable from the Vault that can name it.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr as _;

use rorolala_auth::{Account, KeyLocateRule};
use rorolala_daemon::{action_list_remote_async, action_read_remote_index_async};
use rorolala_layout::Layout;
use rorolala_protocol::VaultAddress;
use rorolala_status::tracked_vault;
use rorolala_status::{AskedTarget, Existence, ExistenceError, Target, local};
use rorolala_storage::Key;
use rorolala_utils_configure::Configure as _;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rorolala_vcs::{VCSIndex, VCSIndexObject};
use rorolala_workspace::{Config as WorkspaceConfig, Workspace};
use serde::{Deserialize, Serialize};

use crate::auth;

/// The listing word that asks a store for the objects it holds.
///
/// It is what `rola storage ls-remote-storaged` asks for, so a Vault that answers that answers this.
const STORAGED: &str = "storaged";

/// What asking the Vaults came to for one target.
#[derive(Debug, Clone)]
pub struct Sourced {
    /// What the target is, and whether it is held here.
    pub existence: Existence,
    /// The names of the Vaults whose store holds every object the target needs.
    pub sources: Vec<String>,
    /// The names of the Vaults that could not be asked.
    pub unreachable: Vec<String>,
}

impl Sourced {
    /// Whether the target's content is here, with every file a tree names.
    #[must_use]
    pub const fn offline(&self) -> bool {
        self.existence.held
    }

    /// Whether the tree's files were checked, which is false when its structure was not here to say
    /// what they are.
    #[must_use]
    pub const fn expanded(&self) -> bool {
        self.existence.contents.is_some()
    }
}

/// Asks `candidates` which of `targets` each of them can provide.
///
/// The answer is one value per target, in the order the targets were given. A Vault that cannot be
/// reached is named in [`unreachable`](Sourced::unreachable) rather than ending the search: one
/// Vault being down says nothing about the rest.
///
/// A target whose version or variant this side does not hold is resolved from the Vault's own index,
/// pair by pair, so a Vault can be found even for content this side has never named. What such a
/// target is, whether it is a tree or a file, is not known without reading its content, so its
/// answer covers the content's own key and not a tree's files.
pub fn search(
    runtime: &tokio::runtime::Runtime,
    workspace: &Workspace,
    layout: &Layout,
    index: &VCSIndex,
    account: &Account,
    candidates: &[(String, VaultAddress)],
    targets: &[Target],
) -> Vec<Sourced> {
    let store = workspace.get_current_rola_storage();
    let existence = runtime.block_on(local(layout, index, store.as_ref(), targets));

    let mut sourced: Vec<Sourced> = existence
        .into_iter()
        .map(|existence| Sourced {
            existence,
            sources: Vec::new(),
            unreachable: Vec::new(),
        })
        .collect();

    for (name, address) in candidates {
        let Some(held) = listing(runtime, workspace, account, address) else {
            for one in &mut sourced {
                one.unreachable.push(name.clone());
            }

            continue;
        };

        for (one, target) in sourced.iter_mut().zip(targets) {
            let Some(needed) = needed(runtime, workspace, account, address, target, &one.existence)
            else {
                continue;
            };

            if needed.iter().all(|key| held.contains(key)) {
                one.sources.push(name.clone());
            }
        }
    }

    sourced
}

/// What a Vault's store holds, or nothing when the Vault could not be asked.
fn listing(
    runtime: &tokio::runtime::Runtime,
    workspace: &Workspace,
    account: &Account,
    address: &VaultAddress,
) -> Option<BTreeSet<Key>> {
    let text = runtime
        .block_on(action_list_remote_async(
            workspace,
            account,
            address.to_string(),
            STORAGED.to_owned(),
            Progress::silent(),
        ))
        .ok()?;

    Some(
        text.lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| Key::from_str(line).ok())
            .collect(),
    )
}

/// Every key the Vault must hold for `target`, or nothing when it cannot name the target at all.
fn needed(
    runtime: &tokio::runtime::Runtime,
    workspace: &Workspace,
    account: &Account,
    address: &VaultAddress,
    target: &Target,
    existence: &Existence,
) -> Option<Vec<Key>> {
    // What this side resolved is the whole of what the target needs: the content, and the files a
    // tree names when the structure was here to say what they are.
    if let Some(storage) = existence.storage {
        let mut needed = vec![storage];

        if let Some(contents) = &existence.contents {
            needed.extend(contents.iter().copied());
        }

        return Some(needed);
    }

    let storage = match target {
        Target::Storage(key) => *key,
        Target::Variant(key) => variant_storage(runtime, workspace, account, address, *key)?,
        Target::Version(key) | Target::Unknown(key) => {
            match object(runtime, workspace, account, address, *key)? {
                VCSIndexObject::Version(version) => {
                    let variant = *version.variant();
                    if variant == [0_u8; 32] {
                        return None;
                    }

                    variant_storage(runtime, workspace, account, address, Key::new(variant))?
                }
                VCSIndexObject::Variant(variant) => Key::new(*variant.storage_hash()),
                _ => return None,
            }
        }
        Target::Entry(_) => {
            // The Layout records the version even when the object is not here, which is what a
            // search is for: the Vault's index may hold it.
            let version = existence.version?;

            match object(runtime, workspace, account, address, version)? {
                VCSIndexObject::Version(version) => {
                    let variant = *version.variant();
                    if variant == [0_u8; 32] {
                        return None;
                    }

                    variant_storage(runtime, workspace, account, address, Key::new(variant))?
                }
                _ => return None,
            }
        }
    };

    Some(vec![storage])
}

/// The content key of the variant `key`, read from this side when it is here and from the Vault when
/// it is not.
fn variant_storage(
    runtime: &tokio::runtime::Runtime,
    workspace: &Workspace,
    account: &Account,
    address: &VaultAddress,
    key: Key,
) -> Option<Key> {
    match object(runtime, workspace, account, address, key)? {
        VCSIndexObject::Variant(variant) => Some(Key::new(*variant.storage_hash())),
        _ => None,
    }
}

/// The index object `key` names, from the Vault.
fn object(
    runtime: &tokio::runtime::Runtime,
    workspace: &Workspace,
    account: &Account,
    address: &VaultAddress,
    key: Key,
) -> Option<VCSIndexObject> {
    runtime
        .block_on(action_read_remote_index_async(
            workspace,
            account,
            address.to_string(),
            key.hex(),
            Progress::silent(),
        ))
        .ok()
}

/// The request the C ABI carries.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Request {
    /// The targets, in the order the answer keeps.
    targets: Vec<AskedTarget>,
    /// The account to ask as, or nothing for the user's current account.
    account: Option<String>,
}

/// What the C ABI answers with.
#[derive(Debug, Serialize)]
struct Answer {
    /// One answer per target, in the order they were asked about.
    targets: Vec<AnswerTarget>,
}

/// What one target came to, as the C ABI writes it.
#[derive(Debug, Serialize)]
struct AnswerTarget {
    /// What kind the target was.
    kind: &'static str,
    /// The hash it was written with, for a variant or a version.
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
    /// The path it was written with, for an entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// The version it names or records, as hex.
    version: Option<String>,
    /// The variant that version fixes, as hex.
    variant: Option<String>,
    /// The key of the content, as hex.
    storage: Option<String>,
    /// Whether every object it needs is held here.
    offline: bool,
    /// Whether its content is a tree structure.
    tree: bool,
    /// Whether a tree's files were checked, which needs its structure to be here.
    expanded: bool,
    /// The names of the Vaults whose store holds every object it needs.
    sources: Vec<String>,
    /// The names of the Vaults that could not be asked.
    unreachable: Vec<String>,
}

impl AnswerTarget {
    /// One answer for `asked`, from what the search came to.
    fn new(asked: &AskedTarget, sourced: &Sourced) -> Self {
        let (hash, path) = match asked {
            AskedTarget::Variant { hash } | AskedTarget::Version { hash } => {
                (Some(hash.clone()), None)
            }
            AskedTarget::Entry { path } => (None, Some(path.clone())),
        };
        let existence = &sourced.existence;

        Self {
            kind: asked.kind(),
            hash,
            path,
            version: existence.version.map(|key| key.hex()),
            variant: existence.variant.map(|key| key.hex()),
            storage: existence.storage.map(|key| key.hex()),
            offline: existence.held,
            tree: existence.tree,
            expanded: sourced.expanded(),
            sources: sourced.sources.clone(),
            unreachable: sourced.unreachable.clone(),
        }
    }
}

/// Which Vaults can provide what a variant, a version or a Layout entry names.
///
/// The request names the targets and, when it names one, the account to ask as; without one the
/// user's current account is asked as. Every Vault the Workspace has bound is asked what its store
/// holds, and a target is answered with the Vaults holding every object it needs. Content this side
/// does not hold is named by the Vault's own index, so a Vault can be found for a version this side
/// never fetched.
///
/// # FFI
///
/// `directory` is the directory the Workspace is searched upwards from, and `request` is a UTF-8
/// JSON document of this shape:
///
/// ```json
/// {"targets": [{"kind": "version", "hash": "blake3:..."}], "account": "alice"}
/// ```
///
/// `account` is optional. The answer is an owned JSON string the caller releases with
/// `free_string`, `{"targets": [...]}` in the order asked, each with `kind`, the `hash` or `path`,
/// `version`, `variant`, `storage`, `offline`, `tree`, `expanded`, `sources` and `unreachable`. A
/// failure is a `RolaExistenceError *` the caller releases with `free_rola_existence_error`.
///
/// # Errors
///
/// Returns [`ExistenceError::NoWorkspace`] when no Workspace holds `directory`,
/// [`ExistenceError::NoLayout`] when the Workspace works in no Layout,
/// [`ExistenceError::NoIndex`] when it holds no version control index,
/// [`ExistenceError::BadRequest`] when `request` names a target this build cannot read,
/// [`ExistenceError::NoCurrentAccount`] when no account was named and the user's current account
/// names none, [`ExistenceError::NoAccount`] when the account named is not here, and
/// [`ExistenceError::Read`] when the Workspace's configuration or the answer could not be read.
#[lazyffi(export = rola_search_source_json)]
pub fn search_source_json(directory: &Path, request: &str) -> Result<String, ExistenceError> {
    let workspace = Workspace::locate(directory).ok_or(ExistenceError::NoWorkspace)?;
    let layout = workspace
        .layout_if_there()
        .map_err(|error| ExistenceError::Read {
            reason: error.to_string(),
        })?
        .ok_or(ExistenceError::NoLayout)?;
    let index = VCSIndex::locate(directory).ok_or(ExistenceError::NoIndex)?;
    let request: Request =
        serde_json::from_str(request).map_err(|error| ExistenceError::BadRequest {
            reason: error.to_string(),
        })?;

    let name = request
        .account
        .clone()
        .or_else(auth::current_account)
        .ok_or(ExistenceError::NoCurrentAccount)?;
    let account = auth::find_account(
        &name,
        &crate::local_roots(None, Some(&workspace)),
        &KeyLocateRule::new(),
    )
    .ok_or(ExistenceError::NoAccount { name })?;

    let mut targets = Vec::with_capacity(request.targets.len());
    for asked in &request.targets {
        targets.push(asked.target()?);
    }

    let candidates = candidates(&workspace)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| ExistenceError::Read {
        reason: error.to_string(),
    })?;
    let sourced = search(
        &runtime,
        &workspace,
        &layout,
        &index,
        &account,
        &candidates,
        &targets,
    );

    serde_json::to_string(&Answer {
        targets: request
            .targets
            .iter()
            .zip(&sourced)
            .map(|(asked, sourced)| AnswerTarget::new(asked, sourced))
            .collect(),
    })
    .map_err(|error| ExistenceError::Read {
        reason: error.to_string(),
    })
}

/// The Vaults the Workspace has bound, the one its Layout tracks first.
///
/// It is the order `fetch::Sources` asks in: the Vault the content belongs to is asked before the
/// ones it may merely be found in, and the rest keep the name order the Workspace does not.
fn candidates(workspace: &Workspace) -> Result<Vec<(String, VaultAddress)>, ExistenceError> {
    let config = WorkspaceConfig::read_from(&workspace.config_path()).map_err(|error| {
        ExistenceError::Read {
            reason: error.to_string(),
        }
    })?;
    let primary =
        tracked_vault(workspace).or_else(|| config.default_config().vault().map(str::to_owned));

    let mut named: Vec<(&String, &String)> = config.vaults().iter().collect();
    named.sort_by(|left, right| left.0.cmp(right.0));

    let mut candidates = Vec::with_capacity(named.len());
    let mut first: Option<String> = None;

    if let Some(primary) = primary
        && let Some((name, address)) = named.iter().find(|(name, _)| **name == primary)
        && let Ok(address) = VaultAddress::from_str(address)
    {
        first = Some((*name).clone());
        candidates.push(((*name).clone(), address));
    }

    for (name, address) in named {
        if first.as_deref() == Some(name.as_str()) {
            continue;
        }

        // A binding that does not read as an address is not a Vault anything can ask; it is left out
        // rather than ending a search that the other bindings can still answer.
        if let Ok(address) = VaultAddress::from_str(address) {
            candidates.push((name.clone(), address));
        }
    }

    Ok(candidates)
}
