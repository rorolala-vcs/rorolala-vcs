use rorolala_utils_configure::Configure;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

/// Top-level configuration for the Vault
///
/// Vault serves as a remote storage warehouse, and this struct aggregates
/// all of the settings required to connect to and interact with it.
#[lazyffi(export = VaultConfig)]
#[derive(Debug, Default, Clone, Configure, Serialize, Deserialize)]
pub struct Config {
    /// Configuration for the Vault daemon.
    daemon_config: DaemonConfig,
}

/// Configuration for the Vault daemon.
#[lazyffi(export = VaultDaemonConfig)]
#[derive(Debug, Default, Clone, Configure, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// The preferred port the daemon should listen on.
    prefer_port: u16,
}
