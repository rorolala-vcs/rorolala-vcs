use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

use librorolala::workspace::{Config as WorkspaceConfig, Workspace};
use mingling::{
    LazyInit, ProgramCollect, Wrap, macros::arg, picker::PickerArg, picker::PickerHelper,
    setup::ProgramSetup,
};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_location::Locate;

/// Global argument representing the directory of the current Workspace
///
/// Usage: `--workspace-dir="/to/workspace/path"`
pub const GLOBAL_ARG_WORKSPACE_DIR: PickerArg<'static, PathBuf> = arg![workspace_dir: PathBuf];

/// Resource representing the Workspace's configuration
///
/// The configuration is read from the file the Workspace keeps it in, and written back to
/// the same file once the program is done with it — so a change made through
/// [`config_mut`](Self::config_mut) is kept without the command having to say how.
#[derive(Debug, Default, Clone)]
pub enum ResWorkspaceConfig {
    /// No Workspace was found, so there was no configuration to read.
    #[default]
    Absent,
    /// The configuration, and the file it was read from.
    Read {
        /// The file the configuration came from.
        path: PathBuf,
        /// Whether it was changed through [`config_mut`](Self::config_mut).
        changed: bool,
        /// What it held.
        config: WorkspaceConfig,
    },
    /// A configuration is there, but could not be read.
    Unread {
        /// The file that could not be read.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
}

impl ResWorkspaceConfig {
    /// The configuration, when one was read.
    #[must_use]
    pub const fn config(&self) -> Option<&WorkspaceConfig> {
        match self {
            Self::Read { config, .. } => Some(config),
            Self::Absent | Self::Unread { .. } => None,
        }
    }

    /// The configuration, to be changed.
    ///
    /// What is changed through here is written back when the program is done with it; a
    /// configuration that is only read is left as it is.
    pub const fn config_mut(&mut self) -> Option<&mut WorkspaceConfig> {
        match self {
            Self::Read {
                changed, config, ..
            } => {
                *changed = true;
                Some(config)
            }
            Self::Absent | Self::Unread { .. } => None,
        }
    }

    /// Why the configuration could not be read, when it could not.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unread { reason, .. } => Some(reason),
            Self::Absent | Self::Read { .. } => None,
        }
    }
}

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

        // ResWorkspaceConfig
        {
            let workspace_dir_cloned = workspace_dir.clone();
            program.with_resource(
                ResWorkspaceConfig::lazy_init(move || read_workspace_config(&workspace_dir_cloned))
                    .with_on_drop(save_workspace_config),
            );
        }

        // ResWorkspaceDir
        program.with_resource(ResWorkspaceDir::from(workspace_dir));
    }
}

fn init_workspace(dir: &Path) -> ResWorkspace {
    let workspace = Workspace::locate(dir);
    ResWorkspace { workspace }
}

/// Reads the Workspace's configuration from beside it, if there is a Workspace at all.
fn read_workspace_config(dir: &Path) -> ResWorkspaceConfig {
    let Some(workspace) = Workspace::locate(dir) else {
        return ResWorkspaceConfig::Absent;
    };

    let path = workspace.config_path();
    match WorkspaceConfig::read_from(&path) {
        Ok(config) => ResWorkspaceConfig::Read {
            path,
            changed: false,
            config,
        },
        Err(error) => ResWorkspaceConfig::Unread {
            path,
            reason: error.to_string(),
        },
    }
}

/// Writes the configuration back to the file it was read from.
///
/// This runs when the program is done with the resource, and only for a configuration that
/// was actually read and then changed: a Workspace that was not found, one whose file would
/// not parse, and one that was only read have nothing to write back. A drop has nowhere to
/// report to, so a write that fails is lost with it — the file is left as it was read.
fn save_workspace_config(resource: ResWorkspaceConfig) {
    if let ResWorkspaceConfig::Read {
        path,
        changed: true,
        config,
    } = resource
    {
        let _ = config.write_to(&path);
    }
}
