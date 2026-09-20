use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

use librorolala::vault::{Config as VaultConfig, Vault};
use mingling::{
    LazyInit, ProgramCollect, Wrap, macros::arg, picker::PickerArg, picker::PickerHelper,
    setup::ProgramSetup,
};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_location::Locate;

/// Global argument representing the directory of the current Vault
///
/// Usage: `--vault-dir="/to/vault/path"`
pub const GLOBAL_ARG_VAULT_DIR: PickerArg<'static, PathBuf> = arg![vault_dir: PathBuf];

/// Resource representing the Vault's configuration
///
/// The configuration is read from the file the Vault keeps it in, and written back to the
/// same file once the program is done with it — so a change made through
/// [`config_mut`](Self::config_mut) is kept without the command having to say how.
#[derive(Debug, Default, Clone)]
pub enum ResVaultConfig {
    /// No Vault was found, so there was no configuration to read.
    #[default]
    Absent,
    /// The configuration, and the file it was read from.
    Read {
        /// The file the configuration came from.
        path: PathBuf,
        /// Whether it was changed through [`config_mut`](Self::config_mut).
        changed: bool,
        /// What it held.
        config: VaultConfig,
    },
    /// A configuration is there, but could not be read.
    Unread {
        /// The file that could not be read.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
}

impl ResVaultConfig {
    /// The configuration, when one was read.
    #[must_use]
    pub const fn config(&self) -> Option<&VaultConfig> {
        match self {
            Self::Read { config, .. } => Some(config),
            Self::Absent | Self::Unread { .. } => None,
        }
    }

    /// The configuration, to be changed.
    ///
    /// What is changed through here is written back when the program is done with it; a
    /// configuration that is only read is left as it is.
    pub const fn config_mut(&mut self) -> Option<&mut VaultConfig> {
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

/// A [`ProgramSetup`] implementation used to register Vault-related resources and behaviors
pub struct VaultSetup;

/// Resource representing the directory where the current Vault is located
#[derive(Debug, Default, Clone, Wrap)]
pub struct ResVaultDir(PathBuf);

/// Vault resource, representing the Vault that the current program context points to
///
/// If no Vault is currently found, the internal Option value may be None
#[derive(Default, Clone, Wrap)]
pub struct ResVault {
    /// The internal Vault type
    vault: Option<Vault>,
}

impl ResVault {
    /// Returns `true` if a Vault is present (i.e. the internal value is not None)
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_cli_setups::ResVault;
    ///
    /// let res_vault = ResVault::default();
    /// assert!(!res_vault.exist());
    /// ```
    #[must_use]
    pub const fn exist(&self) -> bool {
        self.vault.is_some()
    }

    /// Whether this run is inside a Vault, as an error when it is not.
    ///
    /// A command that works on a Vault rather than through one asks this first, so that
    /// everything after it can take one for granted rather than asking again.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorShouldInVault`] when no Vault was found.
    pub const fn check(&self) -> Result<(), ErrorShouldInVault> {
        if self.exist() {
            Ok(())
        } else {
            Err(ErrorShouldInVault)
        }
    }
}

/// Error: this run is not inside a Vault.
///
/// A Vault is the other side of the work, so a command that serves one has nothing to serve
/// without it. [`ResVault::check`] is where a command asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorShouldInVault;

impl<ThisProgram> ProgramSetup<ThisProgram> for VaultSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let vault_dir = program
            .pick_argument(&GLOBAL_ARG_VAULT_DIR)
            .unwrap_or_else(|| current_dir().unwrap());

        // ResVault
        {
            let vault_dir_cloned = vault_dir.clone();
            program.with_resource(ResVault::lazy_init(move || init_vault(&vault_dir_cloned)));
        }

        // ResVaultConfig
        {
            let vault_dir_cloned = vault_dir.clone();
            program.with_resource(
                ResVaultConfig::lazy_init(move || read_vault_config(&vault_dir_cloned))
                    .with_on_drop(save_vault_config),
            );
        }

        // ResVaultDir
        program.with_resource(ResVaultDir::from(vault_dir));
    }
}

fn init_vault(dir: &Path) -> ResVault {
    let vault = Vault::locate(dir);
    ResVault { vault }
}

/// Reads the Vault's configuration from beside it, if there is a Vault at all.
fn read_vault_config(dir: &Path) -> ResVaultConfig {
    let Some(vault) = Vault::locate(dir) else {
        return ResVaultConfig::Absent;
    };

    let path = vault.config_path();
    match VaultConfig::read_from(&path) {
        Ok(config) => ResVaultConfig::Read {
            path,
            changed: false,
            config,
        },
        Err(error) => ResVaultConfig::Unread {
            path,
            reason: error.to_string(),
        },
    }
}

/// Writes the configuration back to the file it was read from.
///
/// This runs when the program is done with the resource, and only for a configuration that
/// was actually read and then changed: a Vault that was not found, one whose file would not
/// parse, and one that was only read have nothing to write back. A drop has nowhere to
/// report to, so a write that fails is lost with it — the file is left as it was read.
fn save_vault_config(resource: ResVaultConfig) {
    if let ResVaultConfig::Read {
        path,
        changed: true,
        config,
    } = resource
    {
        let _ = config.write_to(&path);
    }
}
