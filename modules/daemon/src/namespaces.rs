//! The Vaults this machine's daemon serves, by the namespace each is reached under.
//!
//! A daemon belongs to the machine rather than to a directory, so which Vaults it answers for
//! cannot be read from a working directory. It is read from a file instead: one flat table of
//! namespace to the path the Vault sits at, kept under the user's own data directory.
//!
//! The file is ordinary configuration — the same extension-named format, and the same staged
//! edit every other configuration goes through — so a daemon that dies mid-update leaves either
//! the whole old table or the whole new one, never half of each.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use rorolala_utils_configure::{Config as Staged, Configure};
use rorolala_utils_constants::{DAEMON_DIR, DAEMON_REGISTRY_FILE, USER_DATA_DIR};
use serde::{Deserialize, Serialize};

/// Where the machine's daemon looks for the Vaults it serves, if this machine names a place.
///
/// It is `~/.local/share/rola/daemon/registry.toml` on a machine that follows XDG; a machine
/// that names no local data directory has nowhere to keep the table, which is the `None`.
#[must_use]
pub fn registry_path() -> Option<PathBuf> {
    Some(
        dirs::data_local_dir()?
            .join(USER_DATA_DIR)
            .join(DAEMON_DIR)
            .join(DAEMON_REGISTRY_FILE),
    )
}

/// Whether `name` is one a namespace may be written as.
///
/// A namespace is one path segment of an address and a key in a TOML table, so it cannot carry
/// the separator or the quoting either of those would need: letters, digits, `-` and `_`, and it
/// starts with a letter or a digit.
#[must_use]
pub fn is_namespace(name: &str) -> bool {
    let mut bytes = name.bytes();

    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// The Vaults a daemon serves, by namespace.
///
/// The map is the file: what is read is what is listed and reached, in namespace order.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configure)]
#[serde(transparent)]
pub struct Namespaces {
    /// The directory each namespace names.
    entries: BTreeMap<String, PathBuf>,
}

impl Namespaces {
    /// Reads the table at `path`.
    ///
    /// A file that is not there is an empty table rather than a failure: a machine nothing has
    /// been registered on serves nothing, which is the same thing.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceError::Unreadable`] when the file is there but cannot be read or does
    /// not parse as a table.
    pub fn read_from(path: &Path) -> Result<Self, NamespaceError> {
        if !path
            .try_exists()
            .map_err(|error| NamespaceError::Unreadable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?
        {
            return Ok(Self::default());
        }

        Self::read_from_path(path)
    }

    /// Reads the table at `path` through the format the extension names.
    fn read_from_path(path: &Path) -> Result<Self, NamespaceError> {
        <Self as rorolala_utils_configure::Configure>::read_from(path).map_err(|error| {
            NamespaceError::Unreadable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })
    }

    /// Writes the table to `path`, making the directory it sits in when it is not there.
    ///
    /// The write is staged beside the file and renamed into place, so a reader sees the table
    /// before the edit or the table after it.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceError::Unreadable`] when an existing file cannot be read, and
    /// [`NamespaceError::Unwritable`] when the directory or the file cannot be written.
    pub fn write_to(&self, path: &Path) -> Result<(), NamespaceError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| NamespaceError::Unwritable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?;
        }

        let mut staged = if path
            .try_exists()
            .map_err(|error| NamespaceError::Unreadable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })? {
            Staged::read(path).map_err(|error| NamespaceError::Unreadable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?
        } else {
            Staged::new(path).map_err(|error| NamespaceError::Unwritable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?
        };

        *staged = self.clone();

        staged.write().map_err(|error| NamespaceError::Unwritable {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
        staged
            .publish()
            .map_err(|error| NamespaceError::Unwritable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })
    }

    /// The directory `namespace` names.
    #[must_use]
    pub fn get(&self, namespace: &str) -> Option<&Path> {
        self.entries.get(namespace).map(PathBuf::as_path)
    }

    /// Every namespace and the directory it names, in namespace order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.entries
            .iter()
            .map(|(namespace, path)| (namespace.as_str(), path.as_path()))
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Names `namespace` as the one `path` is reached under.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceError::InvalidNamespace`] when the name is not one a namespace may be
    /// written as, and [`NamespaceError::AlreadyRegistered`] when the name is taken.
    pub fn register(
        &mut self,
        namespace: &str,
        path: impl Into<PathBuf>,
    ) -> Result<(), NamespaceError> {
        if !is_namespace(namespace) {
            return Err(NamespaceError::InvalidNamespace(namespace.to_owned()));
        }

        if self.entries.contains_key(namespace) {
            return Err(NamespaceError::AlreadyRegistered(namespace.to_owned()));
        }

        self.entries.insert(namespace.to_owned(), path.into());

        Ok(())
    }

    /// Takes `namespace` out, and hands back the directory it named.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceError::NotFound`] when nothing is registered under the name.
    pub fn remove(&mut self, namespace: &str) -> Result<PathBuf, NamespaceError> {
        self.entries
            .remove(namespace)
            .ok_or_else(|| NamespaceError::NotFound(namespace.to_owned()))
    }
}

impl fmt::Display for Namespaces {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (namespace, path) in self.iter() {
            writeln!(formatter, "{namespace} = {}", path.display())?;
        }

        Ok(())
    }
}

/// Why a namespace table could not be read, written, or changed.
#[derive(Debug)]
pub enum NamespaceError {
    /// The name is not one a namespace may be written as.
    InvalidNamespace(String),
    /// The name is already registered.
    AlreadyRegistered(String),
    /// Nothing is registered under the name.
    NotFound(String),
    /// The file is there but cannot be read or parsed.
    Unreadable {
        /// The file that could not be read.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
    /// The file or the directory holding it cannot be written.
    Unwritable {
        /// The file that could not be written.
        path: PathBuf,
        /// Why it could not be written.
        reason: String,
    },
}

impl fmt::Display for NamespaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNamespace(name) => {
                write!(
                    formatter,
                    "`{name}` is not a namespace a Vault may be registered under"
                )
            }
            Self::AlreadyRegistered(name) => {
                write!(formatter, "a Vault is already registered under `{name}`")
            }
            Self::NotFound(name) => write!(formatter, "no Vault is registered under `{name}`"),
            Self::Unreadable { path, reason } => {
                write!(formatter, "{} could not be read: {reason}", path.display())
            }
            Self::Unwritable { path, reason } => {
                write!(
                    formatter,
                    "{} could not be written: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for NamespaceError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Namespaces, is_namespace};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-namespaces-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_namespace_is_one_segment_of_an_address() {
        assert!(is_namespace("alpha"));
        assert!(is_namespace("basic-vault"));
        assert!(is_namespace("basic_vault"));
        assert!(is_namespace("2nd"));

        assert!(!is_namespace(""));
        assert!(!is_namespace("alpha/beta"));
        assert!(!is_namespace("-alpha"));
        assert!(!is_namespace("alpha beta"));
    }

    #[test]
    fn an_absent_table_is_empty_and_a_written_one_reads_back() {
        let dir = scratch("round-trip");
        let file = dir.join("daemon/registry.toml");

        assert!(Namespaces::read_from(&file).unwrap().is_empty());

        let mut namespaces = Namespaces::default();
        namespaces.register("alpha", "/vaults/alpha").unwrap();
        namespaces.register("beta", "/vaults/beta").unwrap();
        namespaces.write_to(&file).unwrap();

        let read = Namespaces::read_from(&file).unwrap();
        assert_eq!(
            read.get("alpha"),
            Some(PathBuf::from("/vaults/alpha").as_path())
        );
        assert_eq!(read.iter().count(), 2);
    }

    #[test]
    fn a_name_may_not_be_registered_twice_or_removed_twice() {
        let mut namespaces = Namespaces::default();
        namespaces.register("alpha", "/vaults/alpha").unwrap();

        assert!(namespaces.register("alpha", "/elsewhere").is_err());
        assert_eq!(
            namespaces.remove("alpha").unwrap(),
            PathBuf::from("/vaults/alpha")
        );
        assert!(namespaces.remove("alpha").is_err());
    }
}
