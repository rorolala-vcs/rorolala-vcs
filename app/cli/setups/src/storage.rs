use std::env::current_dir;
use std::path::{Path, PathBuf};

use librorolala::storage::{StorageNote, StorageSet};
use mingling::{LazyInit, ProgramCollect, setup::ProgramSetup};
use rust_i18n::t;

/// A [`ProgramSetup`] implementation used to register Storage-related resources and behaviors
pub struct RorolalaStorageSetup;

/// The stores the current program context works on.
///
/// A run may work on a store of its own, on the stores a Vault or a Workspace binds, or on none at
/// all. A configuration that is there and does not read is its own answer rather than "no store":
/// the two lead a command to different failures, and a run that treated one as the other would
/// write where the configuration never said.
#[derive(Default, Clone)]
pub enum ResStorageSet {
    /// No store was found anywhere.
    #[default]
    Absent,
    /// The stores this run works on, in the order it reaches for them.
    Read(StorageSet),
    /// A Workspace or a Vault was found, but its configuration does not read.
    Unread {
        /// The file that does not read.
        path: PathBuf,
        /// Why it does not.
        reason: String,
    },
}

impl ResStorageSet {
    /// Returns the stores, when there are any to work on.
    #[must_use]
    pub const fn storage(&self) -> Option<&StorageSet> {
        match self {
            Self::Read(set) => Some(set),
            Self::Absent | Self::Unread { .. } => None,
        }
    }

    /// Returns `true` if a store is present, so a command that needs one has one.
    #[must_use]
    pub const fn exist(&self) -> bool {
        self.storage().is_some()
    }

    /// Why the configuration could not be read, when it could not.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unread { reason, .. } => Some(reason),
            Self::Absent | Self::Read(_) => None,
        }
    }

    /// The file a configuration could not be read from, when it could not.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Unread { path, .. } => Some(path),
            Self::Absent | Self::Read(_) => None,
        }
    }
}

impl<ThisProgram> ProgramSetup<ThisProgram> for RorolalaStorageSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        program.with_resource(ResStorageSet::lazy_init(|| {
            // UNWRAP: the working directory is this process's own; a run that cannot read it has
            // no place to look for a store, so there is no degraded answer to give.
            // `ProgramSetup::setup` returns `()`, so this setup cannot report a failure even if it
            // wanted to.
            init_storage_set(&current_dir().unwrap())
        }));
    }
}

/// The stores a run works on, looked for the way a run finds one.
///
/// A store is looked for on its own first: a directory carrying a configuration is a store, and
/// the nearest one upwards is the one the caller is *in*, whatever else the directories above it
/// hold. A run that is not *in* a store may still be inside a Vault or a Workspace, and each of
/// those names the stores it binds; the deeper one answers. Nothing found anywhere is no store at
/// all, which is a resource whose internal value is None rather than a failure: whether a store is
/// needed is a command's to know, not this one's.
fn init_storage_set(cwd: &Path) -> ResStorageSet {
    match librorolala::storage_set::for_directory(cwd) {
        Ok(Some(set)) => {
            report_notes(&set);
            ResStorageSet::Read(set)
        }
        Ok(None) => ResStorageSet::Absent,
        Err(error) => ResStorageSet::Unread {
            path: error.path().to_path_buf(),
            reason: error.to_string(),
        },
    }
}

/// Says on standard error what the set left out, once per run.
///
/// A binding that is gone or read-only is skipped so the rest of the run still works, and this is
/// the one place that knows the whole set: a command that met the same note again would say it
/// twice. `t!` is resolved after the program has chosen its language, which is why this runs from
/// the lazy initialization rather than from the setup itself.
fn report_notes(set: &StorageSet) {
    for note in set.notes() {
        match note {
            StorageNote::Unusable { .. } => eprintln!(
                "{}",
                t!(
                    "storage.dead_binding",
                    path = note.written(),
                    reason = note.reason().unwrap_or_default()
                )
            ),
            StorageNote::ReadOnly { .. } => eprintln!(
                "{}",
                t!(
                    "storage.readonly_binding",
                    path = note.root().display().to_string()
                )
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use librorolala::storage::RorolalaStorage;
    use librorolala::workspace::{Config, Workspace};
    use rorolala_utils_configure::Configure as _;
    use rorolala_utils_location::Locate as _;

    use super::{ResStorageSet, init_storage_set};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-setups-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_store_in_hand_is_the_one_a_run_works_on() {
        let parent = scratch("store");
        let root = parent.join("store");
        let _ = RorolalaStorage::create(&root);

        let found = init_storage_set(&root);

        assert!(found.exist());
        // What is found is the store the run is in, not some other one up the tree.
        assert_eq!(
            found.storage().unwrap().write_target().get_root(),
            root.as_path()
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn a_run_with_no_store_works_on_none() {
        let parent = scratch("nowhere");

        assert!(!init_storage_set(&parent).exist());

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn a_workspace_still_answers_with_the_stores_it_binds() {
        let dir = scratch("workspace");
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let shared = dir.join("shared");
        let _ = RorolalaStorage::create(&shared);

        let mut config = Config::read_from(&workspace.config_path()).unwrap();
        config
            .default_config_mut()
            .add_storage(shared.to_string_lossy());
        config.write_to(&workspace.config_path()).unwrap();

        let found = init_storage_set(&dir);

        assert_eq!(
            found.storage().unwrap().write_target().get_root(),
            shared.as_path()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_configuration_that_does_not_read_is_not_answered_as_no_store() {
        let dir = scratch("unread");
        Workspace::create(&dir).unwrap();
        fs::write(dir.join(".rola").join("workspace.toml"), "not toml").unwrap();

        assert!(matches!(
            init_storage_set(&dir),
            ResStorageSet::Unread { .. }
        ));

        let _ = fs::remove_dir_all(&dir);
    }
}
