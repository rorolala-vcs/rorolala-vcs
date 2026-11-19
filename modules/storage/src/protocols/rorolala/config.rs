//! What the store is told by the file that makes it a store.
//!
//! The file is the store's own (`rolast.toml`, the same one a search finds the store by) and what
//! it says is read here rather than in the places that act on it, so a caller asks a store what it
//! was told instead of knowing where that is written down.
//!
//! The places that write to the store are recorded here too, under `[workspaces] connected` and
//! `[vaults] connected`. A collecting run reads them as its roots, which is why writing one is a
//! read-modify-write under the store's lock: two bindings made at once must not lose each other's
//! record.

use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

use rorolala_utils_location::{Locate as _, normalize};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use super::RorolalaStorage;
use crate::{Lockable as _, LockingGuard, StorageConfig, StorageConfigError};

/// The places a store records as writing to it, with their paths resolved.
///
/// A path is absolute and lexically normalised; `.` is the store's own root. The lists are in the
/// order the file writes them, which is the order a collecting run checks them in.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Connected {
    /// The Workspaces that write to this store.
    pub workspaces: Vec<String>,
    /// The Vaults that write to this store.
    pub vaults: Vec<String>,
}

impl RorolalaStorage {
    /// The configuration this store works from.
    ///
    /// A configuration that cannot be read (no file, or one that does not read) is the empty one,
    /// so what a store does not say is defaulted rather than refused.
    #[must_use]
    pub async fn config(&self) -> StorageConfig {
        tokio::fs::read_to_string(self.config_path())
            .await
            .map_or_else(
                |_| StorageConfig::default(),
                |text| StorageConfig::parse(&text),
            )
    }

    /// The largest a pack may grow before another is started.
    ///
    /// A store that says nothing about it takes
    /// [`DEFAULT_MAX_PACK_SIZE`](crate::DEFAULT_MAX_PACK_SIZE).
    #[must_use]
    pub async fn max_pack_size(&self) -> u64 {
        self.config().await.max_pack_size()
    }

    /// The places recorded as writing to this store, with their paths resolved.
    ///
    /// A list that is not there, or is empty, is the single place `.`: the store is read as one a
    /// run in it writes to. That default is put on here rather than where the file is parsed,
    /// because `.` means the directory the file sits in.
    ///
    /// # Errors
    ///
    /// Returns [`StorageConfigError::Io`] when the file is there and cannot be read.
    pub fn connected(&self) -> Result<Connected, StorageConfigError> {
        let config = self.read_config()?;
        let root = self.get_root();
        let workspaces = config.connected_workspaces();
        let vaults = config.connected_vaults();

        // A file that records nobody is a store standing on its own, and `.` is the one place that
        // still names it. The default is written once, under workspaces, rather than into both
        // lists: two entries for one place would be read as two dead bindings.
        if workspaces.is_empty() && vaults.is_empty() {
            return Ok(Connected {
                workspaces: vec![normalize(root).to_string_lossy().into_owned()],
                vaults: Vec::new(),
            });
        }

        Ok(Connected {
            workspaces: resolve(workspaces, root),
            vaults: resolve(vaults, root),
        })
    }

    /// Records `root` as a Workspace writing to this store, answering whether the file changed.
    ///
    /// # Errors
    ///
    /// Returns [`StorageConfigError::Io`] when the file cannot be read or written, or the store
    /// cannot be locked, and [`StorageConfigError::Malformed`] when it does not read as TOML. A
    /// file that does not read is refused rather than replaced.
    pub fn connect_workspace(&self, root: &Path) -> Result<bool, StorageConfigError> {
        self.record("workspaces", root, true)
    }

    /// Forgets `root` as a Workspace writing to this store.
    ///
    /// # Errors
    ///
    /// As [`connect_workspace`](Self::connect_workspace).
    pub fn disconnect_workspace(&self, root: &Path) -> Result<bool, StorageConfigError> {
        self.record("workspaces", root, false)
    }

    /// Records `root` as a Vault writing to this store, answering whether the file changed.
    ///
    /// # Errors
    ///
    /// As [`connect_workspace`](Self::connect_workspace).
    pub fn connect_vault(&self, root: &Path) -> Result<bool, StorageConfigError> {
        self.record("vaults", root, true)
    }

    /// Forgets `root` as a Vault writing to this store.
    ///
    /// # Errors
    ///
    /// As [`connect_workspace`](Self::connect_workspace).
    pub fn disconnect_vault(&self, root: &Path) -> Result<bool, StorageConfigError> {
        self.record("vaults", root, false)
    }

    /// Whether the store's configuration can be opened for writing.
    ///
    /// It answers whether a write could land here without writing anything: a read-only mount, or
    /// permissions that do not allow a write, refuse the open.
    #[must_use]
    pub fn is_writable(&self) -> bool {
        std::fs::OpenOptions::new()
            .write(true)
            .open(self.config_path())
            .is_ok()
    }

    /// The configuration as it reads now, without making anything.
    fn read_config(&self) -> Result<StorageConfig, StorageConfigError> {
        match std::fs::read_to_string(self.config_path()) {
            Ok(text) => Ok(StorageConfig::parse(&text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(StorageConfig::default())
            }
            Err(error) => Err(StorageConfigError::Io {
                path: self.config_path(),
                reason: error.to_string(),
            }),
        }
    }

    /// Adds or removes one place under `section`, keeping everything else in the file.
    fn record(&self, section: &str, root: &Path, add: bool) -> Result<bool, StorageConfigError> {
        // The lock is held over the whole read, change and write: two bindings made at once would
        // otherwise each read the list without the other's place in it, and one would be lost.
        let lock = self.lock_path();
        let _guard = LockingGuard::try_acquire(self.clone(), lock.clone()).map_err(|error| {
            StorageConfigError::Io {
                path: lock,
                reason: error.to_string(),
            }
        })?;

        let path = self.config_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => {
                return Err(StorageConfigError::Io {
                    path,
                    reason: error.to_string(),
                });
            }
        };
        let mut document: DocumentMut =
            text.parse().map_err(
                |error: toml_edit::TomlError| StorageConfigError::Malformed {
                    path: path.clone(),
                    reason: error.to_string(),
                },
            )?;

        let target = normalize(root);
        let written = target.to_string_lossy().into_owned();
        let base = self.get_root();
        let same = |value: &Value| {
            value
                .as_str()
                .is_some_and(|entry| same_path(&normalize(&base.join(entry)), &target))
        };

        let changed = if add {
            let table = entry_table(&mut document, section, &path)?;
            let array = connected_array(table, &path)?;

            if array.iter().any(&same) {
                false
            } else {
                array.push(Value::from(written.as_str()));
                true
            }
        } else {
            remove_place(&mut document, section, &same)
        };

        if !changed {
            return Ok(false);
        }

        write_atomically(&path, &document.to_string())?;

        Ok(true)
    }
}

/// The `[section]` table, made when it is not there yet.
fn entry_table<'a>(
    document: &'a mut DocumentMut,
    section: &str,
    path: &Path,
) -> Result<&'a mut Table, StorageConfigError> {
    let item = document
        .as_table_mut()
        .entry(section)
        .or_insert_with(|| Item::Table(Table::new()));

    item.as_table_mut()
        .ok_or_else(|| malformed(path, format!("`{section}` is not a table")))
}

/// The `connected` array of `table`, made when it is not there yet.
fn connected_array<'a>(
    table: &'a mut Table,
    path: &Path,
) -> Result<&'a mut Array, StorageConfigError> {
    let item = table
        .entry("connected")
        .or_insert_with(|| Item::Value(Value::Array(Array::new())));

    item.as_array_mut()
        .ok_or_else(|| malformed(path, "`connected` is not an array"))
}

/// Removes one place from `section`, answering whether anything was removed.
///
/// A list that ends up empty takes its key with it, and a table that ends up empty takes the table,
/// so binding and unbinding leave the file as clean as they found it.
fn remove_place(document: &mut DocumentMut, section: &str, same: &impl Fn(&Value) -> bool) -> bool {
    let Some(table) = document
        .as_table_mut()
        .get_mut(section)
        .and_then(Item::as_table_mut)
    else {
        return false;
    };
    let Some(array) = table.get_mut("connected").and_then(Item::as_array_mut) else {
        return false;
    };

    let before = array.len();
    array.retain(|value| !same(value));
    let changed = array.len() != before;

    if array.is_empty() {
        table.remove("connected");
    }
    if table.is_empty() {
        document.as_table_mut().remove(section);
    }

    changed
}

/// `text` as the configuration's failure on `path`.
fn malformed(path: &Path, reason: impl Into<String>) -> StorageConfigError {
    StorageConfigError::Malformed {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

/// The places of a list, resolved against `root`, with `.` for a list that says nothing.
/// The roots a written list names, made absolute. A list that is empty stays empty: the one
/// default a store records nobody with is decided by [`Connected`] as a whole, not by each list.
fn resolve(entries: &[String], root: &Path) -> Vec<String> {
    entries
        .iter()
        .map(|entry| normalize(&root.join(entry)).to_string_lossy().into_owned())
        .collect()
}

/// Whether two paths name the same place.
///
/// What can be canonicalised is, because a symbolic link and a short name are how one directory may
/// be spelled another way; what cannot is compared after lexical normalisation, so `.` and `..` in
/// what a person typed do not make two paths differ. Two spellings that differ only in ASCII case
/// are one path on Windows.
fn same_path(left: &Path, right: &Path) -> bool {
    let left = std::fs::canonicalize(left).unwrap_or_else(|_| normalize(left));
    let right = std::fs::canonicalize(right).unwrap_or_else(|_| normalize(right));

    let mut left = left.components();
    let mut right = right.components();

    loop {
        match (left.next(), right.next()) {
            (None, None) => return true,
            (Some(one), Some(other)) if same_component(one, other) => {}
            _ => return false,
        }
    }
}

/// Whether two path components name the same thing.
#[cfg(windows)]
fn same_component(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

/// Whether two path components name the same thing.
#[cfg(not(windows))]
fn same_component(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str() == right.as_os_str()
}

/// Writes `text` to `path` whole, so a run that stops halfway leaves the file it found.
fn write_atomically(path: &Path, text: &str) -> Result<(), StorageConfigError> {
    let temporary = temporary_path(path);
    let failure = |error: std::io::Error| StorageConfigError::Io {
        path: path.to_path_buf(),
        reason: error.to_string(),
    };

    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()
    })();

    if let Err(error) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(failure(error));
    }

    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(failure(error));
    }

    Ok(())
}

/// Where the whole of a configuration is written before it replaces the file.
fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("rolast.toml");

    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::RorolalaStorage;

    use super::Connected;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-storage-connected-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_place_is_recorded_once_and_forgotten_once() {
        let dir = scratch("round-trip");
        let store = RorolalaStorage::create(&dir);
        let project = dir.join("project");
        let vault = dir.join("vault");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&vault).unwrap();

        assert!(store.connect_workspace(&project).unwrap());
        assert!(!store.connect_workspace(&project).unwrap());
        assert!(store.connect_vault(&vault).unwrap());

        let connected = store.connected().unwrap();
        assert_eq!(connected.workspaces, [project.to_string_lossy()]);
        assert_eq!(connected.vaults, [vault.to_string_lossy()]);

        assert!(store.disconnect_workspace(&project).unwrap());
        assert!(!store.disconnect_workspace(&project).unwrap());

        let connected = store.connected().unwrap();
        // A side that records nobody records nobody: the one default is a store standing alone,
        // not a place added to a list that still has a Vault in it.
        assert_eq!(connected.workspaces, [] as [String; 0]);
        assert_eq!(connected.vaults, [vault.to_string_lossy()]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn editing_the_list_keeps_what_the_file_says_beside_it() {
        let dir = scratch("preserve");
        let store = RorolalaStorage::create(&dir);
        let project = dir.join("project");
        fs::create_dir_all(&project).unwrap();

        // A hand-written file with a comment and the packing setting the store uses.
        fs::write(
            dir.join("rolast.toml"),
            "# the shared store\n[storage]\nmax_pack_size = \"1MiB\"\n",
        )
        .unwrap();

        assert!(store.connect_workspace(&project).unwrap());

        let text = fs::read_to_string(dir.join("rolast.toml")).unwrap();
        assert!(text.contains("# the shared store"), "{text}");
        assert!(text.contains("max_pack_size = \"1MiB\""), "{text}");
        assert!(text.contains("[workspaces]"), "{text}");

        // Unbinding takes the table with it and leaves the rest as it was.
        assert!(store.disconnect_workspace(&project).unwrap());

        let text = fs::read_to_string(dir.join("rolast.toml")).unwrap();
        assert!(!text.contains("[workspaces]"), "{text}");
        assert!(text.contains("# the shared store"), "{text}");
        assert!(text.contains("max_pack_size = \"1MiB\""), "{text}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_does_not_read_is_refused_rather_than_replaced() {
        let dir = scratch("malformed");
        let store = RorolalaStorage::create(&dir);
        fs::write(dir.join("rolast.toml"), "this is not toml at all").unwrap();

        assert!(store.connect_workspace(&dir.join("project")).is_err());
        assert_eq!(
            fs::read_to_string(dir.join("rolast.toml")).unwrap(),
            "this is not toml at all"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_store_that_records_nobody_reads_as_the_one_place_it_is() {
        let dir = scratch("default");
        let store = RorolalaStorage::create(&dir);

        assert_eq!(
            store.connected().unwrap(),
            Connected {
                workspaces: vec![dir.to_string_lossy().into_owned()],
                vaults: Vec::new(),
            }
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_relative_place_is_resolved_from_the_configuration() {
        let dir = scratch("relative");
        let store = RorolalaStorage::create(&dir);
        let project = dir.join("project");
        fs::create_dir_all(&project).unwrap();

        fs::write(
            dir.join("rolast.toml"),
            "[workspaces]\nconnected = [\"project\"]\n",
        )
        .unwrap();

        assert_eq!(
            store.connected().unwrap().workspaces,
            [project.to_string_lossy()]
        );
        assert_eq!(store.connected().unwrap().vaults, [] as [String; 0]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_side_that_records_nobody_does_not_gain_the_store_itself() {
        let dir = scratch("one-side");
        let store = RorolalaStorage::create(&dir);
        let vault = dir.join("vault");
        fs::create_dir_all(&vault).unwrap();
        store.connect_vault(&vault).unwrap();

        let connected = store.connected().unwrap();
        assert_eq!(connected.workspaces, [] as [String; 0]);
        assert_eq!(connected.vaults, [vault.to_string_lossy()]);

        let _ = fs::remove_dir_all(&dir);
    }
}
