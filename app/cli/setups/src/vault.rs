use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

use librorolala::vault::Vault;
use mingling::{
    LazyInit, ProgramCollect, Wrap, macros::arg, picker::PickerArg, picker::PickerHelper,
    setup::ProgramSetup,
};
use rorolala_utils_location::Locate;

/// Global argument representing the directory of the current Vault
///
/// Usage: `--vault-dir="/to/vault/path"`
pub const GLOBAL_ARG_VAULT_DIR: PickerArg<'static, PathBuf> = arg![vault_dir: PathBuf];

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
}

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

        // ResVaultDir
        program.with_resource(ResVaultDir::from(vault_dir));
    }
}

fn init_vault(dir: &Path) -> ResVault {
    let vault = Vault::locate(dir);
    ResVault { vault }
}
