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

impl Config {
    /// The configuration for the Vault daemon.
    #[must_use]
    pub const fn daemon_config(&self) -> &DaemonConfig {
        &self.daemon_config
    }
}

/// Configuration for the Vault daemon.
#[lazyffi(export = VaultDaemonConfig)]
#[derive(Debug, Default, Clone, Configure, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// The preferred port the daemon should listen on.
    prefer_port: u16,
}

impl DaemonConfig {
    /// The port the daemon prefers to listen on.
    ///
    /// It is a preference and not an order: the daemon asks for this port, and takes
    /// another if it is already taken.
    #[must_use]
    pub const fn prefer_port(&self) -> u16 {
        self.prefer_port
    }
}
