use rorolala_utils_configure::Config;
use rorolala_utils_lazyffi::lazyffi;
use std::fs;
use std::path::Path;

use crate::{CONFIG_PATH, CreationError, Vault};

#[lazyffi]
impl Vault {
    /// Creates a Vault in the specified directory
    ///
    /// The Vault is its configuration: the directory is made first, so that there is
    /// somewhere to write the configuration into — the same way a Workspace makes its own
    /// data directory before writing the configuration inside it.
    ///
    /// # Errors
    ///
    /// Returns [`CreationError::DirCreateFailed`] if the directory cannot be created, and
    /// the configuration errors if the configuration file cannot be created inside it.
    #[lazyffi(export = create_vault)]
    pub fn create(dir: &Path) -> Result<(), CreationError> {
        fs::create_dir_all(dir).map_err(|_| CreationError::DirCreateFailed)?;

        let config = Config::<crate::config::Config>::new(dir.join(CONFIG_PATH)).map_err(
            |error| match error {
                rorolala_utils_configure::Error::Locked { .. } => CreationError::ConfigLocked,
                rorolala_utils_configure::Error::Render { .. } => CreationError::ConfigRenderFailed,
                rorolala_utils_configure::Error::Stage { .. } => CreationError::ConfigStageFailed,
                rorolala_utils_configure::Error::Publish { .. } => {
                    CreationError::ConfigPublishFailed
                }
                _ => CreationError::UnknownError,
            },
        )?;

        config.write().map_err(|error| match error {
            rorolala_utils_configure::Error::Render { .. } => {
                CreationError::ConfigWriteRenderFailed
            }
            rorolala_utils_configure::Error::Stage { .. } => CreationError::ConfigWriteStageFailed,
            _ => CreationError::UnknownError,
        })?;

        // Drop config to write it to the filesystem
        drop(config);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_configure::Configure;
    use rorolala_utils_location::Locate;

    use crate::{CONFIG_PATH, Vault};

    /// A parent directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-vault-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn creating_a_vault_makes_a_missing_directory_and_a_configuration() {
        let parent = scratch("create");
        let dir = parent.join("vault");

        // The directory is deliberately not there: creating a Vault at a path is what
        // creates it, the same way it does for a Workspace.
        Vault::create(&dir).unwrap();

        let file = dir.join(CONFIG_PATH);
        assert!(file.is_file(), "{file:?}");
        assert!(Vault::locate(&dir).is_some(), "{dir:?}");
        crate::config::Config::read_from(&file).unwrap();

        let _ = fs::remove_dir_all(&parent);
    }
}
