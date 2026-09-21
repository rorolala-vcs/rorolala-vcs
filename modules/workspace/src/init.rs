use std::fs;
use std::path::Path;

use rorolala_utils_configure::Config;
use rorolala_utils_lazyffi::lazyffi;

use crate::{CONFIG_PATH, CreationError, DATA_DIR, Workspace};

#[lazyffi]
impl Workspace {
    /// Creates a Workspace in the specified directory
    ///
    /// The Workspace is its data directory: a directory is a Workspace once it holds
    /// one, and the configuration is written inside it, so the directory is made first
    /// and there is somewhere to write to.
    ///
    /// # Errors
    ///
    /// Returns [`CreationError`] if the data directory
    /// cannot be created at the given directory, or if the configuration
    /// file cannot be created inside it.
    #[lazyffi(export = create_workspace)]
    pub fn create(dir: &Path) -> Result<(), CreationError> {
        fs::create_dir_all(dir.join(DATA_DIR)).map_err(|_| CreationError::DataDirCreateFailed)?;

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

    use crate::{CONFIG_PATH, CreationError, DATA_DIR, Workspace};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-workspace-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn creating_a_workspace_leaves_a_configuration_it_can_read() {
        let dir = scratch("create");

        Workspace::create(&dir).unwrap();

        let file = dir.join(CONFIG_PATH);

        // The data directory is what makes the directory a Workspace, and the
        // configuration is inside it.
        assert!(dir.join(DATA_DIR).is_dir(), "{dir:?}");
        assert!(file.is_file(), "{file:?}");
        assert!(Workspace::locate(&dir).is_some(), "{dir:?}");

        // And what was written is what reading it back expects to find: a configuration
        // that holds nothing yet still has to survive the format it was written in.
        crate::config::Config::read_from(&file).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creating_a_workspace_where_the_data_directory_cannot_be_made_says_the_data_directory_failed()
    {
        let parent = scratch("data-dir-create");
        let blocker = parent.join("blocker");
        fs::write(&blocker, b"in the way").unwrap();

        // A file cannot hold a data directory, so there is nowhere to make the Workspace.
        let result = Workspace::create(&blocker);

        assert!(
            matches!(result, Err(CreationError::DataDirCreateFailed)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn creating_a_workspace_over_a_leftover_staging_file_refuses_rather_than_replacing_it() {
        let dir = scratch("locked");
        let lock = dir.join(format!("{CONFIG_PATH}.lock"));
        fs::create_dir_all(lock.parent().unwrap()).unwrap();

        // A staging file left by an edit that was never published: reading past it would
        // discard whatever was staged, so creating over it is refused instead.
        fs::write(&lock, b"staged").unwrap();

        let result = Workspace::create(&dir);

        assert!(
            matches!(result, Err(CreationError::ConfigLocked)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creating_a_workspace_where_the_configuration_cannot_be_put_in_place_says_the_publish_failed()
    {
        let dir = scratch("publish");

        // A directory sits where the configuration file would go, so the staged file has
        // somewhere to be written but nowhere to be published to.
        fs::create_dir_all(dir.join(CONFIG_PATH)).unwrap();

        let result = Workspace::create(&dir);

        assert!(
            matches!(result, Err(CreationError::ConfigPublishFailed)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn creating_a_workspace_where_the_staging_file_cannot_be_written_says_the_stage_failed() {
        use std::os::unix::fs::symlink;

        let dir = scratch("stage");
        let lock = dir.join(format!("{CONFIG_PATH}.lock"));
        fs::create_dir_all(lock.parent().unwrap()).unwrap();

        // A dangling symlink: the staging file looks absent, but writing through it lands
        // in a directory that is not there.
        symlink(lock.parent().unwrap().join("missing").join("target"), &lock).unwrap();

        let result = Workspace::create(&dir);

        assert!(
            matches!(result, Err(CreationError::ConfigStageFailed)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn creating_a_workspace_with_a_staging_file_that_cannot_be_looked_at_says_the_cause_is_unknown()
    {
        use std::os::unix::fs::symlink;

        let dir = scratch("unknown");
        let lock = dir.join(format!("{CONFIG_PATH}.lock"));
        fs::create_dir_all(lock.parent().unwrap()).unwrap();

        // A symlink that points at itself: looking the staging file up fails rather than
        // reporting it missing, which is a cause this taxonomy does not tell apart.
        symlink(&lock, &lock).unwrap();

        let result = Workspace::create(&dir);

        assert!(
            matches!(result, Err(CreationError::UnknownError)),
            "{result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
