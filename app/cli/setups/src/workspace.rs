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

/// The Vault a run reaches for, as the Workspace beside it names it.
///
/// A command that reaches for a Vault has two questions to answer: which one the caller
/// named, and — when they named none — which one the Workspace reaches for. Both answers come
/// out of the Workspace's configuration, so they are read together and handed over as an
/// address, which saves the command from loading the configuration and working them out.
///
/// The Vault is deliberately not a resource of its own here: reaching for one is dialling it,
/// and a run inside a Workspace holds no Vault to hand — the one it reaches for is elsewhere.
#[derive(Debug, Default, Clone)]
pub struct ResCurrentRemoteVault {
    /// What the Workspace says, when this run is inside one at all.
    state: RemoteState,
}

/// What the Workspace beside a run says about the Vaults a run can reach for.
#[derive(Debug, Default, Clone)]
enum RemoteState {
    /// No Workspace was found, so nothing could have named a Vault.
    #[default]
    Absent,
    /// The Workspace's configuration, as read.
    Read(WorkspaceConfig),
    /// A configuration is there, but could not be read.
    Unread {
        /// The file that could not be read.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
}

impl ResCurrentRemoteVault {
    /// The address of the Vault the Workspace reaches for.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorRemoteVault::ShouldInWorkspace`] when this run is not inside a
    /// Workspace, [`ErrorRemoteVault::Unread`] when its configuration would not read, and
    /// [`ErrorRemoteVault::NotChosen`] when it reaches for no Vault.
    pub fn vault(&self) -> Result<String, ErrorRemoteVault> {
        let config = self.reach()?;
        let name = config
            .default_config()
            .vault()
            .ok_or(ErrorRemoteVault::NotChosen)?;

        Ok(address_of(name, config))
    }

    /// The address to reach for: what `name` names, or the one the Workspace reaches for.
    ///
    /// A `name` that is empty names none. One the Workspace does not know is taken to be an
    /// address already, which is what lets a Vault be reached that was never given a name;
    /// any other answers at the address the Workspace bound it to.
    ///
    /// # Errors
    ///
    /// Returns what [`vault`](Self::vault) does: naming none is not a way out of a Workspace
    /// that has chosen none, since there is then nothing to reach for at all.
    pub fn vault_or_default(&self, name: impl Into<String>) -> Result<String, ErrorRemoteVault> {
        let name = name.into();
        if name.is_empty() {
            return self.vault();
        }

        let config = self.reach()?;

        Ok(address_of(&name, config))
    }

    /// Each Vault the Workspace knows, under the name it is known by.
    ///
    /// These are the names a caller can reach for, so they are also the ones worth offering:
    /// the same set [`vault_or_default`](Self::vault_or_default) resolves.
    ///
    /// # Errors
    ///
    /// Returns what [`vault`](Self::vault) does.
    pub fn names(&self) -> Result<Vec<&str>, ErrorRemoteVault> {
        Ok(self.reach()?.vaults().names().map(String::as_str).collect())
    }

    /// The Workspace's configuration, or why there is none to read a Vault out of.
    fn reach(&self) -> Result<&WorkspaceConfig, ErrorRemoteVault> {
        match &self.state {
            RemoteState::Read(config) => Ok(config),
            RemoteState::Absent => Err(ErrorRemoteVault::ShouldInWorkspace),
            RemoteState::Unread { path, reason } => Err(ErrorRemoteVault::Unread {
                path: path.clone(),
                reason: reason.clone(),
            }),
        }
    }
}

/// Error: the Vault a run was to reach for could not be worked out.
///
/// The ways there can be none are told apart because what is to be done about them differs:
/// work inside a Workspace, fix the configuration the Workspace keeps, or choose a Vault for
/// it to reach for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorRemoteVault {
    /// This run is not inside a Workspace, so nothing could have named a Vault.
    ShouldInWorkspace,
    /// The Workspace's configuration is there, but would not read.
    Unread {
        /// The file that could not be read.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
    /// The Workspace is here, and reaches for no Vault.
    NotChosen,
}

/// The address `name` answers at, or `name` itself when the Workspace knows no such Vault.
///
/// A name is the Workspace's own shorthand, so one it knows says where to go; one it does not
/// is taken to be an address already, which is what lets a Vault be reached that was never
/// given a name.
fn address_of(name: &str, config: &WorkspaceConfig) -> String {
    config
        .vaults()
        .get(name)
        .map_or_else(|| name.to_string(), std::string::ToString::to_string)
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

    /// Whether this run is inside a Workspace, as an error when it is not.
    ///
    /// A command that cannot do anything without a Workspace asks this first, so that
    /// everything after it can take one for granted rather than asking again.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorShouldInWorkspace`] when no Workspace was found.
    pub const fn check(&self) -> Result<(), ErrorShouldInWorkspace> {
        if self.exist() {
            Ok(())
        } else {
            Err(ErrorShouldInWorkspace)
        }
    }
}

/// Error: this run is not inside a Workspace.
///
/// A Workspace is where the work is done, so a command that works on one has nowhere to
/// work without it. [`ResWorkspace::check`] is where a command asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorShouldInWorkspace;

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

        // ResCurrentRemoteVault
        {
            let workspace_dir_cloned = workspace_dir.clone();
            program.with_resource(ResCurrentRemoteVault::lazy_init(move || {
                read_current_remote_vault(&workspace_dir_cloned)
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

/// Reads the Vaults the Workspace beside `dir` names, and the one it reaches for.
///
/// It is read the way the configuration resource is, through the same function, so there is
/// one reading of the file to keep right. What comes back is a snapshot: nothing here writes
/// the file, and a command that only reaches for a Vault never holds the configuration.
fn read_current_remote_vault(dir: &Path) -> ResCurrentRemoteVault {
    let state = match read_workspace_config(dir) {
        ResWorkspaceConfig::Read { config, .. } => RemoteState::Read(config),
        ResWorkspaceConfig::Unread { path, reason } => RemoteState::Unread { path, reason },
        ResWorkspaceConfig::Absent => RemoteState::Absent,
    };

    ResCurrentRemoteVault { state }
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
