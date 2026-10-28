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

/// Path, inside the Workspace's data directory, where its Layouts are kept
///
/// One directory per Layout, each named by the Layout's own name, so a Workspace holds as
/// many as it has names for. A Vault keeps one instead, in a directory of its own.
#[lazyffi(export = ROLA_WORKSPACE_LAYOUTS_DIR)]
pub const WORKSPACE_LAYOUTS_DIR: &str = "./.rola/layouts/";

/// Path to the file naming the Layout a Workspace has checked out
///
/// It holds the name of one of the Layouts under [`WORKSPACE_LAYOUTS_DIR`], which is the Layout
/// the Workspace works in until another is chosen.
#[lazyffi(export = ROLA_WORKSPACE_LAYOUT_PATH)]
pub const WORKSPACE_LAYOUT_PATH: &str = "./.rola/LAYOUT";

/// Path to the file a run opens in an editor to write what it is recording
///
/// A message is written where it can be edited as text — the whole of what a run records, the group
/// it applies to and what each file did — so the editor writes one file the run then reads back. It
/// lives in the Workspace's data directory, beside the rest of what the Workspace keeps for itself,
/// since it is not part of the work.
#[lazyffi(export = ROLA_WORKSPACE_EDITING_PATH)]
pub const WORKSPACE_EDITING_PATH: &str = "./.rola/EDITING.md";

/// Path, inside the Workspace's data directory, where what a Layout's analysis is kept
///
/// The tree a Layout describes is read back against the files a Workspace holds, and what was
/// found — each file's time, size and fingerprint — is kept here so the next reading can skip
/// what has not changed. One directory per Layout; see [`LAYOUT_TRACK_FILE`] and its kin.
#[lazyffi(export = ROLA_WORKSPACE_CACHE_DIR)]
pub const WORKSPACE_CACHE_DIR: &str = "./.rola/cache/";

/// Path, inside the Workspace's cache, where remote Layouts are kept read-only
///
/// A Layout fetched from a Vault is not one the Workspace works in: it is a copy of what the
/// Vault holds, read to see who holds what rather than written to. One directory per Vault,
/// named by the name the Workspace bound it under, so the same Vault is fetched to the same
/// place every time.
#[lazyffi(export = ROLA_WORKSPACE_READONLY_LAYOUTS_DIR)]
pub const WORKSPACE_READONLY_LAYOUTS_DIR: &str = "./.rola/cache/readonly-layouts/";

/// Path, inside the Workspace's data directory, where the ignore rules it works by are kept
///
/// The rules are managed rather than scattered through the tree the way a `.gitignore` is: one
/// directory holds them all, a file per rule set, and a Workspace reads none of them from the
/// work itself. What a Vault hands down lands under [`IGNORE_RULES_VAULTS_DIR`] inside it, so a
/// rule of the Workspace's own always sits shallower — and therefore decides over — one that was
/// fetched.
#[lazyffi(export = ROLA_WORKSPACE_IGNORE_RULES_DIR)]
pub const WORKSPACE_IGNORE_RULES_DIR: &str = "./.rola/info/ignore/";

/// The directory, inside a Workspace's ignore rules, that a fetched Vault's rules are kept under
///
/// One directory per Vault, named by the name the Workspace bound it under. Being deeper than the
/// Workspace's own files is what makes a Vault's rule yield to a Workspace's contradicting one.
pub const IGNORE_RULES_VAULTS_DIR: &str = "vaults";

/// Path, inside the Vault's root, where the ignore rules it hands down are kept
///
/// The same shape as a Workspace's — one directory, a file per rule set — read by a fetch rather
/// than by the Vault itself: a Vault has no tree of work for the rules to be read against.
#[lazyffi(export = ROLA_VAULT_IGNORE_RULES_DIR)]
pub const VAULT_IGNORE_RULES_DIR: &str = "./info/ignore/";

/// The file, inside a Layout's directory, naming the upstream Vault it tracks
///
/// It holds the name of a Vault the Workspace has bound, or nothing when the Layout tracks
/// none. A Layout is a place to work; the Vault it tracks is where the work goes.
#[lazyffi(export = ROLA_LAYOUT_TRACK_FILE)]
pub const LAYOUT_TRACK_FILE: &str = "TRACK";

/// Path, inside the Vault's root, where its single Layout is kept
///
/// A Vault holds one Layout rather than many, so it is kept directly under here rather than in
/// a directory of its own; see [`VAULT_LAYOUT_NAME`] for what that one is called.
#[lazyffi(export = ROLA_VAULT_LAYOUT_DIR)]
pub const VAULT_LAYOUT_DIR: &str = "./layout/";

/// The name a Workspace's first Layout is commonly given
///
/// It is a name to offer rather than one that is imposed: a Workspace is made with no Layout at
/// all, and the name of the first one is whoever adds it to choose. This is what that name is
/// expected to be, for anything that has to suggest one.
#[lazyffi(export = ROLA_DEFAULT_LAYOUT_NAME)]
pub const DEFAULT_LAYOUT_NAME: &str = "main";

/// The name a Vault's single Layout is known by
///
/// A Vault's Layout is one and has no name of its own on disk; this is what it is called when
/// it has to be named, as when a Vault's Layouts are listed.
#[lazyffi(export = ROLA_VAULT_LAYOUT_NAME)]
pub const VAULT_LAYOUT_NAME: &str = "truth";

/// Path to a directory's lock file, besides whatever names the directory
///
/// A place is locked while this file is there, and unlocked while it is not — so a lock is
/// something a person can see, and take away, without a tool.
pub const LOCK_FILE: &str = "lock";

/// Path to the Workspace's lock file
///
/// A Workspace keeps its lock beside the rest of its data rather than at its root: the root is
/// where the work is, and the lock is about the Workspace's own bookkeeping.
pub const WORKSPACE_LOCK_PATH: &str = "./.rola/lock";

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

/// Path, inside the Workspace's data directory, where its index is kept
///
/// The index is the version control index the Workspace reads and writes as work is
/// recorded, kept beside the store rather than in it: what is indexed is the versions,
/// and the store is where the content they name lives.
#[lazyffi(export = ROLA_WORKSPACE_INDEX_DIR)]
pub const WORKSPACE_INDEX_DIR: &str = "./.rola/index/";

/// Path, inside the Vault's root, where its store is kept
#[lazyffi(export = ROLA_VAULT_STORAGE_DIR)]
pub const VAULT_STORAGE_DIR: &str = "./storage/";

/// Path, inside the Vault's root, where its index is kept
///
/// As for a Workspace's: the index is kept beside the store rather than in it.
#[lazyffi(export = ROLA_VAULT_INDEX_DIR)]
pub const VAULT_INDEX_DIR: &str = "./index/";

/// The sub-vault an address names when it names none
///
/// An address is `rola://ip:port/namespace`, and a request with no namespace names the daemon
/// itself rather than any Vault it serves. Naming none is what an address written without a
/// path says, and what says so is the empty name.
#[lazyffi(export = ROLA_ROOT_SUB_VAULT)]
pub const ROOT_SUB_VAULT: &str = "";

/// The port a daemon listens on when neither the run nor the environment names one.
///
/// It is a port of its own rather than an ephemeral one, so that a machine has one address to
/// be reached at without being told it every time. It is also what an address written without
/// a port means, which is why it lives here beside the rest of the layout rather than only in
/// the daemon that happens to bind it.
#[lazyffi(export = ROLA_VAULT_DEFAULT_PORT)]
pub const VAULT_DEFAULT_PORT: u16 = 7717;

/// The variable that names the port a daemon listens on.
///
/// It is consulted only where `--port` named none, and what it says is the same number
/// [`VAULT_DEFAULT_PORT`] would otherwise be.
#[lazyffi(export = ROLA_PORT_ENV_VAR)]
pub const PORT_ENV_VAR: &str = "ROLA_DEFAULT_PORT";

/// Keys of a global (machine-wide) scope, under the filesystem root
#[lazyffi(export = ROLA_GLOBAL_KEYS_DIR)]
pub const GLOBAL_KEYS_DIR: &str = ".rola/keys";

/// Keys of a user scope, under the user's local data directory
#[lazyffi(export = ROLA_USER_KEYS_DIR)]
pub const USER_KEYS_DIR: &str = "rola/keys";

/// The directory, under the user's local data directory, Rorolala keeps its own files in
///
/// These belong to no Vault and no Workspace: they are the user's own, and they are small enough
/// to be plain files rather than configurations. It is the parent of [`USER_KEYS_DIR`], which
/// keeps the one thing here that is a directory rather than a file, and of
/// [`DAEMON_DIR`], which is what the machine's daemon keeps.
pub const USER_DATA_DIR: &str = "rola";

/// The directory, under [`USER_DATA_DIR`], the machine's daemon keeps its files in.
#[lazyffi(export = ROLA_DAEMON_DIR)]
pub const DAEMON_DIR: &str = "daemon";

/// The file, under [`DAEMON_DIR`], naming the Vaults the daemon serves.
///
/// It is a flat table of namespace to the path the Vault sits at, written the way any other
/// Rorolala configuration is.
#[lazyffi(export = ROLA_DAEMON_REGISTRY_FILE)]
pub const DAEMON_REGISTRY_FILE: &str = "registry.toml";

/// The file, under [`DAEMON_DIR`], saying what this machine allows its daemon to do.
///
/// It is the machine's rather than any one Vault's, and it sits beside the table of namespaces
/// for the same reason: both are about the daemon that runs here rather than about a Vault it
/// happens to serve.
#[lazyffi(export = ROLA_DAEMON_PREFERENCE_FILE)]
pub const DAEMON_PREFERENCE_FILE: &str = "preference.toml";

/// The file, under [`USER_DATA_DIR`], naming the account the work acts as
pub const USER_ACCOUNT_FILE: &str = "user";

/// The file, under [`USER_DATA_DIR`], the addresses seen so far are kept in, one per line
pub const USER_HISTORY_FILE: &str = "addr.hs";

/// The file, under [`USER_DATA_DIR`], the exit code of the last run is kept in
pub const USER_LASTEC_FILE: &str = "lastec";

/// The variable naming the directory the user's own files are kept under
///
/// What it names is the directory [`USER_DATA_DIR`] sits in, which is the same thing a machine
/// that follows the XDG layout means by `XDG_DATA_HOME`. It is for a run that must keep its files
/// somewhere other than the user's — an integration suite, which would otherwise read and write
/// the machine's — rather than for a person to set: a sandbox built on `XDG_DATA_HOME` alone is a
/// sandbox on Unix, since Windows has no such variable to read.
#[lazyffi(export = ROLA_DATA_HOME_ENV_VAR)]
pub const DATA_HOME_ENV_VAR: &str = "ROLA_DATA_HOME";

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
