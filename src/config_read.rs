//! The configuration a place holds, as the C ABI reads it.
//!
//! A Workspace's configuration is where the Vaults it knows are bound, which name it reaches for,
//! and what a collecting run is told; a Vault's is what it says about itself, who may administer it,
//! and what its daemon may serve. Both are written as files by [`rorolala_utils_configure`], and the
//! reading here is the same one the commands make before they act.
//!
//! What crosses is JSON. A binding is a name and an address, an administrator is a name, and both
//! are lists: one export per configuration keeps the ABI smaller than a reader per field, and lets
//! the shape stay the one `rola vault ls --json` already writes.

use std::path::Path;

use rorolala_daemon::{Namespaces, registry_path};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vault::{Config as VaultConfig, Vault};
use rorolala_workspace::{Config as WorkspaceConfig, Workspace};
use serde::Serialize;

/// One Vault a Workspace knows, as `rola vault ls` shows it.
#[derive(Debug, Serialize)]
struct VaultItem {
    /// The name the Workspace knows it by.
    name: String,
    /// The address it answers at.
    address: String,
    /// Whether it is the Vault the Workspace reaches for.
    is_default: bool,
}

/// What a collecting run is told, as the Workspace's configuration says it.
#[derive(Debug, Serialize)]
struct GcItem {
    /// How long an entry nothing references is kept, in seconds, when the file says.
    grace: Option<u64>,
}

/// A Workspace's configuration, as the C ABI writes it.
#[derive(Debug, Serialize)]
struct WorkspaceAnswer {
    /// Each Vault, in name order.
    vaults: Vec<VaultItem>,
    /// The Vault the Workspace reaches for, when one has been chosen.
    default: Option<String>,
    /// What a collecting run in this Workspace is told, when the configuration says.
    gc: Option<GcItem>,
}

/// The Vaults the Workspace holding `directory` knows, as `rola vault ls --json` writes them.
///
/// Each Vault is its name, its address and whether it is the one the Workspace reaches for. The
/// answer is a superset of what `vault ls` writes: it also carries `default`, the name the
/// Workspace reaches for, and `gc`, what a collecting run in it is told. A directory no Workspace
/// holds, a Workspace with no configuration, and a configuration that cannot be read are all
/// answered with the JSON `null`.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"vaults":[{"name","address","is_default"}],"default":null,"gc":null}`, or `null` when there is
/// no configuration to read. `vaults` is in name order, `is_default` marks the one `default` names,
/// and `gc` is `null` or `{"grace":N}`.
#[must_use]
#[lazyffi(export = rola_workspace_config_json)]
pub fn workspace_config_json(directory: &Path) -> String {
    let Some(workspace) = Workspace::locate(directory) else {
        return "null".to_owned();
    };
    let Ok(config) = WorkspaceConfig::read_from(&workspace.config_path()) else {
        return "null".to_owned();
    };

    let default = config.default_config().vault().map(str::to_owned);
    let mut vaults: Vec<VaultItem> = config
        .vaults()
        .iter()
        .map(|(name, address)| VaultItem {
            name: name.clone(),
            address: address.clone(),
            is_default: default.as_deref() == Some(name.as_str()),
        })
        .collect();
    vaults.sort_by(|left, right| left.name.cmp(&right.name));

    let answer = WorkspaceAnswer {
        vaults,
        default,
        gc: config.gc_config().map(|gc| GcItem { grace: gc.grace() }),
    };

    serde_json::to_string(&answer).unwrap_or_else(|_| "null".to_owned())
}

/// A Vault's configuration, as the C ABI writes it.
#[derive(Debug, Serialize)]
struct VaultAnswer {
    /// The name the Vault goes by.
    name: String,
    /// What the Vault says about itself.
    description: String,
    /// Who may act as an administrator, in the order the file names them.
    admins: Vec<String>,
    /// The most transfers its daemon carries at once.
    max_concurrency: u16,
}

/// What the Vault holding `directory` says about itself, as one JSON document.
///
/// The name and description are its own, the administrators are the members that may move any file
/// in its Layout, and the concurrency is the most transfers its daemon serves at once. A directory
/// no Vault holds, and a configuration that cannot be read, are answered with the JSON `null`.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"name","description","admins":[...],"max_concurrency":N}`, or `null` when there is no Vault or
/// no configuration to read.
#[must_use]
#[lazyffi(export = rola_vault_config_json)]
pub fn vault_config_json(directory: &Path) -> String {
    let Some(vault) = Vault::locate(directory) else {
        return "null".to_owned();
    };
    let Ok(config) = VaultConfig::read_from(&vault.config_path()) else {
        return "null".to_owned();
    };

    let meta = config.vault_config();
    let answer = VaultAnswer {
        name: meta.name().to_owned(),
        description: meta.description().to_owned(),
        admins: config.auth_config().admins().to_vec(),
        max_concurrency: config.daemon_config().max_concurrency(),
    };

    serde_json::to_string(&answer).unwrap_or_else(|_| "null".to_owned())
}

/// The Vaults the machine's daemon serves, by namespace.
///
/// The table is the daemon's own registry, under the user's data home. A machine that has
/// registered nothing has an empty table, which is an empty list rather than a failure; a table
/// that is there and cannot be read is `null`, since the reader cannot tell what is served.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"vaults":[{"name":"default","path":"/vaults/default"}]}`, or `null` when the table cannot be
/// read.
#[must_use]
#[lazyffi(export = rola_daemon_vaults_json)]
pub fn daemon_vaults_json() -> String {
    let Some(path) = registry_path() else {
        return "{\"vaults\":[]}".to_owned();
    };
    let Ok(namespaces) = Namespaces::read_from(&path) else {
        return "null".to_owned();
    };
    let vaults: Vec<DaemonVaultItem> = namespaces
        .iter()
        .map(|(name, path)| DaemonVaultItem {
            name: name.to_owned(),
            path: path.display().to_string(),
        })
        .collect();

    serde_json::to_string(&DaemonVaultsAnswer { vaults }).unwrap_or_else(|_| "null".to_owned())
}

/// One Vault the daemon serves.
#[derive(Debug, Serialize)]
struct DaemonVaultItem {
    /// The namespace it answers under.
    name: String,
    /// The directory the Vault sits at.
    path: String,
}

/// The daemon's registered Vaults.
#[derive(Debug, Serialize)]
struct DaemonVaultsAnswer {
    /// The Vaults, in namespace order.
    vaults: Vec<DaemonVaultItem>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_configure::Configure as _;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::{Config as WorkspaceConfig, Workspace};

    use super::{vault_config_json, workspace_config_json};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let name = format!(
            "rorolala-config-read-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );

        for base in [Some(std::env::temp_dir()), program_dir()] {
            let Some(base) = base else {
                continue;
            };
            let dir = base.join(&name);
            let _ = fs::remove_dir_all(&dir);

            if fs::create_dir_all(&dir).is_ok() {
                return dir;
            }
        }

        panic!("no writable directory to make {name} in");
    }

    /// The directory this test's program sits in, or nothing when it cannot be found.
    fn program_dir() -> Option<PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
    }

    #[test]
    fn no_workspace_is_answered_with_null() {
        let dir = scratch("absent");

        assert_eq!(workspace_config_json(&dir), "null");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_vaults_a_workspace_knows_are_listed_by_name() {
        let dir = scratch("vaults");
        Workspace::create(&dir).unwrap();

        let workspace = Workspace::locate(&dir).unwrap();
        let path = workspace.config_path();
        let mut config = WorkspaceConfig::read_from(&path).unwrap();
        config.vaults_mut().bind("studio", "tcp://127.0.0.1:9270");
        config
            .vaults_mut()
            .bind("alpha", "ssh://alice@example.test/vault");
        config.default_config_mut().set_vault("studio");
        config.write_to(&path).unwrap();

        let answer: serde_json::Value = serde_json::from_str(&workspace_config_json(&dir)).unwrap();
        let vaults = answer["vaults"].as_array().unwrap();

        assert_eq!(vaults.len(), 2);
        assert_eq!(vaults[0]["name"], "alpha");
        assert_eq!(vaults[1]["name"], "studio");
        assert_eq!(vaults[1]["is_default"], true);
        assert_eq!(vaults[1]["address"], "tcp://127.0.0.1:9270");
        assert_eq!(answer["default"], "studio");
        assert!(answer["gc"].is_null());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_that_is_no_vault_is_answered_with_null() {
        let dir = scratch("no-vault");

        assert_eq!(vault_config_json(&dir), "null");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_says_what_it_is_and_who_administers_it() {
        let dir = scratch("vault");
        rorolala_vault::Vault::create(&dir).unwrap();

        let answer: serde_json::Value = serde_json::from_str(&vault_config_json(&dir)).unwrap();

        assert!(answer["name"].as_str().is_some_and(|name| !name.is_empty()));
        assert!(
            answer["description"]
                .as_str()
                .is_some_and(|description| !description.is_empty())
        );
        assert!(answer["admins"].is_array());
        assert!(answer["max_concurrency"].as_u64().is_some());

        let _ = fs::remove_dir_all(&dir);
    }
}
