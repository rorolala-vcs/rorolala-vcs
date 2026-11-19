//! The `rola bind` command: name a Vault address and point the Layout at it.
//!
//! Making a Workspace and putting it to work otherwise takes two commands, binding the address and
//! choosing the Vault the Layout tracks, and the two share one name. This is that one name said
//! once: the Vault is bound under it, and the Layout being worked in tracks it when the Layout
//! tracks nothing yet.
//!
//! The same word binds a store: a directory carrying `rolast.toml` is recorded in the store, and
//! then in the Workspace's and the Vault's own configurations, in that order.

// Binding a store reads both places it is bound to, checks the path against each, records the
// binding in the store, and writes each configuration: one ordered pass that cannot be cut without
// leaving a place bound to a store that does not record it.
#![allow(
    clippy::too_many_lines,
    reason = "the store form is one ordered pass over both places and the store"
)]

use std::path::{Path, PathBuf};

use just_fmt::fmt_case_style::CaseFormatter;
use librorolala::protocol::VaultAddress;
use librorolala::storage::RorolalaStorage;
use librorolala::vault::STORAGE_DIR as VAULT_STORAGE_DIR;
use librorolala::workspace::{DATA_DIR, STORAGE_DIR as WORKSPACE_STORAGE_DIR};
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResVaultConfig, ResWorkspace, ResWorkspaceConfig};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::{Locate as _, normalize};
use rust_i18n::t;

use crate::Next;
use crate::address::ResAddressHistory;
use crate::cmd_create::{ErrorWorkspaceNotExist, absolute};
use crate::complete::{offer, positional, strip_written, typing_flag};
use crate::error::ErrorConfigUnreadable;
use crate::exit_codes::{
    EC_ERR_CONFIG_UNREADABLE, EC_ERR_CREATION_ARGUMENT, EC_ERR_LAYOUT, EC_ERR_STORAGE_BIND,
    EC_HELP, EC_NOT_EXIST,
};
use crate::failure::failure;
use crate::layout::failed;
use crate::vault::cmd_vault_bind::{ErrorVaultAddressInvalid, ErrorVaultAddressMissing};

/// The name a Vault is bound under when none is given.
///
/// It is the name a Vault upstream is usually known by, so a run that has one Vault to share
/// through does not have to name it.
const DEFAULT_NAME: &str = "origin";

/// Leave the Vault being reached for where it is.
const ARG_NO_SET_DEFAULT_VAULT: PickerArg<'static, Flag> = arg![no_set_default_vault: Flag];

#[help(buffer)]
pub fn help_bind(_: EntryBind, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("bind.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryBind)]
pub fn desc_bind() -> Description {
    t!("bind.description").to_string().into()
}

/// Completes what `rola bind` can be given next.
///
/// The address is completed from the addresses reached before, the way `rola vault bind` answers
/// it; the name is the caller's own to choose, so nothing is offered for it.
#[completion(EntryBind)]
pub fn complete_bind(ctx: ShellContext, history: &mut LazyRes<ResAddressHistory>) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_NO_SET_DEFAULT_VAULT: t!("bind.complete.no_set_default_vault"),
            },
        );
    }

    if positional(&ctx, "bind") != 0 {
        return suggest!();
    }

    offer(&ctx, history.get_ref().iter().map(str::to_string))
}

/// Binds a Vault address under a name and points the Layout at it
///
/// The name is the Workspace's own, so the same Vault may be `origin` here and something else
/// elsewhere. It is normalised the way a name used as a directory key is, words of letters and
/// digits joined by `-`, and the name `origin` is used when none is given.
///
/// The Vault bound becomes the one the Workspace reaches for, unless `--no-set-default-vault`.
/// The Layout being worked in is made to track that Vault when it tracks nothing: a Layout that
/// already tracks one is left tracking it, since which Vault the work goes to is not this command's
/// to change out from under it.
///
/// # Errors
///
/// Renders [`ErrorWorkspaceNotExist`] when the run is not inside a Workspace,
/// [`ErrorVaultAddressMissing`] when no address was given, [`ErrorVaultAddressInvalid`] when the
/// address would not read as one, [`ErrorVaultNameInvalid`] when the name is not one a binding may
/// be given, and the configuration-unreadable or Layout-not-readable failures otherwise.
#[command(node = "bind", entry = EntryBind)]
pub fn bind(args: EntryBind) -> Next {
    let picked = args
        .pick(ARG_NO_SET_DEFAULT_VAULT)
        .pick_or_route(arg![String], || ErrorVaultAddressMissing.to_chain())
        .pick(arg![Option<String>])
        .pick(&UNMATCHED)
        .to_result();
    let (no_set_default_vault, target, name, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "bind") {
        return next;
    }

    // What the first word is: a Vault address is a name, a store is a place on this machine.
    if is_store_path(&target) {
        if name.is_some() {
            return ErrorStorageTakesNoName.into();
        }
        if matches!(no_set_default_vault, Flag::Active) {
            return ErrorFlagNotForStorage.into();
        }

        return StateBindStorage { path: target }.into();
    }

    StateBind {
        address: target,
        name: name.unwrap_or_else(|| DEFAULT_NAME.to_owned()),
        set_default_vault: matches!(no_set_default_vault, Flag::Inactive),
    }
    .into()
}

/// Whether what `rola bind` was given names a store rather than a Vault address.
///
/// The shape decides, not whether anything is there: a path that does not exist still has to get
/// "there is no such store" rather than "this address does not read". A bare word is an address,
/// which is what `rola bind origin` has always meant, so a relative directory is written `./name`.
fn is_store_path(word: &str) -> bool {
    if word.starts_with("rola://") {
        return false;
    }

    // An address with a user part, `user@host:path`, is read as an address.
    let at = word.find('@');
    let colon = word.find(':');
    if at.is_some() && (colon.is_none() || at < colon) {
        return false;
    }

    if word.starts_with(['/', '\\', '.']) {
        return true;
    }

    // A Windows drive, `C:\...`.
    let mut symbols = word.chars();
    if matches!((symbols.next(), symbols.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic())
    {
        return true;
    }

    word.contains(['/', '\\'])
}

/// The state of binding a Vault address under a name.
#[derive(Grouped)]
pub struct StateBind {
    /// The address, as it was written.
    address: String,
    /// The name the address is bound under.
    name: String,
    /// Whether the name is also made the one the Workspace reaches for.
    set_default_vault: bool,
}

#[chain]
pub fn handle_bind(
    state: StateBind,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
    history: &mut LazyRes<ResAddressHistory>,
) -> Next {
    let StateBind {
        address,
        name,
        set_default_vault,
    } = state;

    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorWorkspaceNotExist.into();
    };

    let Ok(address) = VaultAddress::parse(&address) else {
        return ErrorVaultAddressInvalid { address }.into();
    };

    // What is written down is the link the address adds up to, so a name reached for as
    // `10.0.0.1` and one reached for as `rola://10.0.0.1/` are written the same way, and
    // reading one back later is reading the same thing either time.
    let address = address.to_string();

    // The name is a directory key for what is fetched from the Vault, so it is held to the shape a
    // directory name has, the same shape a Layout's name was held to when a name was one.
    let Some(name) = normalized_name(&name) else {
        return ErrorVaultNameInvalid { name }.into();
    };

    // Report what cannot be read before anything is written: a binding that could not be saved has
    // no business leaving a Layout pointed at a Vault the Workspace does not know.
    if let ResWorkspaceConfig::Unread { path, reason } = config.get_ref() {
        return ErrorConfigUnreadable::new(path.clone(), reason.clone()).into();
    }

    // UNWRAP: a configuration that was not read was turned away above, which leaves one that can be
    // changed.
    let workspace_config = config.get_mut().config_mut().unwrap();

    history.get_mut().remember(address.clone());
    workspace_config
        .vaults_mut()
        .bind(name.clone(), address.clone());
    if set_default_vault {
        let _ = workspace_config
            .default_config_mut()
            .set_vault(name.clone());
    }

    // The Layout is made where an older Workspace adopted none, so that binding a Vault is putting a
    // Workspace to work rather than a link nothing records work against.
    if let Err(error) = workspace.layout() {
        return failed(&error);
    }

    let tracks = matches!(workspace.layout_track(), Ok(Some(track)) if track == name);
    if !tracks {
        match workspace.layout_track() {
            // A Layout that already tracks another Vault is left as it is: what the work goes to is
            // not a binding's to change, and `rola layout set-track` is what changes it.
            Ok(Some(_)) => {}
            Ok(None) => {
                if let Err(error) = workspace.set_layout_track(&name) {
                    return failed(&error);
                }
            }
            Err(error) => return failed(&error),
        }
    }
    let tracks = matches!(workspace.layout_track(), Ok(Some(track)) if track == name);

    ResultBound {
        name,
        address,
        tracking: tracks,
    }
    .into()
}

/// The state of binding a store to the place the run is in.
#[derive(Grouped)]
pub struct StateBindStorage {
    /// The store, as it was written.
    path: String,
}

#[chain]
pub fn handle_bind_storage(
    state: StateBindStorage,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
    vault: &mut LazyRes<ResVault>,
    vault_config: &mut LazyRes<ResVaultConfig>,
) -> Next {
    let StateBindStorage { path } = state;
    let path = absolute(Path::new(&path));

    // A `git-rola` project has `.rola/` but no `workspace.toml`: it is bound by `git rola bind`,
    // and treating it as a Workspace here would write a configuration the project does not use.
    let workspace_held = workspace
        .get_ref()
        .as_ref()
        .filter(|held| held.config_path().is_file());
    let vault_held = vault.get_ref().as_ref();

    if workspace_held.is_none() && vault_held.is_none() {
        return ErrorStorageNowhereToBind.into();
    }

    if let Some(held) = workspace_held {
        let data = normalize(&held.get_root().join(DATA_DIR));
        let own = normalize(&held.get_root().join(WORKSPACE_STORAGE_DIR));

        if path == own {
            return ErrorStorageIsOwn { path }.into();
        }
        if path.starts_with(&data) {
            return ErrorStorageInsideData { path }.into();
        }
    }

    if let Some(held) = vault_held {
        let root = normalize(held.get_root());
        let own = normalize(&held.get_root().join(VAULT_STORAGE_DIR));

        if path == own {
            return ErrorStorageIsOwn { path }.into();
        }
        if path.starts_with(&root) {
            return ErrorStorageInsideVault { path }.into();
        }
    }

    if !path.is_dir() {
        return ErrorStorageNotDirectory { path }.into();
    }

    let Some(store) = RorolalaStorage::at(&path) else {
        return ErrorStorageNotAStore { path }.into();
    };

    if let ResWorkspaceConfig::Unread { path, reason } = config.get_ref() {
        return ErrorConfigUnreadable::new(path.clone(), reason.clone()).into();
    }
    if let ResVaultConfig::Unread { path, reason } = vault_config.get_ref() {
        return ErrorConfigUnreadable::new(path.clone(), reason.clone()).into();
    }

    // The reverse record goes first, so a store that records nobody does not leave a place bound to
    // it and unrecorded, which is what a collecting run would take the place's content for.
    let read_only = !store.is_writable();
    let mut recorded = false;
    if !read_only {
        if let Some(held) = workspace_held {
            if let Err(error) = store.connect_workspace(held.get_root()) {
                return ErrorStorageReverseFailed {
                    path,
                    reason: error.to_string(),
                }
                .into();
            }
            recorded = true;
        }

        if let Some(held) = vault_held {
            if let Err(error) = store.connect_vault(held.get_root()) {
                return ErrorStorageReverseFailed {
                    path,
                    reason: error.to_string(),
                }
                .into();
            }
            recorded = true;
        }
    }

    let written = path.to_string_lossy().into_owned();
    let bound_workspace = if let Some(held) = workspace_held {
        let root = held.get_root().to_path_buf();
        let already = held
            .storage_set()
            .is_ok_and(|set| set.bindings().iter().any(|binding| binding.root() == path));

        if !already {
            let resource = config.get_mut();
            let Some(workspace_config) = resource.config_mut() else {
                return ErrorStorageWriteFailed {
                    path,
                    reason: "the Workspace's configuration was not read".to_owned(),
                }
                .into();
            };
            workspace_config
                .default_config_mut()
                .add_storage(written.clone());

            if let Err(error) = resource.save() {
                return ErrorStorageWriteFailed {
                    path,
                    reason: error.to_string(),
                }
                .into();
            }
        }

        Some(root)
    } else {
        None
    };

    let bound_vault = if let Some(held) = vault_held {
        let root = held.get_root().to_path_buf();
        let already = held
            .storage_set()
            .is_ok_and(|set| set.bindings().iter().any(|binding| binding.root() == path));

        if !already {
            let resource = vault_config.get_mut();
            let Some(vault_configuration) = resource.config_mut() else {
                return ErrorStorageWriteFailed {
                    path,
                    reason: "the Vault's configuration was not read".to_owned(),
                }
                .into();
            };
            vault_configuration
                .storage_config_mut()
                .add_storage(written);

            if let Err(error) = resource.save() {
                return ErrorStorageWriteFailed {
                    path,
                    reason: error.to_string(),
                }
                .into();
            }
        }

        Some(root)
    } else {
        None
    };

    ResultStorageBound {
        path,
        workspace: bound_workspace,
        vault: bound_vault,
        recorded,
        read_only,
    }
    .into()
}

/// The name a binding may be given, worked out from what a caller wrote.
///
/// A name is a directory name for what is fetched from the Vault, so it is the kebab-case form of
/// what was written: one word of letters and digits with the words joined by `-`. `My Vault` and
/// `my-vault` are the same name said two ways rather than two names, and nothing is answered when
/// what was written holds no name at all.
#[must_use]
fn normalized_name(name: &str) -> Option<String> {
    let normalized = CaseFormatter::from(name).to_kebab_case();

    (!normalized.is_empty()).then_some(normalized)
}

/// Error: the name a `rola bind` was given is not one a binding may be given.
#[derive(Grouped)]
pub struct ErrorVaultNameInvalid {
    /// The name that would not do.
    name: String,
}

impl Failure for ErrorVaultNameInvalid {
    fn name(&self) -> &'static str {
        "error_vault_name_invalid"
    }

    fn reason(&self) -> String {
        t!("bind.err_name_invalid", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorVaultNameInvalid);

#[renderer(buffer)]
pub fn render_error_vault_name_invalid(error: ErrorVaultNameInvalid, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bind.err_name_invalid_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT;
}

/// Result: a Vault address was bound under a name.
#[derive(Grouped)]
pub struct ResultBound {
    /// The name that was bound.
    name: String,
    /// The address it is bound to now.
    address: String,
    /// Whether the Layout being worked in tracks it afterwards.
    tracking: bool,
}

#[renderer(buffer)]
pub fn render_result_bound(result: ResultBound) {
    r_println!(
        "{}",
        t!(
            "bind.result_bound",
            name = result.name,
            address = result.address
        )
        .trim()
    );

    if result.tracking {
        r_println!("{}", t!("bind.result_tracking", name = result.name).trim());
    }
}

/// Error: the store form was given a name.
#[derive(Grouped)]
pub struct ErrorStorageTakesNoName;

impl Failure for ErrorStorageTakesNoName {
    fn name(&self) -> &'static str {
        "error_storage_takes_no_name"
    }

    fn reason(&self) -> String {
        t!("bind.err_storage_takes_no_name").trim().to_string()
    }
}

failure!(ErrorStorageTakesNoName);

#[renderer(buffer)]
pub fn render_error_storage_takes_no_name(error: ErrorStorageTakesNoName, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_CREATION_ARGUMENT;
}

/// Error: `--no-set-default-vault` was given with a store.
#[derive(Grouped)]
pub struct ErrorFlagNotForStorage;

impl Failure for ErrorFlagNotForStorage {
    fn name(&self) -> &'static str {
        "error_flag_not_for_storage"
    }

    fn reason(&self) -> String {
        t!("bind.err_flag_not_for_storage").trim().to_string()
    }
}

failure!(ErrorFlagNotForStorage);

#[renderer(buffer)]
pub fn render_error_flag_not_for_storage(error: ErrorFlagNotForStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_CREATION_ARGUMENT;
}

/// Error: the store named is the place's own store.
#[derive(Grouped)]
pub struct ErrorStorageIsOwn {
    /// The path that is the own store.
    path: PathBuf,
}

impl Failure for ErrorStorageIsOwn {
    fn name(&self) -> &'static str {
        "error_storage_is_own"
    }

    fn reason(&self) -> String {
        t!(
            "bind.err_storage_is_own",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageIsOwn);

#[renderer(buffer)]
pub fn render_error_storage_is_own(error: ErrorStorageIsOwn, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bind.err_storage_is_own_help").trim()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the store is inside the Workspace's own data directory.
#[derive(Grouped)]
pub struct ErrorStorageInsideData {
    /// The path that is inside it.
    path: PathBuf,
}

impl Failure for ErrorStorageInsideData {
    fn name(&self) -> &'static str {
        "error_storage_inside_data"
    }

    fn reason(&self) -> String {
        t!(
            "bind.err_storage_inside_data",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageInsideData);

#[renderer(buffer)]
pub fn render_error_storage_inside_data(error: ErrorStorageInsideData, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the store is inside the Vault's own root.
#[derive(Grouped)]
pub struct ErrorStorageInsideVault {
    /// The path that is inside it.
    path: PathBuf,
}

impl Failure for ErrorStorageInsideVault {
    fn name(&self) -> &'static str {
        "error_storage_inside_vault"
    }

    fn reason(&self) -> String {
        t!(
            "bind.err_storage_inside_vault",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageInsideVault);

#[renderer(buffer)]
pub fn render_error_storage_inside_vault(error: ErrorStorageInsideVault, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the store path is not a directory.
#[derive(Grouped)]
pub struct ErrorStorageNotDirectory {
    /// The path that is not a directory.
    path: PathBuf,
}

impl Failure for ErrorStorageNotDirectory {
    fn name(&self) -> &'static str {
        "error_storage_not_directory"
    }

    fn reason(&self) -> String {
        t!(
            "bind.err_storage_not_directory",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageNotDirectory);

#[renderer(buffer)]
pub fn render_error_storage_not_directory(error: ErrorStorageNotDirectory, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the directory is not a store.
#[derive(Grouped)]
pub struct ErrorStorageNotAStore {
    /// The directory that carries no `rolast.toml`.
    path: PathBuf,
}

impl Failure for ErrorStorageNotAStore {
    fn name(&self) -> &'static str {
        "error_storage_not_a_store"
    }

    fn reason(&self) -> String {
        t!(
            "bind.err_storage_not_a_store",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageNotAStore);

#[renderer(buffer)]
pub fn render_error_storage_not_a_store(error: ErrorStorageNotAStore, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("bind.err_storage_not_a_store_help").trim())
    );
    ec.exit_code = EC_NOT_EXIST;
}

/// Error: the run is in no place a store can be bound to.
#[derive(Grouped)]
pub struct ErrorStorageNowhereToBind;

impl Failure for ErrorStorageNowhereToBind {
    fn name(&self) -> &'static str {
        "error_storage_nowhere_to_bind"
    }

    fn reason(&self) -> String {
        t!("bind.err_storage_nowhere_to_bind").trim().to_string()
    }
}

failure!(ErrorStorageNowhereToBind);

#[renderer(buffer)]
pub fn render_error_storage_nowhere_to_bind(
    error: ErrorStorageNowhereToBind,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the store could not record the place that binds it.
#[derive(Grouped)]
pub struct ErrorStorageReverseFailed {
    /// The store that could not record it.
    path: PathBuf,
    /// Why not.
    reason: String,
}

impl Failure for ErrorStorageReverseFailed {
    fn name(&self) -> &'static str {
        "error_storage_reverse_failed"
    }

    fn reason(&self) -> String {
        t!(
            "bind.err_storage_reverse_failed",
            path = self.path.display().to_string(),
            reason = self.reason
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageReverseFailed);

#[renderer(buffer)]
pub fn render_error_storage_reverse_failed(error: ErrorStorageReverseFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

/// Error: the place's configuration could not be written.
#[derive(Grouped)]
pub struct ErrorStorageWriteFailed {
    /// The store the binding names.
    path: PathBuf,
    /// Why the configuration could not be written.
    reason: String,
}

impl Failure for ErrorStorageWriteFailed {
    fn name(&self) -> &'static str {
        "error_storage_write_failed"
    }

    fn reason(&self) -> String {
        t!(
            "bind.err_storage_write_failed",
            path = self.path.display().to_string(),
            reason = self.reason
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStorageWriteFailed);

#[renderer(buffer)]
pub fn render_error_storage_write_failed(error: ErrorStorageWriteFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_CONFIG_UNREADABLE;
}

/// Result: a store was bound to the place the run is in.
#[derive(Grouped)]
pub struct ResultStorageBound {
    /// The store that was bound.
    path: PathBuf,
    /// The Workspace it is bound to, when one is.
    workspace: Option<PathBuf>,
    /// The Vault it is bound to, when one is.
    vault: Option<PathBuf>,
    /// Whether the store recorded the binding back.
    recorded: bool,
    /// Whether the store can only be read.
    read_only: bool,
}

#[renderer(buffer)]
pub fn render_result_storage_bound(result: ResultStorageBound) {
    r_println!(
        "{}",
        t!(
            "bind.result_storage_bound",
            path = result.path.display().to_string()
        )
        .trim()
    );

    if let Some(workspace) = &result.workspace {
        r_println!(
            "{}",
            t!(
                "bind.result_storage_workspace",
                path = workspace.display().to_string()
            )
            .trim()
        );
    }

    if let Some(vault) = &result.vault {
        r_println!(
            "{}",
            t!(
                "bind.result_storage_vault",
                path = vault.display().to_string()
            )
            .trim()
        );
    }

    if result.recorded {
        r_println!("{}", t!("bind.result_storage_recorded").trim());
    }

    if result.read_only {
        r_eprintln!(
            "{}",
            t!(
                "bind.result_storage_readonly",
                path = result.path.display().to_string()
            )
            .trim()
        );
    }
}
