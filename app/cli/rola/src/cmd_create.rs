use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use librorolala::{
    vault::{CONFIG_PATH, VAULTS_DIR, Vault},
    workspace::Workspace,
};
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
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate;
use rust_i18n::t;

use crate::{
    Next,
    exit_codes::{EC_ALREADY_EXIST, EC_ERR_CREATION_ARGUMENT, EC_HELP, EC_NOT_EXIST},
    failure::failure,
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
pub fn create(args: EntryCreate) -> Next {
    let path: PathBuf = args
        .pick_or_route(&arg![PathBuf], || ErrorCreatePathNotProvided.into())
        .to_result()?;

    StateCreatePath::from(path).into()
}

/// Represents the state data naming where a creation is asked for.
///
/// A path is the whole of what a creation is given: whether a Vault or a Workspace is what is
/// made is asked for beside it, so this is the state every creation starts in, and the one a
/// name for the current directory — `rola init` — reaches with as well.
#[derive(Grouped, Wrap)]
pub struct StateCreatePath {
    /// The directory expected to be created.
    path: PathBuf,
}

#[chain(routeify)]
pub fn handle_create(state: StateCreatePath, using_vault: &ResUsingVault) -> Next {
    let path = state.path;

    if **using_vault {
        StateCreateVault::from(path).into()
    } else {
        StateCreateWorkspace::from(path).into()
    }
}

/// Represents the state data used to create a Vault.
#[derive(Grouped, Wrap)]
pub struct StateCreateVault {
    /// The directory expected to be created.
    vault_dir: PathBuf,
}

/// Represents the state data used to create a Workspace.
#[derive(Grouped, Wrap)]
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

    // Being inside a Vault is not a reason to refuse: a Vault is what others sit inside, so one
    // asked for from within a Vault is a Vault inside it rather than a second of its own. What a
    // Vault holds are the directories under its `vaults/`, which is where `RootVault` reads them
    // from — so it is also where they are made.
    let Some(holding) = vault.get_ref().as_ref() else {
        Vault::create(&dir)?;
        return ResultVaultCreated::from(dir).into();
    };

    // What is made inside a Vault is named by the directory it takes under `vaults/`, so a path
    // is not something it can be given: a Vault inside this one has exactly one place to be.
    let Some(name) = vault_name(&dir) else {
        return ErrorSubVaultNameIsPath {
            name: dir.display().to_string(),
        }
        .into();
    };

    // The path is walked back into a plain one, because the directory a Vault holds them in
    // is written `./vaults/`: joining it on leaves that `./` in the middle of the path, and a
    // path is worth reporting as the place it is rather than as the pieces it was built from.
    let sub: PathBuf = holding
        .get_root()
        .join(VAULTS_DIR)
        .join(name)
        .components()
        .collect();

    if sub.join(CONFIG_PATH).is_file() {
        return ErrorVaultAlreadyExist.into();
    }

    Vault::create(&sub)?;
    ResultVaultCreated::from(sub).into()
}

/// The name a Vault inside another is given, when what was given is a name rather than a path.
///
/// A Vault held inside another is reached by the directory it takes under `vaults/`, so one
/// segment is the whole of what names it. `.`, `..`, a separator, and nothing at all are all
/// ways of saying where rather than what, and a Vault inside another cannot be pointed at.
fn vault_name(path: &Path) -> Option<&OsStr> {
    let mut parts = path.components();

    match (parts.next(), parts.next()) {
        (Some(Component::Normal(name)), None) => Some(name),
        _ => None,
    }
}

/// Error: the create path was not provided.
#[derive(Grouped)]
pub struct ErrorCreatePathNotProvided;

impl Failure for ErrorCreatePathNotProvided {
    fn name(&self) -> &'static str {
        "error_create_path_not_provided"
    }

    fn reason(&self) -> String {
        t!("create.path_not_provided").trim().to_string()
    }
}

failure!(ErrorCreatePathNotProvided);

#[renderer(buffer)]
pub fn render_error_create_path_not_provided(error: ErrorCreatePathNotProvided) {
    r_println!("{}", err_line!(error.reason()).trim());
    r_println!("{}", help_line!(t!("create.path_not_provided_help").trim()));
}

/// Error: what was given to name a Vault inside another is a path, not a name.
#[derive(Grouped)]
pub struct ErrorSubVaultNameIsPath {
    /// What was given instead of a name.
    name: String,
}

impl Failure for ErrorSubVaultNameIsPath {
    fn name(&self) -> &'static str {
        "error_sub_vault_name_is_path"
    }

    fn reason(&self) -> String {
        t!("create.sub_vault_name_is_path", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorSubVaultNameIsPath);

#[renderer(buffer)]
pub fn render_error_sub_vault_name_is_path(error: ErrorSubVaultNameIsPath, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("create.sub_vault_name_is_path_help").trim())
    );
    ec.exit_code = EC_ERR_CREATION_ARGUMENT;
}

/// Error: the vault already exists.
#[derive(Grouped)]
pub struct ErrorVaultAlreadyExist;

impl Failure for ErrorVaultAlreadyExist {
    fn name(&self) -> &'static str {
        "error_vault_already_exist"
    }

    fn reason(&self) -> String {
        t!("common.err_vault_already_exist").trim().to_string()
    }
}

failure!(ErrorVaultAlreadyExist);

#[renderer(buffer)]
pub fn render_error_vault_already_exist(error: ErrorVaultAlreadyExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_vault_already_exist_help").trim())
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: the vault does not exist.
#[derive(Grouped)]
pub struct ErrorVaultNotExist;

impl Failure for ErrorVaultNotExist {
    fn name(&self) -> &'static str {
        "error_vault_not_exist"
    }

    fn reason(&self) -> String {
        t!("common.err_vault_not_exist").trim().to_string()
    }
}

failure!(ErrorVaultNotExist);

#[renderer(buffer)]
pub fn render_error_vault_not_exist(error: ErrorVaultNotExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_vault_not_exist_help").trim())
    );
    ec.exit_code = EC_NOT_EXIST;
}

/// Error: the workspace already exists.
#[derive(Grouped)]
pub struct ErrorWorkspaceAlreadyExist;

impl Failure for ErrorWorkspaceAlreadyExist {
    fn name(&self) -> &'static str {
        "error_workspace_already_exist"
    }

    fn reason(&self) -> String {
        t!("common.err_workspace_already_exist").trim().to_string()
    }
}

failure!(ErrorWorkspaceAlreadyExist);

#[renderer(buffer)]
pub fn render_error_workspace_already_exist(
    error: ErrorWorkspaceAlreadyExist,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_workspace_already_exist_help").trim())
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: the workspace does not exist.
#[derive(Grouped)]
pub struct ErrorWorkspaceNotExist;

impl Failure for ErrorWorkspaceNotExist {
    fn name(&self) -> &'static str {
        "error_workspace_not_exist"
    }

    fn reason(&self) -> String {
        t!("common.err_workspace_not_exist").trim().to_string()
    }
}

failure!(ErrorWorkspaceNotExist);

#[renderer(buffer)]
pub fn render_error_workspace_not_exist(error: ErrorWorkspaceNotExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("common.err_workspace_not_exist_help").trim())
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
