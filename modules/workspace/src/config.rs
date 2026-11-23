use std::collections::HashMap;
use std::path::Path;

use rorolala_utils_configure::Configure;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use serde::{Deserialize, Serialize};

use crate::Workspace;

/// Top-level configuration for the Workspace
///
/// A Workspace keeps its own settings beside the data it holds: where the Vaults it shares
/// through are, and, as there comes to be more, whatever else the copy being worked in
/// needs to remember.
// The conversions the export generates only move a handle; what the configuration carries is
// ordinary data, so the derived `Deserialize` has nothing unsafe to hide.
#[allow(clippy::unsafe_derive_deserialize)]
#[lazyffi(export = RolaWorkspaceConfig)]
#[derive(Debug, Default, Clone, Configure, Serialize, Deserialize)]
pub struct Config {
    /// The Vaults this Workspace knows, each under a name of its own.
    #[serde(default, skip_serializing_if = "VaultsConfig::is_empty")]
    vaults: VaultsConfig,
    /// What the Workspace reaches for when nothing else names anything.
    #[serde(
        default,
        rename = "default",
        skip_serializing_if = "DefaultConfig::is_empty"
    )]
    fallback: DefaultConfig,
    /// What a collecting run in this Workspace is told about what was written recently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gc: Option<GcConfig>,
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

    /// What this Workspace reaches for when nothing else names anything.
    #[must_use]
    pub const fn default_config(&self) -> &DefaultConfig {
        &self.fallback
    }

    /// What this Workspace reaches for, to be changed.
    ///
    /// A change made through here is kept, the way a change to the Vaults is.
    pub const fn default_config_mut(&mut self) -> &mut DefaultConfig {
        &mut self.fallback
    }

    /// What a collecting run in this Workspace is told, when the configuration says.
    #[must_use]
    pub const fn gc_config(&self) -> Option<&GcConfig> {
        self.gc.as_ref()
    }
}

/// One Vault a Workspace knows.
#[lazyffi(export = RolaWorkspaceVault)]
#[derive(Debug, Clone)]
pub struct WorkspaceVault {
    /// The name the Workspace knows it by.
    name: String,
    /// The address it answers at.
    address: String,
    /// Whether it is the Vault the Workspace reaches for.
    is_default: bool,
}

#[lazyffi(export = rola_)]
impl WorkspaceVault {
    /// The name the Workspace knows it by.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// The address it answers at.
    #[must_use]
    pub fn address(&self) -> String {
        self.address.clone()
    }

    /// Whether it is the Vault the Workspace reaches for.
    #[must_use]
    pub const fn is_default(&self) -> bool {
        self.is_default
    }
}

#[lazyffi(export = rola_workspace_)]
impl Config {
    /// How many Vaults this Workspace knows.
    #[must_use]
    pub fn vault_count(&self) -> usize {
        self.vaults().len()
    }

    /// The Vault at `index`, in name order, or null past the end.
    #[must_use]
    pub fn vault_at(&self, index: usize) -> Option<WorkspaceVault> {
        let default = self.default_config().vault();
        let mut vaults: Vec<(&String, &String)> = self.vaults().iter().collect();
        vaults.sort_by_key(|(left, _)| *left);

        vaults.get(index).map(|(name, address)| WorkspaceVault {
            name: (*name).clone(),
            address: (*address).clone(),
            is_default: default == Some(name.as_str()),
        })
    }

    /// The name of the Vault this Workspace reaches for, or an empty string when it names none.
    #[must_use]
    pub fn default_vault(&self) -> String {
        self.default_config().vault().unwrap_or_default().to_owned()
    }

    /// Whether the configuration tells a collecting run what to keep.
    #[must_use]
    pub const fn gc_present(&self) -> bool {
        self.gc.is_some()
    }

    /// How long a collecting run keeps what nothing references, in seconds, or -1 when the
    /// configuration says nothing.
    #[must_use]
    pub fn gc_grace(&self) -> i64 {
        self.gc
            .as_ref()
            .and_then(GcConfig::grace)
            .and_then(|grace| i64::try_from(grace).ok())
            .unwrap_or(-1)
    }
}

/// A Workspace's configuration, read from the Workspace holding `directory`.
///
/// # FFI
///
/// A `RolaWorkspaceConfig *` the caller releases with `free_rola_workspace_config`, or null when a
/// directory no Workspace holds, or when the configuration cannot be read.
#[must_use]
#[lazyffi(export = rola_workspace_config_read)]
pub fn read(directory: &Path) -> Option<Config> {
    let workspace = Workspace::locate(directory)?;

    Config::read_from(&workspace.config_path()).ok()
}

/// What a collecting run in this Workspace is told about what was written recently.
///
/// It is written under `[gc]`. The grace is how long an entry nothing references is kept after it
/// was written, so a run that is still putting its reference down does not lose what it wrote; a
/// grace of zero suppresses the keeping, and a table that says nothing leaves the collecting run's
/// own default.
#[derive(Debug, Default, Clone, Copy, Configure, Serialize, Deserialize)]
#[serde(default)]
pub struct GcConfig {
    /// How long an entry nothing references is kept, in seconds; zero keeps nothing.
    grace: Option<u64>,
}

impl GcConfig {
    /// How long an entry nothing references is kept, in seconds, when the file says.
    #[must_use]
    pub const fn grace(&self) -> Option<u64> {
        self.grace
    }
}

/// What a Workspace reaches for when nothing else names anything.
///
/// It is the same kind of thing a binding is, a name, but a choice among them rather than
/// one of them: the name is meant to be one the Workspace knows, which is the caller's to
/// check, and nothing else is kept here.
///
/// The stores are the places outside the Workspace's own `.rola/storage` that its content is
/// written to and read from, in the order they are tried. `.` among them means the Workspace's own
/// store, and one that is not there is added last, so a list without it means "the shared ones
/// first, then mine".
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultConfig {
    /// The name the Workspace reaches for, if one has been chosen.
    vault: Option<String>,
    /// The stores the Workspace binds, as they were written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    storages: Vec<String>,
}

impl DefaultConfig {
    /// The Vault the Workspace reaches for, if one has been chosen.
    #[must_use]
    pub fn vault(&self) -> Option<&str> {
        self.vault.as_deref()
    }

    /// Chooses the Vault to reach for, answering with the one that was chosen before.
    pub fn set_vault(&mut self, name: impl Into<String>) -> Option<String> {
        self.vault.replace(name.into())
    }

    /// Forgets the Vault to reach for, answering with the one that was chosen.
    pub const fn clear_vault(&mut self) -> Option<String> {
        self.vault.take()
    }

    /// The stores the Workspace binds, in the order it tries them.
    #[must_use]
    pub fn storages(&self) -> &[String] {
        &self.storages
    }

    /// Replaces the stores the Workspace binds, in one order.
    pub fn set_storages(&mut self, storages: impl IntoIterator<Item = String>) {
        self.storages = storages.into_iter().collect();
    }

    /// Binds `path` last, answering whether it was added.
    ///
    /// A path that is already bound is not added again and the order is left as it was: binding a
    /// store twice says nothing the first binding did not.
    pub fn add_storage(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        if self.storages.contains(&path) {
            return false;
        }

        self.storages.push(path);

        true
    }

    /// Unbinds `path`, answering whether it was bound.
    pub fn remove_storage(&mut self, path: &str) -> bool {
        let before = self.storages.len();
        self.storages.retain(|storage| storage != path);

        before != self.storages.len()
    }

    /// Whether nothing has been chosen.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.vault.is_none() && self.storages.is_empty()
    }
}

/// The Vaults a Workspace knows, each under a name of its own.
///
/// A name is how the Workspace talks about a Vault (`origin`, say) and the address is
/// where that Vault answers: the daemon to knock at, and which Vault under it is meant. The
/// address is kept as it was written, a link, `rola://<host>[:<port>]/<sub-vault>`, or the
/// bare address it is short for. Nothing here reads it: a Workspace writes down where to go,
/// and whoever dials reads the address it wrote down.
///
/// Names are not shared between Workspaces: the same Vault may well be `origin` in one and
/// `upstream` in another.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VaultsConfig {
    /// The address each name means, as it was written.
    vaults: HashMap<String, String>,
}

impl VaultsConfig {
    /// Binds `name` to `address`, answering with the address it was bound to before, if any.
    ///
    /// Binding a name that is already taken is how an address is changed: the new one
    /// replaces the old and the old is handed back.
    pub fn bind(&mut self, name: impl Into<String>, address: impl Into<String>) -> Option<String> {
        self.vaults.insert(name.into(), address.into())
    }

    /// Lets `name` go, answering with the address it was bound to, if it was bound at all.
    pub fn unbind(&mut self, name: &str) -> Option<String> {
        self.vaults.remove(name)
    }

    /// The address `name` is bound to, if it is bound at all.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&String> {
        self.vaults.get(name)
    }

    /// Whether `name` is bound to anything.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.vaults.contains_key(name)
    }

    /// Each name and the address it is bound to.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
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

impl From<HashMap<String, String>> for VaultsConfig {
    fn from(vaults: HashMap<String, String>) -> Self {
        Self { vaults }
    }
}

impl FromIterator<(String, String)> for VaultsConfig {
    fn from_iter<IntoIter: IntoIterator<Item = (String, String)>>(entries: IntoIter) -> Self {
        Self {
            vaults: entries.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{Config, VaultsConfig};

    /// An address to bind names to, as a caller would have written it.
    fn address(text: &str) -> String {
        text.to_owned()
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

    #[test]
    fn choosing_a_vault_to_reach_for_hands_back_the_one_it_replaced() {
        let mut config = Config::default();

        // Nothing is chosen to begin with, and a choice is answered by the one before it.
        assert_eq!(config.default_config().vault(), None);
        assert_eq!(config.default_config_mut().set_vault("origin"), None);
        assert_eq!(config.default_config().vault(), Some("origin"));

        assert_eq!(
            config.default_config_mut().set_vault("upstream"),
            Some("origin".to_owned())
        );

        // Letting it go answers with what was chosen, and leaves nothing chosen.
        assert_eq!(
            config.default_config_mut().clear_vault(),
            Some("upstream".to_owned())
        );
        assert!(config.default_config().is_empty());
        assert_eq!(config.default_config_mut().clear_vault(), None);
    }

    #[test]
    fn the_vault_a_workspace_reaches_for_survives_the_format_it_is_written_in() {
        let mut config = Config::default();
        config
            .vaults_mut()
            .bind("origin", address("127.0.0.1:7000"));
        config.default_config_mut().set_vault("origin");

        let toml = toml::to_string(&config).unwrap();
        let read: Config = toml::from_str(&toml).unwrap();

        assert_eq!(read.default_config().vault(), Some("origin"));
    }

    #[test]
    fn a_configuration_with_nothing_chosen_writes_no_default_table() {
        let toml = toml::to_string(&Config::default()).unwrap();

        assert!(!toml.contains("default"), "{toml}");
    }

    #[test]
    fn listing_the_vaults_gives_back_each_name_with_the_address_it_means() {
        let mut vaults = VaultsConfig::default();
        vaults.bind("origin", address("127.0.0.1:7000"));
        vaults.bind("upstream", address("[::1]:7001"));

        let mut listed: Vec<(String, String)> = vaults
            .iter()
            .map(|(name, address)| (name.clone(), address.clone()))
            .collect();
        listed.sort_by(|left, right| left.0.cmp(&right.0));

        assert_eq!(
            listed,
            [
                ("origin".to_owned(), address("127.0.0.1:7000")),
                ("upstream".to_owned(), address("[::1]:7001")),
            ]
        );
    }

    #[test]
    fn listing_the_names_gives_back_each_name_in_use() {
        let mut vaults = VaultsConfig::default();
        vaults.bind("origin", address("127.0.0.1:7000"));
        vaults.bind("upstream", address("[::1]:7001"));

        let mut names: Vec<String> = vaults.names().cloned().collect();
        names.sort();

        assert_eq!(names, ["origin", "upstream"]);
    }

    #[test]
    fn an_empty_configuration_lists_nothing() {
        let vaults = VaultsConfig::default();

        assert_eq!(vaults.iter().count(), 0);
        assert_eq!(vaults.names().count(), 0);
    }

    #[test]
    fn a_map_of_names_and_addresses_becomes_a_configuration_that_knows_them() {
        let mut map = HashMap::new();
        map.insert("origin".to_owned(), address("127.0.0.1:7000"));
        map.insert("upstream".to_owned(), address("[::1]:7001"));

        let vaults = VaultsConfig::from(map);

        assert_eq!(vaults.get("origin"), Some(&address("127.0.0.1:7000")));
        assert_eq!(vaults.get("upstream"), Some(&address("[::1]:7001")));
        assert_eq!(vaults.len(), 2);
    }

    #[test]
    fn collecting_pairs_of_names_and_addresses_binds_them_all() {
        let vaults: VaultsConfig = [
            ("origin".to_owned(), address("127.0.0.1:7000")),
            ("upstream".to_owned(), address("[::1]:7001")),
        ]
        .into_iter()
        .collect();

        assert_eq!(vaults.get("origin"), Some(&address("127.0.0.1:7000")));
        assert_eq!(vaults.get("upstream"), Some(&address("[::1]:7001")));
        assert_eq!(vaults.len(), 2);
    }

    #[test]
    fn the_stores_a_workspace_binds_survive_the_format_they_are_written_in() {
        let mut config = Config::default();
        config
            .default_config_mut()
            .set_storages(["/srv/rola/shared".to_owned(), "../shared".to_owned()]);

        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("storages"), "{toml}");

        let read: Config = toml::from_str(&toml).unwrap();
        assert_eq!(
            read.default_config().storages(),
            ["/srv/rola/shared", "../shared"]
        );
    }

    #[test]
    fn a_store_is_bound_once_and_keeps_the_order_it_was_bound_in() {
        let mut config = Config::default();

        assert!(config.default_config_mut().add_storage("/srv/shared"));
        assert!(config.default_config_mut().add_storage("../other"));
        assert!(!config.default_config_mut().add_storage("/srv/shared"));

        assert_eq!(
            config.default_config().storages(),
            ["/srv/shared", "../other"]
        );

        assert!(config.default_config_mut().remove_storage("/srv/shared"));
        assert!(!config.default_config_mut().remove_storage("/srv/shared"));
        assert_eq!(config.default_config().storages(), ["../other"]);
    }

    #[test]
    fn a_default_table_holding_only_stores_is_still_written() {
        let mut config = Config::default();
        config.default_config_mut().add_storage("/srv/shared");

        let toml = toml::to_string(&config).unwrap();

        assert!(toml.contains("[default]"), "{toml}");
        assert!(toml.contains("storages"), "{toml}");
        assert!(!config.default_config().is_empty());
    }

    #[test]
    fn a_table_that_keeps_only_a_vault_writes_no_empty_store_list() {
        let mut config = Config::default();
        config.default_config_mut().set_vault("origin");

        let toml = toml::to_string(&config).unwrap();

        assert!(!toml.contains("storages"), "{toml}");
    }
}
