//! The `rola bind` command: name a Vault address and point the Layout at it.
//!
//! Making a Workspace and putting it to work otherwise takes two commands, binding the address and
//! choosing the Vault the Layout tracks, and the two share one name. This is that one name said
//! once: the Vault is bound under it, and the Layout being worked in tracks it when the Layout
//! tracks nothing yet.

use just_fmt::fmt_case_style::CaseFormatter;
use librorolala::protocol::VaultAddress;
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
use rorolala_cli_setups::{ResWorkspace, ResWorkspaceConfig};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::address::ResAddressHistory;
use crate::cmd_create::ErrorWorkspaceNotExist;
use crate::complete::{offer, positional, strip_written, typing_flag};
use crate::error::ErrorConfigUnreadable;
use crate::exit_codes::{EC_ERR_LAYOUT, EC_HELP};
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
    let (no_set_default_vault, address, name, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "bind") {
        return next;
    }

    StateBind {
        address,
        name: name.unwrap_or_else(|| DEFAULT_NAME.to_owned()),
        set_default_vault: matches!(no_set_default_vault, Flag::Inactive),
    }
    .into()
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
