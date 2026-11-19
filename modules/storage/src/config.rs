//! The configuration a store works from.
//!
//! A store is told how to hold what it holds by the file that makes its directory one,
//! `rolast.toml`, and this is what that file says. What is not said is defaulted rather than
//! refused: a store that says nothing about a thing takes the ordinary answer, so a file that is
//! missing, empty, or written in a way this build does not know still leaves a store that works.

use serde::Deserialize;
use size::Size;

/// The largest a pack may grow before another is started, when the configuration says nothing.
///
/// Packing is the store's own business, and packs are largely a matter of how many files a directory
/// has to hold, so the limit is generous: it is there to bound one file rather than to squeeze a
/// store.
pub const DEFAULT_MAX_PACK_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// What the configuration of a store says.
///
/// Only the sections a store is told by are named here; one that is not there is defaulted, so a
/// configuration says what it cares about and nothing else.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StorageConfig {
    /// What the store says about keeping packs.
    #[serde(default)]
    storage: StorageSection,
    /// The Workspaces that write to this store.
    #[serde(default)]
    workspaces: ConnectedSection,
    /// The Vaults that write to this store.
    #[serde(default)]
    vaults: ConnectedSection,
}

/// What the configuration says about packing.
#[derive(Debug, Clone, Default, Deserialize)]
struct StorageSection {
    /// The largest a pack may grow before another is started, written as a size, `"2GiB"`.
    max_pack_size: Option<String>,
}

/// The places a store records as writing to it, under one kind.
///
/// A store that does not say who writes to it is read as one used by a single place, `.`, which is
/// the store the run itself is in. That default is put on where the paths are resolved, not here,
/// because `.` means the directory the configuration sits in and this text does not know it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConnectedSection {
    /// The roots of the places that write here, as the file writes them.
    #[serde(default)]
    connected: Vec<String>,
}

impl ConnectedSection {
    /// The roots that write to this store, as the file writes them.
    #[must_use]
    pub fn connected(&self) -> &[String] {
        &self.connected
    }
}

impl StorageConfig {
    /// Reads a configuration from the text of a `rolast.toml`.
    ///
    /// A file that does not read as one is a configuration that says nothing, which is a store that
    /// takes the ordinary answers rather than one that does not work: what a store is told is not
    /// worth refusing to open a store over.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        toml::from_str(text).unwrap_or_default()
    }

    /// The largest a pack may grow before another is started.
    ///
    /// A size the file writes that this build does not read, or one that is nothing at all, is
    /// answered with [`DEFAULT_MAX_PACK_SIZE`], for the same reason a file that does not read is.
    #[must_use]
    pub fn max_pack_size(&self) -> u64 {
        self.storage
            .max_pack_size
            .as_deref()
            .and_then(|written| Size::from_str(written.trim()).ok())
            .and_then(|size| u64::try_from(size.bytes()).ok())
            .filter(|bytes| *bytes > 0)
            .unwrap_or(DEFAULT_MAX_PACK_SIZE)
    }

    /// The Workspaces that write to this store, as the file writes them.
    #[must_use]
    pub fn connected_workspaces(&self) -> &[String] {
        self.workspaces.connected()
    }

    /// The Vaults that write to this store, as the file writes them.
    #[must_use]
    pub fn connected_vaults(&self) -> &[String] {
        self.vaults.connected()
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MAX_PACK_SIZE, StorageConfig};

    #[test]
    fn a_size_the_configuration_writes_is_the_one_it_means() {
        assert_eq!(
            StorageConfig::parse("[storage]\nmax_pack_size = \"2GiB\"").max_pack_size(),
            2 * 1024 * 1024 * 1024
        );
        assert_eq!(
            StorageConfig::parse("[storage]\nmax_pack_size = \"64MiB\"").max_pack_size(),
            64 * 1024 * 1024
        );
        // Base-10 units are their own sizes, and spacing around what is written is not part of it.
        assert_eq!(
            StorageConfig::parse("[storage]\nmax_pack_size = \" 1MB \"").max_pack_size(),
            1_000_000
        );
    }

    #[test]
    fn a_configuration_that_says_nothing_is_defaulted() {
        // Empty, other sections only, a size that does not read, a size that is nothing, and a file
        // that is not a configuration at all: every one of them is the ordinary answer.
        for text in [
            "",
            "[other]\nkey = 1",
            "[storage]\nmax_pack_size = \"a great many\"",
            "[storage]\nmax_pack_size = \"0B\"",
            "this is not toml at all",
        ] {
            assert_eq!(
                StorageConfig::parse(text).max_pack_size(),
                DEFAULT_MAX_PACK_SIZE,
                "{text:?}"
            );
        }
    }

    #[test]
    fn the_places_a_store_records_as_writing_to_it_are_read() {
        let config = StorageConfig::parse(
            "[workspaces]\nconnected = [\"/home/alice/project\"]\n\
             [vaults]\nconnected = [\"/srv/rola/vault\", \"../other\"]\n",
        );

        assert_eq!(config.connected_workspaces(), ["/home/alice/project"]);
        assert_eq!(config.connected_vaults(), ["/srv/rola/vault", "../other"]);
    }

    #[test]
    fn a_store_that_records_nobody_records_nothing_here() {
        // The `["."]` a missing list means is put on where the paths are resolved, so the parser
        // answers the empty slice it read rather than a path it cannot resolve.
        let config = StorageConfig::parse("[storage]\nmax_pack_size = \"1MiB\"");

        assert_eq!(config.connected_workspaces(), [] as [String; 0]);
        assert_eq!(config.connected_vaults(), [] as [String; 0]);
    }

    #[test]
    fn one_place_can_be_recorded_for_workspaces_and_vaults_at_once() {
        let config = StorageConfig::parse(
            "[workspaces]\nconnected = [\"/srv/project\"]\n\
             [vaults]\nconnected = [\"/srv/vault\"]\n",
        );

        assert_eq!(config.connected_workspaces(), ["/srv/project"]);
        assert_eq!(config.connected_vaults(), ["/srv/vault"]);
    }
}
