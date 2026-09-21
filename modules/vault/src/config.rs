use std::ffi::OsStr;
use std::path::Path;

use just_fmt::CaseFormatter;
use rorolala_utils_configure::Configure;
use rorolala_utils_constants::VAULT_DEFAULT_PORT;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Deserializer, Serialize};

/// The name a Vault goes by when its configuration says nothing about one.
const UNKNOWN_VAULT_NAME: &str = "unknown_vault";

/// What a Vault says about itself when its configuration says nothing about it.
const UNKNOWN_VAULT_DESCRIPTION: &str = "Unnamed Vault";

/// The name a Vault is made with when the directory it is made in names nothing.
const UNNAMED_VAULT_NAME: &str = "Unnamed Vault";

/// What a Vault is made saying about itself.
const NEW_VAULT_DESCRIPTION: &str = "New Rola Vault";

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
    /// What the Vault says about itself.
    vault_config: MetaConfig,
}

impl Config {
    /// The configuration for the Vault daemon.
    #[must_use]
    pub const fn daemon_config(&self) -> &DaemonConfig {
        &self.daemon_config
    }

    /// The configuration for the Vault daemon, to be changed.
    ///
    /// A change made through here reaches the file the configuration came from, the way a
    /// change to any other part of it does.
    pub const fn daemon_config_mut(&mut self) -> &mut DaemonConfig {
        &mut self.daemon_config
    }

    /// What this Vault says about itself.
    #[must_use]
    pub const fn vault_config(&self) -> &MetaConfig {
        &self.vault_config
    }

    /// What this Vault says about itself, to be changed.
    ///
    /// A change made through here reaches the file the configuration came from, the way a
    /// change to any other part of it does.
    pub const fn vault_config_mut(&mut self) -> &mut MetaConfig {
        &mut self.vault_config
    }
}

/// What a Vault says about itself: the name it goes by, and what it is.
///
/// A name is written with letters, digits, the symbols `( ) , . _ -`, and the space between
/// its words; anything else in what was written is dropped. A new Vault is named after the
/// directory it is made in, converted to that shape — see [`vault_name_of`] — and says it is
/// new until someone says otherwise.
///
/// The name goes through [`standardized`] on the way in and on the way out, so a name is the
/// same name however it was typed, and the description is read and written **trimmed**: space
/// in front of either is how the file was typed, not part of what it says, so it is taken off
/// where the value enters and never reaches a reader.
///
/// The type is `MetaConfig` rather than `VaultConfig` because the latter is the name
/// [`Config`] crosses the C ABI under.
#[lazyffi(export = VaultMetaConfig)]
#[derive(Debug, Clone, Configure, Serialize, Deserialize)]
#[serde(default)]
pub struct MetaConfig {
    /// The name the Vault goes by.
    #[serde(deserialize_with = "standardized_name")]
    vault_name: String,
    /// What the Vault says about itself.
    #[serde(deserialize_with = "trimmed")]
    vault_description: String,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            vault_name: UNKNOWN_VAULT_NAME.to_owned(),
            vault_description: UNKNOWN_VAULT_DESCRIPTION.to_owned(),
        }
    }
}

impl MetaConfig {
    /// The name the Vault goes by.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.vault_name
    }

    /// Names the Vault.
    ///
    /// What is written is put through [`standardized`], so what is read back is the same name
    /// whatever was typed around it.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.vault_name = standardized(&name.into());
    }

    /// What the Vault says about itself.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.vault_description
    }

    /// Says what the Vault is.
    ///
    /// Trimmed the way a name is.
    pub fn set_description(&mut self, description: impl Into<String>) {
        description
            .into()
            .trim()
            .clone_into(&mut self.vault_description);
    }

    /// What a Vault made in `dir` starts out saying about itself.
    #[must_use]
    pub fn made_in(dir: &Path) -> Self {
        Self {
            vault_name: vault_name_of(dir),
            vault_description: NEW_VAULT_DESCRIPTION.to_owned(),
        }
    }
}

/// The name a Vault made in `dir` is given.
///
/// A directory names the Vault made in it, converted to the shape a name is written in:
/// `my-vault` becomes `My Vault`, and `vault2` becomes `Vault2`. A directory whose own name is
/// nothing a name can be made of — all punctuation, or none of it a letter or digit — leaves
/// nothing to convert, and the Vault falls back to the `Unnamed Vault` a Vault with no name of
/// its own has.
#[must_use]
pub fn vault_name_of(dir: &Path) -> String {
    let named = dir.file_name().and_then(OsStr::to_str).unwrap_or_default();
    let name = CaseFormatter::from(named).to_title_case();

    if name.is_empty() {
        UNNAMED_VAULT_NAME.to_owned()
    } else {
        name
    }
}

/// `name` as a Vault Name is written.
///
/// What a name is made of is letters, digits, the symbols `( ) , . _ -`, and the space between
/// its words. Everything else in what was written is dropped, runs of space are collapsed to
/// one, and the space around it is taken off — so `  My  (Rola)  Vault!!  ` and
/// `My (Rola) Vault` are one name, and neither a reader nor a writer of the file has to repair
/// it.
#[must_use]
pub fn standardized(name: &str) -> String {
    let mut standard = String::new();
    // The leading space is dropped, so the first word is pushed as though one had just been.
    let mut spaced = true;

    for symbol in name.trim().chars() {
        if symbol.is_ascii_alphanumeric() || is_name_symbol(symbol) {
            standard.push(symbol);
            spaced = false;
        } else if symbol.is_whitespace() && !spaced {
            standard.push(' ');
            spaced = true;
        }
    }

    standard
}

/// Whether `symbol` is one a name carries as itself, beside its letters and digits.
const fn is_name_symbol(symbol: char) -> bool {
    matches!(symbol, '(' | ')' | ',' | '.' | '_' | '-')
}

/// Reads a string into what a field holds: what was written, with the space around it taken
/// off.
///
/// A drop has nowhere to report to and a reader has nothing to repair, so an edit that leaves
/// space around a description is repaired where it is read rather than refused.
fn trimmed<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;

    Ok(raw.trim().to_owned())
}

/// Reads a string into a name: what was written, put into the shape a name is written in.
///
/// As [`standardized`], and for the same reason: a name that was typed around is a name that
/// was typed, not one this refuses to read.
fn standardized_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;

    Ok(standardized(&raw))
}

/// Where a daemon looks for the keys of the members it admits, when its configuration says
/// nothing: the Vault's own keys, the ones its root keeps, the user's own store, and the keys
/// `ROLA_HOME` names — nearest first.
const DEFAULT_KEY_DISCOVERY: &[KeyDiscovery] = &[
    KeyDiscovery::Vault,
    KeyDiscovery::RootVault,
    KeyDiscovery::User,
    KeyDiscovery::Env,
];

/// Configuration for the Vault daemon.
#[lazyffi(export = VaultDaemonConfig)]
#[derive(Debug, Clone, Configure, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    /// The port the daemon listens on.
    prefer_port: u16,
    /// The places the daemon looks for the keys of the members it admits.
    key_discovery: Vec<KeyDiscovery>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            prefer_port: VAULT_DEFAULT_PORT,
            key_discovery: DEFAULT_KEY_DISCOVERY.to_vec(),
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

    /// The places the daemon looks for the keys of the members it admits, in the order it
    /// looks in them.
    #[must_use]
    pub fn key_discovery(&self) -> &[KeyDiscovery] {
        &self.key_discovery
    }

    /// Chooses the places the daemon looks in, in the order it is to look in them.
    ///
    /// What is set is what the daemon searches and nothing else: a Vault that names no place
    /// admits nobody, which is a way of closing a Vault to callers without stopping it.
    pub fn set_key_discovery(&mut self, places: impl IntoIterator<Item = KeyDiscovery>) {
        self.key_discovery = places.into_iter().collect();
    }

    /// Adds a place for the daemon to look in, at the end of the order.
    ///
    /// A place that is already looked in is not looked in twice.
    pub fn add_key_discovery(&mut self, place: KeyDiscovery) {
        if !self.key_discovery.contains(&place) {
            self.key_discovery.push(place);
        }
    }

    /// Takes a place out of the order, so the daemon no longer looks in it.
    pub fn remove_key_discovery(&mut self, place: KeyDiscovery) {
        self.key_discovery.retain(|held| *held != place);
    }
}

/// A place a Vault looks for the keys of the members it admits.
///
/// Which of them a daemon looks in is [`DaemonConfig::key_discovery`]'s to say, and the order
/// it lists them in is the order they are searched: a member found in one place shadows the
/// same name in the places after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyDiscovery {
    /// The keys under the filesystem root, which are every Vault's on the machine.
    System,
    /// The keys under the user's own local data directory.
    User,
    /// The keys `ROLA_HOME` names.
    ///
    /// This is the scope whoever runs the daemon sets for themselves, so it is the one the
    /// machine's own layout does not decide.
    Env,
    /// The keys the Vault itself keeps.
    Vault,
    /// The keys the Vault's root keeps, which are what the whole tree supports.
    RootVault,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rorolala_utils_constants::VAULT_DEFAULT_PORT;

    use super::{Config, DaemonConfig, KeyDiscovery, MetaConfig, standardized, vault_name_of};

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

        // The same holds a table that is present but says nothing.
        let read: Config = toml::from_str("[daemon_config]\n").unwrap();

        assert_eq!(read.daemon_config().prefer_port(), VAULT_DEFAULT_PORT);
    }

    #[test]
    fn a_configuration_keeps_the_port_it_is_written_with() {
        let read: Config = toml::from_str("[daemon_config]\nprefer_port = 4321\n").unwrap();

        assert_eq!(read.daemon_config().prefer_port(), 4321);

        // And the port is spelled back out under the table it came from, so a caller
        // reading the file finds it where the structure says it lives.
        let written = toml::to_string(&read).unwrap();
        assert!(written.contains("[daemon_config]"), "{written}");
        assert!(written.contains("prefer_port = 4321"), "{written}");

        let read_back: Config = toml::from_str(&written).unwrap();
        assert_eq!(read_back.daemon_config().prefer_port(), 4321);
    }

    #[test]
    fn a_daemon_looks_where_it_is_told_and_nowhere_else() {
        // Nothing said: the Vault's own keys, the ones its root keeps, the user's own store,
        // and the keys `ROLA_HOME` names, nearest first.
        assert_eq!(
            DaemonConfig::default().key_discovery(),
            [
                KeyDiscovery::Vault,
                KeyDiscovery::RootVault,
                KeyDiscovery::User,
                KeyDiscovery::Env
            ]
        );

        // A table that says nothing but the port takes those too...
        let read: Config = toml::from_str("[daemon_config]\nprefer_port = 4321\n").unwrap();
        assert_eq!(read.daemon_config().key_discovery().len(), 4);

        // ...and a list that names no place is a Vault nobody can call on, rather than one
        // that falls back to the places it would have had.
        let read: Config = toml::from_str("[daemon_config]\nkey_discovery = []\n").unwrap();
        assert!(read.daemon_config().key_discovery().is_empty());
    }

    #[test]
    fn the_places_a_daemon_looks_in_are_written_as_they_are_named() {
        let read: Config =
            toml::from_str("[daemon_config]\nkey_discovery = [\"root-vault\", \"system\"]\n")
                .unwrap();

        assert_eq!(
            read.daemon_config().key_discovery(),
            [KeyDiscovery::RootVault, KeyDiscovery::System]
        );

        // What is written is what is read back, and the places are spelled the way the file
        // spells them.
        let written = toml::to_string(&read).unwrap();
        assert!(written.contains("[daemon_config]"), "{written}");
        assert!(written.contains("\"root-vault\""), "{written}");
        assert!(written.contains("\"system\""), "{written}");

        let read_back: Config = toml::from_str(&written).unwrap();
        assert_eq!(
            read_back.daemon_config().key_discovery(),
            read.daemon_config().key_discovery()
        );
    }

    #[test]
    fn the_places_a_daemon_looks_in_are_chosen_one_at_a_time() {
        fn places(daemon: &DaemonConfig) -> Vec<KeyDiscovery> {
            daemon.key_discovery().to_vec()
        }

        let mut daemon = DaemonConfig::default();

        // A place that is already looked in is not looked in twice, and one that is not is
        // looked in last.
        daemon.add_key_discovery(KeyDiscovery::Vault);
        daemon.add_key_discovery(KeyDiscovery::System);
        assert_eq!(
            places(&daemon),
            [
                KeyDiscovery::Vault,
                KeyDiscovery::RootVault,
                KeyDiscovery::User,
                KeyDiscovery::Env,
                KeyDiscovery::System
            ]
        );

        // Taking one out leaves the order of the rest alone.
        daemon.remove_key_discovery(KeyDiscovery::RootVault);
        assert_eq!(
            places(&daemon),
            [
                KeyDiscovery::Vault,
                KeyDiscovery::User,
                KeyDiscovery::Env,
                KeyDiscovery::System
            ]
        );

        // And choosing them all over again replaces what was there.
        daemon.set_key_discovery([KeyDiscovery::System]);
        assert_eq!(places(&daemon), [KeyDiscovery::System]);
    }

    #[test]
    fn a_vault_that_says_nothing_about_itself_is_unnamed() {
        // A file written before the Vault had a name, or emptied by hand, still reads: what
        // is missing is filled from the default rather than failing the whole Vault.
        let read: Config = toml::from_str("").unwrap();

        assert_eq!(read.vault_config().name(), "unknown_vault");
        assert_eq!(read.vault_config().description(), "Unnamed Vault");

        // The same holds a table that is present but says nothing.
        let read: Config = toml::from_str("[vault_config]\n").unwrap();

        assert_eq!(read.vault_config().name(), "unknown_vault");
        assert_eq!(read.vault_config().description(), "Unnamed Vault");
    }

    #[test]
    fn a_name_keeps_what_a_name_is_made_of_and_drops_the_rest() {
        // The space between the words is kept, and runs of it are collapsed to one.
        assert_eq!(standardized("  my   vault  "), "my vault");

        // Letters, digits and the symbols a name carries are kept as they were written...
        assert_eq!(
            standardized("vault2 (dev), build.1_a-b"),
            "vault2 (dev), build.1_a-b"
        );

        // ...and what a name is not made of is dropped rather than kept to be repaired.
        assert_eq!(standardized("vault!!@#"), "vault");
        assert_eq!(standardized("!!!"), "");
        assert_eq!(standardized("注意"), "");
    }

    #[test]
    fn a_name_is_read_and_written_in_the_shape_a_name_is_written_in() {
        // What was typed around a name is not part of it, and neither is a character a name is
        // not made of: reading it and writing it both put it into the shape a name has.
        let read: Config =
            toml::from_str("[vault_config]\nvault_name = \"  My  (Rola)  Vault!!  \"\n").unwrap();

        assert_eq!(read.vault_config().name(), "My (Rola) Vault");

        let mut config = Config::default();
        config.vault_config_mut().set_name("  vault  2  ");

        assert_eq!(config.vault_config().name(), "vault 2");

        let written = toml::to_string(&config).unwrap();
        assert!(written.contains("[vault_config]"), "{written}");
        assert!(written.contains("vault_name = \"vault 2\""), "{written}");

        // What is written is what is read back, so a name does not change by being saved.
        let read_back: Config = toml::from_str(&written).unwrap();
        assert_eq!(read_back.vault_config().name(), "vault 2");
    }

    #[test]
    fn a_description_is_read_and_written_trimmed() {
        let read: Config =
            toml::from_str("[vault_config]\nvault_description = \"  the one  \"\n").unwrap();

        assert_eq!(read.vault_config().description(), "the one");

        // A description is prose, so nothing in it is dropped: only the space around it is.
        let mut config = Config::default();
        config
            .vault_config_mut()
            .set_description("  the other,  in  full.  ");

        assert_eq!(config.vault_config().description(), "the other,  in  full.");

        let written = toml::to_string(&config).unwrap();
        assert!(
            written.contains("vault_description = \"the other,  in  full.\""),
            "{written}"
        );
    }

    #[test]
    fn a_vault_is_named_after_the_directory_it_is_made_in() {
        // A directory name is converted to the shape a name is written in.
        assert_eq!(vault_name_of(Path::new("/home/user/my-vault")), "My Vault");
        assert_eq!(vault_name_of(Path::new("upstream")), "Upstream");
        assert_eq!(vault_name_of(Path::new("/home/user/vault2")), "Vault2");

        // A directory that names nothing a name can be made of leaves the Vault unnamed, as
        // does one with no name of its own at all.
        assert_eq!(vault_name_of(Path::new("/tmp/***")), "Unnamed Vault");
        assert_eq!(vault_name_of(Path::new("/")), "Unnamed Vault");
    }

    #[test]
    fn the_configuration_a_vault_is_made_with_says_it_is_new() {
        let made = MetaConfig::made_in(Path::new("/home/user/my-vault"));

        assert_eq!(made.name(), "My Vault");
        assert_eq!(made.description(), "New Rola Vault");
    }
}
