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
use std::path::{Path, PathBuf};

use rorolala_utils_cli_theme::warn_line;
use rorolala_utils_configure::{Config as Staged, Configure};
use serde::{Deserialize, Serialize};

/// What this machine allows its daemon to do.
///
/// A file that is not there is a machine that has said nothing, which is what it defaults to: a
/// feature that had to be configured before it could be used once would be one nobody turns on,
/// and what the defaults say is the shape it was meant to have. What a machine that has said
/// nothing means is also *written down* the first time anything asks — see [`read`](Self::read) —
/// so that the two switches are a file a person can find and change rather than something only
/// this source knows about.
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

    /// Reads this machine's preferences, writing the defaults down when there are none.
    ///
    /// A file that is not there is a machine that has never said anything, which is the defaults;
    /// what it means is written to the file as those defaults, so that the switches are somewhere a
    /// person can find them. A file that is there is read and left alone, whatever it says.
    ///
    /// A file that cannot be written — a read-only data directory, say — is not a reason to refuse
    /// the session: the defaults are exactly what a machine that says nothing means, so the run
    /// goes on under them and the reason is said on the standard error. Only a file that is *there*
    /// and cannot be understood is a failure.
    ///
    /// # Errors
    ///
    /// Returns [`PreferenceError::Unreadable`] when the file is there and cannot be read or does
    /// not parse.
    pub fn read() -> Result<Self, PreferenceError> {
        let Some(path) = crate::preference_path() else {
            return Ok(Self::default());
        };

        Self::read_at(&path)
    }

    /// Reads the preferences at `path`, writing the defaults there when nothing is.
    fn read_at(path: &Path) -> Result<Self, PreferenceError> {
        let unreadable = |error: std::io::Error| PreferenceError::Unreadable {
            path: path.to_path_buf(),
            reason: error.to_string(),
        };

        if path.try_exists().map_err(unreadable)? {
            return <Self as Configure>::read_from(path).map_err(|error| {
                PreferenceError::Unreadable {
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                }
            });
        }

        let preference = Self::default();

        // Both failures are the same thing to a reader — the file is not there and cannot be made
        // to be — so both are carried as the sentence the filesystem or the format gave.
        let written: Result<(), String> = (|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }

            // Staged beside the name and published by rename rather than written in place: a process
            // that dies while making the file leaves either nothing or the whole of it. A half-written
            // configuration would be a file that refuses every session until somebody found it.
            let mut staged = Staged::new(path).map_err(|error| error.to_string())?;
            *staged = preference.clone();
            staged.write().map_err(|error| error.to_string())?;
            staged.publish().map_err(|error| error.to_string())
        })();

        if let Err(error) = written {
            // Nothing is lost by going on: the value in hand is what a file that is not there means.
            // What would be lost by saying nothing is the knowledge that the switches cannot be kept
            // here, which is what a person has to know to change them at all.
            eprintln!(
                "{}",
                warn_line!(
                    "The preferences at {} could not be written: {error}",
                    (path.display())
                )
            );
        }

        Ok(preference)
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
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::Preference;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-preference-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_machine_that_has_said_nothing_has_it_written_down() {
        // The directories are not there either: what is written has to make its own way to the file.
        let path = scratch("written")
            .join("rola")
            .join("daemon")
            .join("preference.toml");

        let read = Preference::read_at(&path).unwrap();

        assert!(read.security().ssh_connection());
        assert!(!read.security().auto_register_by_ssh());

        // What a machine that says nothing means is now in the file, under the names it is read by.
        let written = fs::read_to_string(&path).unwrap();

        assert!(written.contains("[security]"), "{written}");
        assert!(written.contains("ssh-connection = true"), "{written}");
        assert!(
            written.contains("auto-register-by-ssh = false"),
            "{written}"
        );

        // What was staged is published onto the name rather than left beside it, which a leftover
        // would block every later reading with.
        assert!(!path.with_file_name("preference.toml.lock").exists());

        // And a second reading is that file's rather than the defaults': one file, one answer.
        let again = Preference::read_at(&path).unwrap();
        let (first, second) = (read.security(), again.security());

        assert_eq!(second.ssh_connection(), first.ssh_connection());
        assert_eq!(second.auto_register_by_ssh(), first.auto_register_by_ssh());
    }

    #[test]
    fn a_machine_that_has_said_something_is_left_alone() {
        let path = scratch("kept").join("preference.toml");
        let said = "[security]\nssh-connection = false\nauto-register-by-ssh = true\n";
        fs::write(&path, said).unwrap();

        let read = Preference::read_at(&path).unwrap();

        assert!(!read.security().ssh_connection());
        assert!(read.security().auto_register_by_ssh());

        // A file a person wrote is read and not rewritten: what is there is what stays there.
        assert_eq!(fs::read_to_string(&path).unwrap(), said);
    }

    #[test]
    fn a_file_that_is_there_and_cannot_be_understood_is_the_one_failure() {
        let path = scratch("broken").join("preference.toml");
        fs::write(&path, "[security]\nssh-connection = 3\n").unwrap();

        assert!(Preference::read_at(&path).is_err());
    }

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
