use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

use librorolala::workspace::Workspace;
use mingling::{
    LazyInit, ProgramCollect, Wrap, macros::arg, picker::PickerArg, picker::PickerHelper,
    setup::ProgramSetup,
};
use rorolala_utils_location::Locate;

/// Global argument representing the directory of the current Workspace
///
/// Usage: `--workspace-dir="/to/workspace/path"`
pub const GLOBAL_ARG_WORKSPACE_DIR: PickerArg<'static, PathBuf> = arg![workspace_dir: PathBuf];

/// A [`ProgramSetup`] implementation used to register Workspace-related resources and behaviors
pub struct WorkspaceSetup;

/// Resource representing the directory where the current Workspace is located
#[derive(Debug, Default, Clone, Wrap)]
pub struct ResWorkspaceDir(PathBuf);

/// Workspace resource, representing the Workspace that the current program context points to
///
/// If no Workspace is currently found, the internal Option value may be None
#[derive(Default, Clone, Wrap)]
pub struct ResWorkspace {
    /// The internal Workspace type
    workspace: Option<Workspace>,
}

impl ResWorkspace {
    /// Returns `true` if a Workspace is present (i.e. the internal value is not None)
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_cli_setups::ResWorkspace;
    ///
    /// let res_workspace = ResWorkspace::default();
    /// assert!(!res_workspace.exist());
    /// ```
    #[must_use]
    pub const fn exist(&self) -> bool {
        self.workspace.is_some()
    }
}

impl<ThisProgram> ProgramSetup<ThisProgram> for WorkspaceSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let workspace_dir = program
            .pick_argument(&GLOBAL_ARG_WORKSPACE_DIR)
            .unwrap_or_else(|| current_dir().unwrap());

        // ResWorkspace
        {
            let workspace_dir_cloned = workspace_dir.clone();
            program.with_resource(ResWorkspace::lazy_init(move || {
                init_workspace(&workspace_dir_cloned)
            }));
        }

        // ResWorkspaceDir
        program.with_resource(ResWorkspaceDir::from(workspace_dir));
    }
}

fn init_workspace(dir: &Path) -> ResWorkspace {
    let workspace = Workspace::locate(dir);
    ResWorkspace { workspace }
}
