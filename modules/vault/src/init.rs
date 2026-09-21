use rorolala_utils_configure::Config;
use rorolala_utils_lazyffi::lazyffi;
use std::fs;
use std::path::Path;

use crate::{CONFIG_PATH, CreationError, KEYS_DIR, Vault, config::MetaConfig};

#[lazyffi]
impl Vault {
    /// Creates a Vault in the specified directory
    ///
    /// The Vault is its configuration: the directory is made first, so that there is
    /// somewhere to write the configuration into — the same way a Workspace makes its own
    /// data directory before writing the configuration inside it.
    ///
    /// The directory it keeps its keys in is made beside it, so that the key pair a Vault
    /// proves itself with has somewhere to go: a Vault is not served without one, and the tool
    /// that makes one writes into a directory that has to be there already.
    ///
    /// # Errors
    ///
    /// Returns [`CreationError::DirCreateFailed`] if the directory or its keys directory
    /// cannot be created, and the configuration errors if the configuration file cannot be
    /// created inside it.
    #[lazyffi(export = create_vault)]
    pub fn create(dir: &Path) -> Result<(), CreationError> {
        fs::create_dir_all(dir).map_err(|_| CreationError::DirCreateFailed)?;
        fs::create_dir_all(dir.join(KEYS_DIR)).map_err(|_| CreationError::DirCreateFailed)?;

        let mut config = Config::<crate::config::Config>::new(dir.join(CONFIG_PATH)).map_err(
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

        // A Vault is named by the directory it is made in, and says it is new until someone
        // says otherwise about it.
        *config.vault_config_mut() = MetaConfig::made_in(dir);

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

    use crate::{CONFIG_PATH, CreationError, KEYS_DIR, Vault};

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

    #[test]
    fn creating_a_vault_makes_the_directory_its_keys_go_in() {
        let parent = scratch("keys-dir");
        let dir = parent.join("vault");

        Vault::create(&dir).unwrap();

        // A Vault is not served without a key pair of its own, and the tool that makes one
        // writes into a directory that has to be there already: making the Vault makes it.
        assert!(dir.join(KEYS_DIR).is_dir(), "{:?}", dir.join(KEYS_DIR));

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn creating_a_vault_names_it_after_the_directory_it_is_made_in() {
        let parent = scratch("named");
        let dir = parent.join("My Project");

        Vault::create(&dir).unwrap();

        // What is on disk is the name, not the directory it was read from: a Vault renamed
        // by hand, or made somewhere that was moved afterwards, keeps the name it was made
        // with.
        let read = crate::config::Config::read_from(&dir.join(CONFIG_PATH)).unwrap();
        assert_eq!(read.vault_config().name(), "My Project");
        assert_eq!(read.vault_config().description(), "New Rola Vault");

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn creating_a_vault_somewhere_no_directory_can_be_made_says_the_directory_failed() {
        let parent = scratch("dir-create");
        let blocker = parent.join("blocker");
        fs::write(&blocker, b"in the way").unwrap();

        // A file is not a directory, so there is nowhere to put the Vault at all.
        let result = Vault::create(&blocker.join("vault"));

        assert!(
            matches!(result, Err(CreationError::DirCreateFailed)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn creating_a_vault_over_a_leftover_staging_file_refuses_rather_than_replacing_it() {
        let dir = scratch("locked");

        // A staging file left by an edit that was never published: reading past it would
        // discard whatever was staged, so creating over it is refused instead.
        fs::write(dir.join(format!("{CONFIG_PATH}.lock")), b"staged").unwrap();

        let result = Vault::create(&dir);

        assert!(
            matches!(result, Err(CreationError::ConfigLocked)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creating_a_vault_where_the_configuration_cannot_be_put_in_place_says_the_publish_failed() {
        let dir = scratch("publish");

        // A directory sits where the configuration file would go, so the staged file has
        // somewhere to be written but nowhere to be published to.
        fs::create_dir_all(dir.join(CONFIG_PATH)).unwrap();

        let result = Vault::create(&dir);

        assert!(
            matches!(result, Err(CreationError::ConfigPublishFailed)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn creating_a_vault_where_the_staging_file_cannot_be_written_says_the_stage_failed() {
        use std::os::unix::fs::symlink;

        let dir = scratch("stage");
        let lock = dir.join(format!("{CONFIG_PATH}.lock"));

        // A dangling symlink: the staging file looks absent, but writing through it lands
        // in a directory that is not there.
        symlink(dir.join("missing").join("target"), &lock).unwrap();

        let result = Vault::create(&dir);

        assert!(
            matches!(result, Err(CreationError::ConfigStageFailed)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn creating_a_vault_with_a_staging_file_that_cannot_be_looked_at_says_the_cause_is_unknown() {
        use std::os::unix::fs::symlink;

        let dir = scratch("unknown");
        let lock = dir.join(format!("{CONFIG_PATH}.lock"));

        // A symlink that points at itself: looking the staging file up fails rather than
        // reporting it missing, which is a cause this taxonomy does not tell apart.
        symlink(&lock, &lock).unwrap();

        let result = Vault::create(&dir);

        assert!(
            matches!(result, Err(CreationError::UnknownError)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
