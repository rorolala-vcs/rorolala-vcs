#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use rorolala_utils_lazyffi::lazyffi;

/// The Workspace's data directory, inside the workspace root
///
/// A directory is a Workspace once it holds one, and everything the Workspace keeps for
/// itself sits under it — the configuration and the keys alike.
#[lazyffi(export = ROLA_WORKSPACE_DATA_DIR)]
pub const WORKSPACE_DATA_DIR: &str = "./.rola/";

/// Path to the Workspace configuration file, inside its data directory
#[lazyffi(export = ROLA_WORKSPACE_CONFIG_PATH)]
pub const WORKSPACE_CONFIG_PATH: &str = "./.rola/workspace.toml";

/// Path, inside the Workspace's data directory, where its keys are kept
///
/// A key kept here belongs to the Workspace it was set up in, so it is one of the two
/// directories a local key search covers — the other being the Vault's.
#[lazyffi(export = ROLA_WORKSPACE_KEYS_DIR)]
pub const WORKSPACE_KEYS_DIR: &str = "./.rola/auth/";

/// Path to the Vault configuration file, inside the vault root
#[lazyffi(export = ROLA_VAULT_CONFIG_PATH)]
pub const VAULT_CONFIG_PATH: &str = "./vault.toml";

/// Path, inside the Vault, where its keys are kept
///
/// A key kept here belongs to the Vault it sits in, so it is one of the two directories a
/// local key search covers — the other being the Workspace's.
#[lazyffi(export = ROLA_VAULT_KEYS_DIR)]
pub const VAULT_KEYS_DIR: &str = "./keys/";

/// Keys of a global (machine-wide) scope, under the filesystem root
pub const GLOBAL_KEYS_DIR: &str = ".rola/keys";

/// Keys of a user scope, under the user's local data directory
pub const USER_KEYS_DIR: &str = "rola/keys";

/// Keys named by the environment, under [`HOME_ENV_VAR`]
pub const ENV_KEYS_DIR: &str = "keys";

/// The variable that names the directory an environment key set lives in
pub const HOME_ENV_VAR: &str = "ROLA_HOME";

/// The extension a member's public key carries
pub const PUBLIC_KEY_EXTENSION: &str = "pub";

/// The extension an account's private key carries
pub const PRIVATE_KEY_EXTENSION: &str = "pem";
