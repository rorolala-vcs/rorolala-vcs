use std::collections::HashMap;
use std::net::SocketAddr;

use rorolala_utils_configure::Configure;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

/// An address a Vault answers at, as a Workspace writes it down.
///
/// It is an ip and port, which is what the daemon binds and a client dials: a name has
/// to be resolved before it can be written down, so what is stored is where to go rather
/// than what to ask to get there.
pub type SocketAddress = SocketAddr;

/// Top-level configuration for the Workspace
///
/// A Workspace keeps its own settings beside the data it holds: where the Vaults it shares
/// through are, and — as there comes to be more — whatever else the copy being worked in
/// needs to remember.
#[lazyffi(export = WorkspaceConfig)]
#[derive(Debug, Default, Clone, Configure, Serialize, Deserialize)]
pub struct Config {
    /// The Vaults this Workspace knows, each under a name of its own.
    #[serde(default, skip_serializing_if = "VaultsConfig::is_empty")]
    vaults: VaultsConfig,
}

impl Config {
    /// The Vaults this Workspace knows.
    #[must_use]
    pub const fn vaults(&self) -> &VaultsConfig {
        &self.vaults
    }

    /// The Vaults this Workspace knows, to be changed.
    ///
    /// A change made through here is kept: the configuration is written back to the file it
    /// was read from once the program is done with it.
    pub const fn vaults_mut(&mut self) -> &mut VaultsConfig {
        &mut self.vaults
    }
}

/// The Vaults a Workspace knows, each under a name of its own.
///
/// A name is how the Workspace talks about a Vault — `origin`, say — and the address is
/// where that Vault answers. Names are not shared between Workspaces: the same Vault may
/// well be `origin` in one and `upstream` in another.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VaultsConfig {
    /// The address each name means.
    vaults: HashMap<String, SocketAddress>,
}

impl VaultsConfig {
    /// Binds `name` to `address`, answering with the address it was bound to before, if any.
    ///
    /// Binding a name that is already taken is how an address is changed: the new one
    /// replaces the old and the old is handed back.
    pub fn bind(
        &mut self,
        name: impl Into<String>,
        address: SocketAddress,
    ) -> Option<SocketAddress> {
        self.vaults.insert(name.into(), address)
    }

    /// Lets `name` go, answering with the address it was bound to, if it was bound at all.
    pub fn unbind(&mut self, name: &str) -> Option<SocketAddress> {
        self.vaults.remove(name)
    }

    /// The address `name` is bound to, if it is bound at all.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SocketAddress> {
        self.vaults.get(name)
    }

    /// Whether `name` is bound to anything.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.vaults.contains_key(name)
    }

    /// Each name and the address it is bound to.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &SocketAddress)> {
        self.vaults.iter()
    }

    /// Each name in use.
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.vaults.keys()
    }

    /// How many Vaults are bound.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vaults.len()
    }

    /// Whether no Vault is bound.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vaults.is_empty()
    }
}

impl From<HashMap<String, SocketAddress>> for VaultsConfig {
    fn from(vaults: HashMap<String, SocketAddress>) -> Self {
        Self { vaults }
    }
}

impl FromIterator<(String, SocketAddress)> for VaultsConfig {
    fn from_iter<IntoIter: IntoIterator<Item = (String, SocketAddress)>>(
        entries: IntoIter,
    ) -> Self {
        Self {
            vaults: entries.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::{Config, VaultsConfig};

    /// An address to bind names to, parsed the way a caller's word would be.
    fn address(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    #[test]
    fn binding_a_name_keeps_it_and_hands_back_what_it_replaced() {
        let mut vaults = VaultsConfig::default();

        // A name that is free is simply bound.
        assert_eq!(vaults.bind("origin", address("127.0.0.1:7000")), None);
        assert!(vaults.contains("origin"));
        assert_eq!(vaults.get("origin"), Some(&address("127.0.0.1:7000")));

        // Binding it again is how the address is changed, and the old one comes back.
        assert_eq!(
            vaults.bind("origin", address("10.0.0.1:7001")),
            Some(address("127.0.0.1:7000"))
        );
        assert_eq!(vaults.get("origin"), Some(&address("10.0.0.1:7001")));
        assert_eq!(vaults.len(), 1);
    }

    #[test]
    fn unbinding_hands_back_the_address_and_leaves_nothing_behind() {
        let mut vaults = VaultsConfig::default();
        vaults.bind("origin", address("127.0.0.1:7000"));

        assert_eq!(vaults.unbind("origin"), Some(address("127.0.0.1:7000")));
        assert!(!vaults.contains("origin"));
        assert!(vaults.is_empty());

        // A name that was never bound is not an error, only nothing to hand back.
        assert_eq!(vaults.unbind("origin"), None);
    }

    #[test]
    fn a_configuration_survives_the_format_it_is_written_in() {
        // The extension picks TOML, which is the format a Workspace's settings are in.
        let mut config = Config::default();
        config
            .vaults_mut()
            .bind("origin", address("127.0.0.1:7000"));
        config.vaults_mut().bind("upstream", address("[::1]:7001"));

        let toml = toml::to_string(&config).unwrap();
        let read: Config = toml::from_str(&toml).unwrap();

        assert_eq!(read.vaults(), config.vaults());
        assert_eq!(
            read.vaults().get("origin"),
            Some(&address("127.0.0.1:7000"))
        );
        assert_eq!(read.vaults().get("upstream"), Some(&address("[::1]:7001")));
    }

    #[test]
    fn a_configuration_with_no_vaults_writes_no_table_for_them() {
        let toml = toml::to_string(&Config::default()).unwrap();

        assert!(!toml.contains("vaults"), "{toml}");
    }
}
