//! The `rola bind` command: name a Vault address and start a Layout on it.
//!
//! Making a Workspace and putting it to work otherwise takes three commands — binding the
//! address, choosing the Vault to reach for, and making a Layout that tracks it — and the three
//! share one name. This is that one name said once: the Vault is bound under it, a Layout of the
//! same name is made to track it, and the Workspace works in that Layout when it had none.

use librorolala::layout::normalize_name;
use librorolala::protocol::VaultAddress;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
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
use crate::layout::{ErrorLayoutExists, failed};
use crate::vault::cmd_vault_bind::{ErrorVaultAddressInvalid, ErrorVaultAddressMissing};

/// The name a Vault is bound under and a Layout is made with when none is given.
///
/// It is the name a Vault upstream is usually known by, so a run that has one Vault to share
/// through does not have to name it.
const DEFAULT_NAME: &str = "origin";

/// Leave the Vault being reached for where it is.
const ARG_NO_SET_DEFAULT_VAULT: PickerArg<'static, Flag> = arg![no_set_default_vault: Flag];

/// Leave the Layout being worked in where it is, even when the Workspace had none.
const ARG_NO_SET_LAYOUT: PickerArg<'static, Flag> = arg![no_set_layout: Flag];

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
                ARG_NO_SET_LAYOUT: t!("bind.complete.no_set_layout"),
            },
        );
    }

    if positional(&ctx, "bind") != 0 {
        return suggest!();
    }

    offer(&ctx, history.get_ref().iter().map(str::to_string))
}

/// Binds a Vault address under a name and makes a Layout of that name
///
/// The name is the Workspace's own, so the same Vault may be `origin` here and something else
/// elsewhere. It is normalised the way a Layout's name is — words of letters and digits joined by
/// `-` — and the name `origin` is used when none is given.
///
/// The name is one thing said once: the Vault is bound under it, and a Layout by it is made to
/// track that Vault. Binding a name the Workspace has already bound changes what it means, the way
/// `rola vault bind` does; a name a Layout already holds is refused before anything is written,
/// since a Layout keeps its own directory and nothing could be merged into it.
///
/// The Vault bound becomes the one the Workspace reaches for, unless `--no-set-default-vault`.
/// Making the first Layout is also choosing what is worked in; `--no-set-layout` leaves the
/// Workspace working in nothing.
///
/// # Errors
///
/// Renders [`ErrorWorkspaceNotExist`] when the run is not inside a Workspace,
/// [`ErrorVaultAddressMissing`] when no address was given, [`ErrorVaultAddressInvalid`] when the
/// address would not read as one, [`ErrorVaultNameInvalid`] when the name is not one a Layout may
/// be given, [`ErrorLayoutExists`] when a Layout already holds the name, and the configuration-unreadable or Layout-not-worked failures otherwise.
#[command(node = "bind", entry = EntryBind)]
pub fn bind(args: EntryBind) -> Next {
    let picked = args
        .pick(&ARG_NO_SET_DEFAULT_VAULT)
        .pick(&ARG_NO_SET_LAYOUT)
        .pick_or_route(&arg![String], || ErrorVaultAddressMissing.into())
        .pick(&arg![Option<String>])
        .to_result();
    let (no_set_default_vault, no_set_layout, address, name) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateBind {
        address,
        name: name.unwrap_or_else(|| DEFAULT_NAME.to_owned()),
        set_default_vault: matches!(no_set_default_vault, Flag::Inactive),
        set_layout: matches!(no_set_layout, Flag::Inactive),
    }
    .into()
}

/// The state of binding a Vault address under a name.
#[derive(Grouped)]
pub struct StateBind {
    /// The address, as it was written.
    address: String,
    /// The name the address is bound under, and the Layout is made with.
    name: String,
    /// Whether the name is also made the one the Workspace reaches for.
    set_default_vault: bool,
    /// Whether making the Layout may also choose it as the one worked in.
    set_layout: bool,
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
        set_layout,
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

    // What a Layout may be called is what the name has to be, since a Layout by it is made: the two
    // are one name, so refusing the second half of the command is refusing the whole of it.
    let Some(name) = normalize_name(&name) else {
        return ErrorVaultNameInvalid { name }.into();
    };

    let layouts = workspace.layouts();

    // A Layout that tracks this very Vault is the one an earlier binding made under this name, and
    // binding the name again is how its address is changed — the same as `rola vault bind`. Any
    // other Layout holding the name is one this cannot make: a Layout keeps its own directory, and
    // nothing could be merged into one that is already somebody's work.
    //
    // The check comes first and nothing is written before it, so a name this cannot make leaves the
    // Workspace exactly as it was found rather than bound and then refused. Whether the Layout is
    // there is the whole of what it decides: one that is not there is made below, and one that is
    // there is left alone when it is this binding's own.
    let held = layouts.contains(&name);
    if held {
        match layouts.track(&name) {
            Ok(Some(track)) if track == name => {}
            Ok(_) => return ErrorLayoutExists.into(),
            Err(error) => return failed(&error),
        }
    }
    let was_empty = match layouts.names() {
        Ok(names) => names.is_empty(),
        Err(error) => return failed(&error),
    };

    // Report what cannot be read before anything is written: a binding that could not be saved has
    // no business taking a name that a Layout is then made for.
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

    // A Layout that is already there is the one this binding made, and is left as it is: making it
    // again would be refused by the Layout that is there, and its tracking is what it should be.
    if !held {
        if let Err(error) = layouts.create(&name) {
            return failed(&error);
        }
        if let Err(error) = layouts.set_track(&name, &name) {
            return failed(&error);
        }
    }
    if was_empty
        && set_layout
        && let Err(error) = layouts.set_current(&name)
    {
        return failed(&error);
    }

    // What the Layout being worked in is now, which is what a run wants to be told: a binding that
    // made the first Layout is one there is now work in, and one into a Workspace already working
    // somewhere else is not.
    let working = matches!(layouts.current(), Ok(Some(current)) if current == name);

    ResultBound {
        name,
        address,
        working,
    }
    .into()
}

/// Error: the name a `rola bind` was given is not one a Layout may be given.
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

/// Result: a Vault address was bound under a name and a Layout was made.
#[derive(Grouped)]
pub struct ResultBound {
    /// The name that was bound, and the Layout that was made with it.
    name: String,
    /// The address it is bound to now.
    address: String,
    /// Whether the Layout is the one now worked in.
    working: bool,
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

    if result.working {
        r_println!("{}", t!("bind.result_working", name = result.name).trim());
    }
}
