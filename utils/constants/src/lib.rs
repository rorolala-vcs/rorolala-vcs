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

/// Path, inside the Vault's root, where the Vaults it holds are kept
///
/// A directory under here is a Vault the root holds once it carries a configuration of its
/// own, which is what `RootVault` lists. It is a plain directory beside the configuration
/// rather than one of the Vault's own: a Vault held by another is a Vault in its own right,
/// and nothing about it says it is held.
#[lazyffi(export = ROLA_VAULT_VAULTS_DIR)]
pub const VAULT_VAULTS_DIR: &str = "./vaults/";

/// Path to the storage configuration file, inside the storage root
///
/// A directory is a store once it carries one, which is what a search for a local store reads
/// to find where the store begins.
#[lazyffi(export = ROLA_STORAGE_CONFIG_PATH)]
pub const STORAGE_CONFIG_PATH: &str = "./rolast.toml";

/// Path, inside the Workspace's data directory, where its store is kept
#[lazyffi(export = ROLA_WORKSPACE_STORAGE_DIR)]
pub const WORKSPACE_STORAGE_DIR: &str = "./.rola/storage/";

/// Path, inside the Vault's root, where its store is kept
#[lazyffi(export = ROLA_VAULT_STORAGE_DIR)]
pub const VAULT_STORAGE_DIR: &str = "./storage/";

/// The sub-vault an address names when it names none
///
/// An address is `rola://ip:port/sub`, and the Vault at the root of a tree holds the others
/// rather than being one of them. Naming it is what an address written without a `/sub`
/// means, and what says so is the empty name: the root is the Vault a path naming nothing
/// resolves to, which is the same answer `RootVault` gives an empty path.
#[lazyffi(export = ROLA_ROOT_SUB_VAULT)]
pub const ROOT_SUB_VAULT: &str = "";

/// The port a Vault's daemon listens on when its configuration names no other.
///
/// It is a port of its own rather than an ephemeral one, so that a Vault has one address to
/// be reached at without being told it every time. It is also what an address written without
/// a port means, which is why it lives here beside the rest of the layout rather than only in
/// the configuration that happens to read it.
#[lazyffi(export = ROLA_VAULT_DEFAULT_PORT)]
pub const VAULT_DEFAULT_PORT: u16 = 7717;

/// Keys of a global (machine-wide) scope, under the filesystem root
#[lazyffi(export = ROLA_GLOBAL_KEYS_DIR)]
pub const GLOBAL_KEYS_DIR: &str = ".rola/keys";

/// Keys of a user scope, under the user's local data directory
#[lazyffi(export = ROLA_USER_KEYS_DIR)]
pub const USER_KEYS_DIR: &str = "rola/keys";

/// Keys named by the environment, under [`HOME_ENV_VAR`]
#[lazyffi(export = ROLA_ENV_KEYS_DIR)]
pub const ENV_KEYS_DIR: &str = "keys";

/// The variable that names the directory an environment key set lives in
#[lazyffi(export = ROLA_HOME_ENV_VAR)]
pub const HOME_ENV_VAR: &str = "ROLA_HOME";

/// The extension a member's public key carries
#[lazyffi(export = ROLA_PUBLIC_KEY_EXTENSION)]
pub const PUBLIC_KEY_EXTENSION: &str = "pub";

/// The extension an account's private key carries
#[lazyffi(export = ROLA_PRIVATE_KEY_EXTENSION)]
pub const PRIVATE_KEY_EXTENSION: &str = "pem";
