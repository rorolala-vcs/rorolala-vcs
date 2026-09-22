use std::env::current_dir;
use std::path::Path;

use librorolala::storage::RorolalaStorage;
use librorolala::vault::Vault;
use librorolala::workspace::Workspace;
use mingling::{LazyInit, ProgramCollect, Wrap, setup::ProgramSetup};
use rorolala_utils_location::Locate;

/// A [`ProgramSetup`] implementation used to register Storage-related resources and behaviors
pub struct RorolalaStorageSetup;

/// Storage resource, representing the store the current program context points to
///
/// A store is where the objects behind a run's content are kept, so a command that reaches for
/// content reaches for one here. If no store is currently found, the internal Option value may
/// be None.
#[derive(Default, Clone, Wrap)]
pub struct ResRorolalaStorage {
    /// The internal Storage type
    storage: Option<RorolalaStorage>,
}

impl ResRorolalaStorage {
    /// Returns `true` if a storage is present (i.e. the internal value is not None)
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_cli_setups::ResRorolalaStorage;
    ///
    /// let res_storage = ResRorolalaStorage::default();
    /// assert!(!res_storage.exist());
    /// ```
    #[must_use]
    pub const fn exist(&self) -> bool {
        self.storage.is_some()
    }
}

impl<ThisProgram> ProgramSetup<ThisProgram> for RorolalaStorageSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        program.with_resource(ResRorolalaStorage::lazy_init(|| {
            init_storage(&current_dir().unwrap())
        }));
    }
}

/// The store a run works on, looked for the way a run finds one.
///
/// A store is looked for on its own first: a directory carrying a configuration is a store, and
/// the nearest one upwards is the one the caller is *in*, whatever else the directories above it
/// hold. A run that is not *in* a store may still be inside a Vault or a Workspace, and each of
/// those keeps a store of its own under its root — so one of those is found next, and the store
/// it keeps is the one the run works on. Nothing found anywhere is no store at all, which is a
/// resource whose internal value is None rather than a failure: whether a store is needed is a
/// command's to know, not this one's.
fn init_storage(cwd: &Path) -> ResRorolalaStorage {
    let storage = RorolalaStorage::locate(cwd)
        .or_else(|| Vault::locate(cwd).and_then(|vault| vault.get_current_rola_storage()))
        .or_else(|| {
            Workspace::locate(cwd).and_then(|workspace| workspace.get_current_rola_storage())
        });

    ResRorolalaStorage { storage }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{RorolalaStorage, init_storage};
    use rorolala_utils_location::Locate as _;

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

        let found = init_storage(&root);

        assert!(found.exist());
        // What is found is the store the run is in, not some other one up the tree.
        assert_eq!(found.as_ref().unwrap().get_root(), root.as_path());

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn a_run_with_no_store_works_on_none() {
        let parent = scratch("nowhere");

        assert!(!init_storage(&parent).exist());

        let _ = fs::remove_dir_all(&parent);
    }
}
