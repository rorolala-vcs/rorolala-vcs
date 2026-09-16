use std::path::PathBuf;

use librorolala::{vault::Vault, workspace::Workspace};
use mingling::{
    Grouped, LazyRes, Wrap,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResUsingVault, ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::{
    Next,
    exit_codes::{EC_ALREADY_EXIST, EC_HELP, EC_NOT_EXIST},
};

#[help(buffer)]
pub fn help_create(_: EntryCreate, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("create.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryCreate)]
pub fn desc_create() -> Description {
    t!("create.cmd_create_description").to_string().into()
}

#[command(entry = EntryCreate, routeify)]
pub fn create(args: EntryCreate, using_vault: &ResUsingVault) -> Next {
    let path: PathBuf = args
        .pick_or_route(&arg![PathBuf], || ErrorCreatePathNotProvided.into())
        .to_result()?;

    if **using_vault {
        StateCreateVault { vault_dir: path }.into()
    } else {
        StateCreateWorkspace {
            workspace_dir: path,
        }
        .into()
    }
}

/// Represents the state data used to create a Vault.
#[derive(Grouped)]
pub struct StateCreateVault {
    /// The directory expected to be created.
    vault_dir: PathBuf,
}

/// Represents the state data used to create a Workspace.
#[derive(Grouped)]
pub struct StateCreateWorkspace {
    /// The directory expected to be created.
    workspace_dir: PathBuf,
}

#[chain(routeify)]
pub fn handle_create_workspace(
    state: StateCreateWorkspace,
    workspace: &mut LazyRes<ResWorkspace>, // Workspace Resource
) -> Next {
    let dir = state.workspace_dir;
    if workspace.get_ref().exist() {
        return ErrorWorkspaceAlreadyExist.into();
    }

    Workspace::create(&dir)?;
    ResultWorkspaceCreated::from(dir).into()
}

#[chain(routeify)]
pub fn handle_create_vault(
    state: StateCreateVault,
    vault: &mut LazyRes<ResVault>, // Vault Resource
) -> Next {
    let dir = state.vault_dir;
    if vault.get_ref().exist() {
        return ErrorVaultAlreadyExist.into();
    }

    Vault::create(&dir)?;
    ResultVaultCreated::from(dir).into()
}

/// Error: the create path was not provided.
#[derive(Grouped)]
pub struct ErrorCreatePathNotProvided;

#[renderer(buffer)]
pub fn render_error_create_path_not_provided(_: ErrorCreatePathNotProvided) {
    r_println!("{}", err_line!(t!("create.path_not_provided")).trim());
    r_println!("{}", help_line!(t!("create.path_not_provided_help")).trim());
}

/// Error: the vault already exists.
#[derive(Grouped)]
pub struct ErrorVaultAlreadyExist;

#[renderer(buffer)]
pub fn render_error_vault_already_exist(_: ErrorVaultAlreadyExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("common.err_vault_already_exist")).trim());
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_vault_already_exist_help")).trim()
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: the vault does not exist.
#[derive(Grouped)]
pub struct ErrorVaultNotExist;

#[renderer(buffer)]
pub fn render_error_vault_not_exist(_: ErrorVaultNotExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("common.err_vault_not_exist")).trim());
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_vault_not_exist_help")).trim()
    );
    ec.exit_code = EC_NOT_EXIST;
}

/// Error: the workspace already exists.
#[derive(Grouped)]
pub struct ErrorWorkspaceAlreadyExist;

#[renderer(buffer)]
pub fn render_error_workspace_already_exist(_: ErrorWorkspaceAlreadyExist, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("common.err_workspace_already_exist")).trim()
    );
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_workspace_already_exist_help")).trim()
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: the workspace does not exist.
#[derive(Grouped)]
pub struct ErrorWorkspaceNotExist;

#[renderer(buffer)]
pub fn render_error_workspace_not_exist(_: ErrorWorkspaceNotExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("common.err_workspace_not_exist")).trim());
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_workspace_not_exist_help")).trim()
    );
    ec.exit_code = EC_NOT_EXIST;
}

/// Result: the workspace was created.
#[derive(Grouped, Wrap)]
pub struct ResultWorkspaceCreated {
    /// The directory that was created.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_workspace_created(result: ResultWorkspaceCreated) {
    r_println!(
        "{}",
        t!(
            "create.result_workspace_created",
            path = result.path.display().to_string()
        )
        .trim()
    );
}

/// Result: the vault was created.
#[derive(Grouped, Wrap)]
pub struct ResultVaultCreated {
    /// The directory that was created.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_vault_created(result: ResultVaultCreated) {
    r_println!(
        "{}",
        t!(
            "create.result_vault_created",
            path = result.path.display().to_string()
        )
        .trim()
    );
}
