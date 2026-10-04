use std::fs;
use std::path::{Path, PathBuf};

use rorolala_storage::RorolalaStorage;
use rorolala_utils_configure::Config;
use rorolala_utils_lazyffi::lazyffi;

use crate::{CONFIG_PATH, CreationError, DATA_DIR, INDEX_DIR, STORAGE_DIR, Workspace};

/// Marking the data directory hidden the way Windows says a directory is hidden.
///
/// A name beginning with a dot is the whole of the convention where the rest of this was written, and
/// Windows is not one of those platforms: it shows such a directory like any other, and has an attribute
/// for the answer instead. What is set here is that attribute, which is what Explorer and everything else
/// on the platform reads — so a Workspace's own things look like the program's rather than like the work's.
#[cfg(windows)]
mod hidden {
    use std::os::windows::ffi::OsStrExt as _;
    use std::path::Path;

    /// What Windows calls a file or directory that is not shown.
    pub(super) const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;

    /// What a call that answers with a path's attributes answers when it could not read them.
    const INVALID: u32 = u32::MAX;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        /// Reads what a path's attributes are.
        fn GetFileAttributesW(name: *const u16) -> u32;

        /// Says what a path's attributes are.
        fn SetFileAttributesW(name: *const u16, attributes: u32) -> i32;
    }

    /// Tries to mark `directory` hidden, answering whether Windows took it.
    ///
    /// The read is there because the write is told the whole of what a path's attributes are rather than
    /// the one bit that is wanted: setting that bit alone would take away everything else the directory
    /// carries.
    pub fn marked(directory: &Path) -> bool {
        let mut name: Vec<u16> = directory.as_os_str().encode_wide().collect();
        name.push(0);

        // SAFETY: `name` is the NUL-terminated wide string both calls are documented to take, and it
        // outlives them both.
        let attributes = unsafe { GetFileAttributesW(name.as_ptr()) };

        if attributes == INVALID {
            return false;
        }

        // SAFETY: as above, and the attributes are this call's own answer with one bit added.
        unsafe { SetFileAttributesW(name.as_ptr(), attributes | FILE_ATTRIBUTE_HIDDEN) != 0 }
    }
}

#[lazyffi(export = rola_workspace_)]
impl Workspace {
    /// Creates a Workspace in the specified directory
    ///
    /// The Workspace is its data directory: a directory is a Workspace once it holds
    /// one, and the configuration is written inside it, so the directory is made first
    /// and there is somewhere to write to.
    ///
    /// The directory its index is kept in is made with it too, so that a Workspace that
    /// exists has somewhere for the index of what it holds; see [`INDEX_DIR`].
    ///
    /// The store it keeps its objects in is made with it too, so that a Workspace that exists
    /// has somewhere to keep them; see [`Workspace::get_current_rola_storage`].
    ///
    /// No Layout is made: a Workspace that has just been created has none, and one is added —
    /// and becomes the one worked in — by whoever chooses its name.
    ///
    /// On Windows the data directory is marked hidden, which is how that platform says what the name
    /// beginning with a dot says everywhere else.
    ///
    /// # Errors
    ///
    /// Returns [`CreationError`] if the data directory
    /// cannot be created at the given directory, or if the configuration
    /// file cannot be created inside it.
    #[lazyffi(export = create_workspace)]
    pub fn create(dir: &Path) -> Result<(), CreationError> {
        let data = dir.join(DATA_DIR);

        fs::create_dir_all(&data).map_err(|_| CreationError::DataDirCreateFailed)?;

        // Nothing is done about a refusal: a directory that could not be marked is a Workspace that shows
        // rather than one that cannot be made, and what the name says is only read by a program that reads
        // what the name says.
        #[cfg(windows)]
        hidden::marked(&data);

        fs::create_dir_all(dir.join(INDEX_DIR)).map_err(|_| CreationError::DataDirCreateFailed)?;

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

        // The Workspace's store is made last, once the Workspace itself is there: a store is
        // placed under a Workspace, so there is no store to speak of until there is one to
        // place it in.
        let storage: PathBuf = dir.join(STORAGE_DIR).components().collect();
        let _ = RorolalaStorage::create(storage);

        // No Layout is made here. A Layout is a named place to work, and what a Workspace starts
        // with is the fact that it has none: the name of the first one is the caller's to choose,
        // and whether it tracks a Vault is decided with it. A Workspace with no Layout is one
        // that cannot work yet, which is said where a run asks for the Layout to work in rather
        // than guessed at here.

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::internals::Internals as _;
    use rorolala_utils_configure::Configure;
    use rorolala_utils_location::Locate;

    use crate::{CONFIG_PATH, CreationError, DATA_DIR, INDEX_DIR, STORAGE_DIR, Workspace};

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
    fn creating_a_workspace_makes_the_store_it_keeps_objects_in() {
        let dir = scratch("storage");

        Workspace::create(&dir).unwrap();

        // The store is part of a Workspace rather than something to be set up beside it: a
        // Workspace that exists has somewhere to keep objects the moment it does.
        let workspace = Workspace::locate(&dir).unwrap();
        let storage = workspace.get_current_rola_storage().unwrap();

        assert_eq!(
            storage.get_root(),
            dir.join(STORAGE_DIR)
                .components()
                .collect::<PathBuf>()
                .as_path()
        );
        assert!(storage.config_path().is_file());

        let _ = fs::remove_dir_all(&dir);
    }

    /// The data directory is hidden the way Windows says a directory is hidden, and only there: it is a
    /// platform's own notion rather than a second way of saying what the name says.
    #[cfg(windows)]
    #[test]
    fn creating_a_workspace_hides_its_data_directory() {
        use std::os::windows::fs::MetadataExt as _;

        let dir = scratch("hidden");

        Workspace::create(&dir).unwrap();

        // Asking again is what says the call itself is taken rather than refused: a directory that was
        // never marked and one that could not be are the same to read back.
        assert!(super::hidden::marked(&dir.join(DATA_DIR)));

        // And what the platform reports is what Explorer reads: the directory is there either way, and
        // the attribute is the whole of the difference.
        let attributes = fs::metadata(dir.join(DATA_DIR)).unwrap().file_attributes();

        assert_ne!(
            attributes & super::hidden::FILE_ATTRIBUTE_HIDDEN,
            0,
            "{:?}",
            dir.join(DATA_DIR)
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creating_a_workspace_makes_the_directory_its_index_goes_in() {
        let dir = scratch("index-dir");

        Workspace::create(&dir).unwrap();

        // The index is part of a Workspace rather than something to be set up beside it: a
        // Workspace that exists has somewhere to write it the moment it does.
        assert!(dir.join(INDEX_DIR).is_dir(), "{:?}", dir.join(INDEX_DIR));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creating_a_workspace_leaves_it_with_no_layout_to_work_in() {
        let dir = scratch("layout");

        Workspace::create(&dir).unwrap();

        // A name is the caller's to choose, so a Workspace that has just been made has none: what
        // makes one is adding a Layout, which is what also chooses the one being worked in.
        let layouts = Workspace::locate(&dir).unwrap().layouts();
        assert!(layouts.names().unwrap().is_empty());
        assert_eq!(layouts.current().unwrap(), None);

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
