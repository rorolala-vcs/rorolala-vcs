use rorolala_utils_configure::Configure;
use rorolala_utils_constants::VAULT_DEFAULT_PORT;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

/// Top-level configuration for the Vault
///
/// Vault serves as a remote storage warehouse, and this struct aggregates
/// all of the settings required to connect to and interact with it.
#[lazyffi(export = VaultConfig)]
#[derive(Debug, Default, Clone, Configure, Serialize, Deserialize)]
#[serde(default)]
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
#[derive(Debug, Clone, Configure, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    /// The port the daemon listens on.
    prefer_port: u16,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            prefer_port: VAULT_DEFAULT_PORT,
        }
    }
}

impl DaemonConfig {
    /// The port the daemon listens on.
    ///
    /// It is the port itself, not a preference among others: if it is already taken the
    /// daemon reports that and stops, rather than binding somewhere else that a client would
    /// then have to be told about.
    #[must_use]
    pub const fn prefer_port(&self) -> u16 {
        self.prefer_port
    }
}

#[cfg(test)]
mod tests {
    use rorolala_utils_constants::VAULT_DEFAULT_PORT;

    use super::{Config, DaemonConfig};

    #[test]
    fn a_vault_listens_on_the_default_port_by_default() {
        assert_eq!(DaemonConfig::default().prefer_port(), VAULT_DEFAULT_PORT);
        assert_eq!(Config::default().daemon_config().prefer_port(), 7717);
    }

    #[test]
    fn a_configuration_without_a_port_falls_back_to_the_default_one() {
        // A file written before the port existed, or emptied by hand, still reads: what is
        // missing is filled from the default rather than failing the whole Vault.
        let read: Config = toml::from_str("").unwrap();

        assert_eq!(read.daemon_config().prefer_port(), VAULT_DEFAULT_PORT);
    }
}
