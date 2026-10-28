//! What this machine allows its daemon to do.
//!
//! It is the machine's rather than any one Vault's, and sits beside the table of namespaces for
//! that reason: a Vault says who may act on it, while this says what the daemon on this machine
//! may be asked to do at all before any Vault is looked at.
//!
//! It is read by the one mode in which it decides anything — serving a connection that arrived
//! through a shell — and it decides two things: whether such a connection is served at all, and
//! whether a name the Vault has never seen may be written into it. Both are switches rather than
//! limits on what a Vault admits, since a Vault's own configuration is what says that.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use rorolala_utils_configure::Configure;
use serde::{Deserialize, Serialize};

/// What this machine allows its daemon to do.
///
/// A file that is not there is a machine that has said nothing, which is what it defaults to: a
/// feature that had to be configured before it could be used once would be one nobody turns on,
/// and what the defaults say is the shape it was meant to have.
#[derive(Debug, Clone, Default, Configure, Serialize, Deserialize)]
#[serde(default)]
pub struct Preference {
    /// What a session may be served and what it may register.
    security: Security,
}

impl Preference {
    /// What a session may be served and what it may register.
    #[must_use]
    pub const fn security(&self) -> &Security {
        &self.security
    }

    /// What a session may be served and what it may register, to be changed.
    pub const fn security_mut(&mut self) -> &mut Security {
        &mut self.security
    }

    /// Reads this machine's preferences, or what a machine that has said nothing means.
    ///
    /// # Errors
    ///
    /// Returns [`PreferenceError::Unreadable`] when the file is there and cannot be read or does
    /// not parse. A file that is not there is not a failure: it is a machine that has never said
    /// anything, which is the defaults.
    pub fn read() -> Result<Self, PreferenceError> {
        let Some(path) = crate::preference_path() else {
            return Ok(Self::default());
        };

        let unreadable = |error: std::io::Error| PreferenceError::Unreadable {
            path: path.clone(),
            reason: error.to_string(),
        };

        if !path.try_exists().map_err(unreadable)? {
            return Ok(Self::default());
        }

        <Self as Configure>::read_from(&path).map_err(|error| PreferenceError::Unreadable {
            path: path.clone(),
            reason: error.to_string(),
        })
    }
}

/// What the machine allows a daemon to be asked for by a caller that arrived over SSH.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Security {
    /// Whether the daemon may be reached over SSH at all.
    ///
    /// A machine that turns this off refuses a session before any Vault is looked at, so "may I
    /// serve one" is answered once rather than once per Vault.
    ssh_connection: bool,
    /// Whether a session may write the key it proved into the Vault's keys, for a name the Vault
    /// has never seen.
    ///
    /// It is off by default. A name registered this way is registered because the far side proved
    /// it holds the key, not because anyone decided to admit it — which is a thing to turn on
    /// deliberately, and which makes deleting the key file the wrong way to revoke it.
    auto_register_by_ssh: bool,
}

impl Default for Security {
    fn default() -> Self {
        Self {
            ssh_connection: true,
            auto_register_by_ssh: false,
        }
    }
}

impl Security {
    /// Whether the daemon may be reached over SSH at all.
    #[must_use]
    pub const fn ssh_connection(&self) -> bool {
        self.ssh_connection
    }

    /// Whether a session may register the key it proved for a name the Vault has never seen.
    #[must_use]
    pub const fn auto_register_by_ssh(&self) -> bool {
        self.auto_register_by_ssh
    }

    /// Allows or refuses SSH sessions.
    pub const fn set_ssh_connection(&mut self, allowed: bool) {
        self.ssh_connection = allowed;
    }

    /// Allows or refuses a registration made by a session.
    pub const fn set_auto_register_by_ssh(&mut self, allowed: bool) {
        self.auto_register_by_ssh = allowed;
    }
}

/// Why this machine's preferences could not be read.
#[derive(Debug)]
pub enum PreferenceError {
    /// The file is there but cannot be read or parsed.
    Unreadable {
        /// The file that could not be read.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
}

impl fmt::Display for PreferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, reason } => write!(
                formatter,
                "the machine's preferences at {} could not be read: {reason}",
                path.display()
            ),
        }
    }
}

impl Error for PreferenceError {}

#[cfg(test)]
mod tests {
    use super::Preference;

    #[test]
    fn a_machine_that_says_nothing_allows_a_session_and_registers_none() {
        for said in ["", "[security]\n"] {
            let read: Preference = toml::from_str(said).unwrap();

            assert!(read.security().ssh_connection());
            assert!(!read.security().auto_register_by_ssh());
        }
    }

    #[test]
    fn the_switches_are_written_with_hyphens() {
        let read: Preference =
            toml::from_str("[security]\nssh-connection = false\nauto-register-by-ssh = true\n")
                .unwrap();

        assert!(!read.security().ssh_connection());
        assert!(read.security().auto_register_by_ssh());

        // What is read is what is written back, under the same names.
        let written = toml::to_string(&read).unwrap();

        assert!(written.contains("[security]"), "{written}");
        assert!(written.contains("ssh-connection = false"), "{written}");
        assert!(written.contains("auto-register-by-ssh = true"), "{written}");

        let read_back: Preference = toml::from_str(&written).unwrap();
        assert!(!read_back.security().ssh_connection());
        assert!(read_back.security().auto_register_by_ssh());
    }

    #[test]
    fn a_switch_that_is_not_a_yes_or_a_no_is_refused() {
        // A file that does not say what it means is not read as one that says the defaults: a
        // person who wrote this meant something, and what they meant cannot be guessed.
        assert!(toml::from_str::<Preference>("[security]\nssh-connection = \"yes\"\n").is_err());
    }
}
