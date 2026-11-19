use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use librorolala::{
    storage::RorolalaStorage,
    vault::{CONFIG_PATH, VAULTS_DIR, Vault},
    workspace::{DATA_DIR, Workspace},
};
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResUsingVault, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::STORAGE_CONFIG_PATH;
use rorolala_utils_location::{Locate, normalize};
use rust_i18n::t;

use crate::{
    Next,
    complete::positional,
    exit_codes::{
        EC_ALREADY_EXIST, EC_ERR_CREATION_ARGUMENT, EC_ERR_CREATION_STORAGE, EC_ERR_STORAGE_BIND,
        EC_HELP, EC_NOT_EXIST,
    },
    failure::failure,
};

/// Create a store instead of a Workspace.
pub const ARG_STORAGE: PickerArg<'static, Flag> = arg![storage: Flag];

#[help(buffer)]
pub fn help_create(_: EntryCreate, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("create.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryCreate)]
pub fn desc_create() -> Description {
    t!("create.cmd_create_description").to_string().into()
}

/// Completes what `rola create` can be given next.
///
/// A creation is named by a path and nothing else, so the whole of what can be offered is what
/// the filesystem holds where the path is being typed.
#[completion(EntryCreate)]
pub fn complete_create(ctx: ShellContext) -> Suggest {
    if positional(&ctx, "create") == 0 {
        Suggest::file_comp()
    } else {
        suggest!()
    }
}

#[command(entry = EntryCreate, routeify)]
pub fn create(args: EntryCreate) -> Next {
    let (storage, path) = args
        .pick(ARG_STORAGE)
        .pick_or_route(arg![PathBuf], || ErrorCreatePathNotProvided.to_chain())
        .to_result()?;

    StateCreatePath {
        path,
        storage: matches!(storage, Flag::Active),
    }
    .into()
}

/// Represents the state data naming where a creation is asked for.
///
/// A path is the whole of what a creation is given: whether a Vault, a Workspace or a store is what
/// is made is asked for beside it, so this is the state every creation starts in, and the one a
/// name for the current directory, `rola init`, reaches with as well.
#[derive(Grouped)]
pub struct StateCreatePath {
    /// The directory expected to be created.
    path: PathBuf,
    /// Whether the directory is a store rather than a Workspace.
    storage: bool,
}

impl StateCreatePath {
    /// The state a creation starts in: a path and what is to be made there.
    pub(crate) const fn new(path: PathBuf, storage: bool) -> Self {
        Self { path, storage }
    }
}

#[chain(routeify)]
pub fn handle_create(state: StateCreatePath, using_vault: &ResUsingVault) -> Next {
    let StateCreatePath { path, storage } = state;

    if storage {
        if **using_vault {
            return ErrorCreateStorageAndVault.into();
        }

        return StateCreateStoragePath { path }.into();
    }

    if **using_vault {
        StateCreateVault::from(path).into()
    } else {
        StateCreateWorkspace::from(path).into()
    }
}

/// Represents the state data used to create a store.
#[derive(Grouped, Wrap)]
pub struct StateCreateStoragePath {
    /// The directory the store is rooted at.
    path: PathBuf,
}

#[chain(routeify)]
pub fn handle_create_storage(state: StateCreateStoragePath) -> Next {
    let StateCreateStoragePath { path } = state;
    let path = absolute(&path);

    // A directory that carries a store's configuration is a store already, and one at a Workspace
    // or a Vault root would be found before the place's own store by every search that walks up.
    if path.join(STORAGE_CONFIG_PATH).is_file() {
        return ErrorStorageAlreadyExist { path }.into();
    }

    if path.join(DATA_DIR).is_dir() || path.join(CONFIG_PATH).is_file() {
        return ErrorStorageTargetIsPlace { path }.into();
    }

    if path.exists() && !path.is_dir() {
        return ErrorStorageTargetNotDirectory { path }.into();
    }

    match RorolalaStorage::try_create(&path) {
        Ok(_) => ResultStorageCreated { path }.into(),
        Err(error) => ErrorStorageCreationFailed {
            path,
            cause: error.to_string(),
        }
        .into(),
    }
}

/// `path` made absolute against the run's current directory, then normalised.
pub fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return normalize(path);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    normalize(&cwd.join(path))
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

    // Two different things are refused the same way. A run already inside a Workspace is one
    // whose place to work is taken, and a target that already holds a Workspace does not need
    // making; leaving the second to `Workspace::create` would report the Layout that could not
    // be made instead of the Workspace that is already there. The target is read the way the
    // library reads one, a directory holding a data directory, rather than by searching
    // upwards, so a Workspace made inside another is still a Workspace.
    if workspace.get_ref().exist() || dir.join(DATA_DIR).is_dir() {
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
    // from, so it is also where they are made.
    let Some(holding) = vault.get_ref().as_ref() else {
        // `Vault::create` is happy to write over a Vault that is already there, since what it
        // is handed is a configuration to write rather than a place that must be free. A run
        // that named an existing Vault asked to make one, not to have it rewritten, so it is
        // told the Vault is there, the same answer a Vault inside another gives, read the
        // same way.
        if dir.join(CONFIG_PATH).is_file() {
            return ErrorVaultAlreadyExist.into();
        }

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
pub fn render_error_create_path_not_provided(
    error: ErrorCreatePathNotProvided,
    ec: &mut ResExitCode,
) {
    // A failure is not a result: it is said on stderr and ends the run, the way every other
    // failure does, so a caller reading stdout is never handed an error as if it were the
    // answer it asked for.
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("create.path_not_provided_help").trim()));
    ec.exit_code = EC_ERR_CREATION_ARGUMENT;
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

/// Error: a store and a Vault were asked for at once.
#[derive(Grouped)]
pub struct ErrorCreateStorageAndVault;

impl Failure for ErrorCreateStorageAndVault {
    fn name(&self) -> &'static str {
        "error_create_storage_and_vault"
    }

    fn reason(&self) -> String {
        t!("create.err_storage_and_vault").trim().to_string()
    }
}

failure!(ErrorCreateStorageAndVault);

#[renderer(buffer)]
pub fn render_error_create_storage_and_vault(
    error: ErrorCreateStorageAndVault,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("create.err_storage_and_vault_help").trim())
    );
    ec.exit_code = EC_ERR_CREATION_ARGUMENT;
}

/// Error: the target is a store already.
#[derive(Grouped)]
pub struct ErrorStorageAlreadyExist {
    /// The directory that is a store.
    path: PathBuf,
}

impl Failure for ErrorStorageAlreadyExist {
    fn name(&self) -> &'static str {
        "error_storage_already_exist"
    }

    fn reason(&self) -> String {
        t!(
            "create.err_storage_exists",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageAlreadyExist);

#[renderer(buffer)]
pub fn render_error_storage_already_exist(error: ErrorStorageAlreadyExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("create.err_storage_exists_help").trim())
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: the target is a Workspace or a Vault, so it cannot be a store.
#[derive(Grouped)]
pub struct ErrorStorageTargetIsPlace {
    /// The directory that is a place.
    path: PathBuf,
}

impl Failure for ErrorStorageTargetIsPlace {
    fn name(&self) -> &'static str {
        "error_storage_target_is_place"
    }

    fn reason(&self) -> String {
        t!(
            "create.err_storage_target_is_place",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageTargetIsPlace);

#[renderer(buffer)]
pub fn render_error_storage_target_is_place(
    error: ErrorStorageTargetIsPlace,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("create.err_storage_target_is_place_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the target is not a directory, so no store can be made there.
#[derive(Grouped)]
pub struct ErrorStorageTargetNotDirectory {
    /// The path that is not a directory.
    path: PathBuf,
}

impl Failure for ErrorStorageTargetNotDirectory {
    fn name(&self) -> &'static str {
        "error_storage_target_not_directory"
    }

    fn reason(&self) -> String {
        t!(
            "create.err_storage_target_not_directory",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageTargetNotDirectory);

#[renderer(buffer)]
pub fn render_error_storage_target_not_directory(
    error: ErrorStorageTargetNotDirectory,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the store could not be made, and the reason is the filesystem's.
#[derive(Grouped)]
pub struct ErrorStorageCreationFailed {
    /// The directory the store was to be made at.
    path: PathBuf,
    /// Why it could not be.
    cause: String,
}

impl Failure for ErrorStorageCreationFailed {
    fn name(&self) -> &'static str {
        "error_storage_creation_failed"
    }

    fn reason(&self) -> String {
        t!(
            "create.err_storage_failed",
            path = self.path.display().to_string(),
            reason = self.cause
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageCreationFailed);

#[renderer(buffer)]
pub fn render_error_storage_creation_failed(
    error: ErrorStorageCreationFailed,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_CREATION_STORAGE;
}

/// Result: a store was created.
#[derive(Grouped, Wrap)]
pub struct ResultStorageCreated {
    /// The directory that was created.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_storage_created(result: ResultStorageCreated) {
    r_println!(
        "{}",
        t!(
            "create.result_storage_created",
            path = result.path.display().to_string()
        )
        .trim()
    );
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
