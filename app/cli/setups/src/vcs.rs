use std::env::current_dir;
use std::path::Path;

use librorolala::vcs::VCSIndex;
use mingling::{LazyInit, ProgramCollect, Wrap, setup::ProgramSetup};
use rorolala_utils_location::Locate;

/// A [`ProgramSetup`] implementation used to register VCS-related resources and behaviors
pub struct VCSSetup;

/// Index resource, representing the version control index the current program context points to
///
/// The index is the record of the versions and variants a run works with, so a command that
/// reads or writes history reaches for one here. If no index is currently found, the internal
/// Option value may be None.
#[derive(Default, Clone, Wrap)]
pub struct ResVCSIndex {
    /// The internal Index type
    index: Option<VCSIndex>,
}

impl ResVCSIndex {
    /// Returns `true` if an index is present (i.e. the internal value is not None)
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_cli_setups::ResVCSIndex;
    ///
    /// let res_index = ResVCSIndex::default();
    /// assert!(!res_index.exist());
    /// ```
    #[must_use]
    pub const fn exist(&self) -> bool {
        self.index.is_some()
    }

    /// Whether this run is inside a Vault or Workspace, as an error when it is not.
    ///
    /// The index belongs to whichever of the two a run is inside, so a command that works on
    /// the index asks this first, and everything after it can take one for granted rather
    /// than asking again.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorShouldInVCSIndex`] when no index was found.
    pub const fn check(&self) -> Result<(), ErrorShouldInVCSIndex> {
        if self.exist() {
            Ok(())
        } else {
            Err(ErrorShouldInVCSIndex)
        }
    }
}

/// Error: this run is not inside a Vault or Workspace, so there is no index to work on.
///
/// The index is kept by the Vault or Workspace a run is inside, so a command that serves the
/// index has nothing to serve without one. [`ResVCSIndex::check`] is where a command asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorShouldInVCSIndex;

impl rorolala_errors::Failure for ErrorShouldInVCSIndex {
    fn name(&self) -> &'static str {
        "error_should_in_vcs_index"
    }

    /// What went wrong, in the library's own words.
    ///
    /// A library has one voice and speaks in it; what a person is shown is said where the
    /// command is, in the run's language.
    fn reason(&self) -> String {
        "this command works on the version control index, and this run is not inside a Vault \
         or Workspace that holds one"
            .to_owned()
    }
}

rorolala_errors::failure!(ErrorShouldInVCSIndex);

impl<ThisProgram> ProgramSetup<ThisProgram> for VCSSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        program.with_resource(ResVCSIndex::lazy_init(|| {
            init_vcs_index(&current_dir().unwrap())
        }));
    }
}

/// The index a run is inside, looked for the way a run finds one.
///
/// The index belongs to the nearest Vault or Workspace above the current directory, so it is
/// found by the same walk the Vault and Workspace resources make. Nothing found anywhere is no
/// index at all, which is a resource whose internal value is None rather than a failure:
/// whether an index is needed is a command's to know, not this one's.
fn init_vcs_index(cwd: &Path) -> ResVCSIndex {
    let index = VCSIndex::locate(cwd);
    ResVCSIndex { index }
}
