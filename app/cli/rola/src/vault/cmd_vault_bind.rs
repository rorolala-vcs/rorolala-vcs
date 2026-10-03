//! The `rola vault bind` command: give a Workspace name to a Vault address.

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
use rorolala_cli_setups::ResWorkspaceConfig;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::address::ResAddressHistory;
use crate::cmd_create::ErrorWorkspaceNotExist;
use crate::complete::{offer, positional, strip_written, typing_flag};
use crate::error::{ErrorConfigUnreadable, ErrorVaultNameMissing};
use crate::exit_codes::{EC_ERR_VAULT_ARGUMENT, EC_HELP};
use crate::failure::failure;

/// The `--set-default` of `rola vault bind`.
///
/// The name is written once, here: the parse reads it and the completion offers it, so the
/// two cannot come to disagree about what the flag is called.
const ARG_SET_DEFAULT: PickerArg<'static, Flag> = arg![set_default: Flag];

#[help(buffer)]
pub fn help_vault_bind(_: EntryVaultBind, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_bind.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultBind)]
pub fn desc_vault_bind() -> Description {
    t!("vault_bind.cmd_vault_bind_description")
        .to_string()
        .into()
}

/// Binds a name to a Vault address, or changes what the name means.
///
/// The name is the Workspace's own, so the same Vault may be `origin` here and something
/// else elsewhere. Naming one that is already bound is how its address is changed. The
/// address is remembered for completion, whether or not the name had been bound before.
///
/// With `--set-default`, binding also [chooses](crate::vault::cmd_vault_set_default::vault_set_default) the name to be
/// reached for, which saves saying so twice.
#[command(node = "vault.bind")]
pub fn vault_bind(args: EntryVaultBind) -> Next {
    let picked = args
        .pick(&ARG_SET_DEFAULT)
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .pick_or_route(&arg![String], || ErrorVaultAddressMissing.into())
        .to_result();
    let (set_default, name, address) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    // Picking a `Flag` cannot fail, so this is `Active` only when it was written.
    StateVaultBind {
        name,
        address,
        set_default: matches!(set_default, Flag::Active),
    }
    .into()
}
/// The state of binding a name to a Vault address.
#[derive(Grouped)]
pub struct StateVaultBind {
    /// The name the address is bound to.
    name: String,
    /// The address, as it was written.
    address: String,
    /// Whether the name is also made the one the Workspace reaches for.
    set_default: bool,
}

#[chain]
pub fn handle_vault_bind(
    binding: StateVaultBind,
    config: &mut LazyRes<ResWorkspaceConfig>,
    history: &mut LazyRes<ResAddressHistory>,
) -> Next {
    let StateVaultBind {
        name,
        address,
        set_default,
    } = binding;

    let Ok(address) = VaultAddress::parse(&address) else {
        return ErrorVaultAddressInvalid { address }.into();
    };

    // What is written down is the link the address adds up to, so a name reached for as
    // `10.0.0.1` and one reached for as `rola://10.0.0.1/` are written the same way, and
    // reading one back later is reading the same thing either time.
    let address = address.to_string();

    match config.get_mut() {
        state @ ResWorkspaceConfig::Read { .. } => {
            // UNWRAP: the arm this is in shows there is a configuration to change.
            let workspace = state.config_mut().unwrap();
            history.get_mut().remember(address.clone());
            let replaced = workspace.vaults_mut().bind(name.clone(), address.clone());

            // The name is bound by now, so choosing it is choosing one that can be reached
            // for.
            let made_default = set_default;
            if made_default {
                let _ = workspace.default_config_mut().set_vault(name.clone());
            }

            ResultVaultBound {
                name,
                address,
                replaced,
                made_default,
            }
            .into()
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
    }
}

/// Completes what `rola vault bind` can be given next.
///
/// A word that starts a flag is answered with the flags the command takes, read from the same
/// `PickerArg` the parse reads; the address is completed from the addresses reached before. The
/// name is deliberately left alone, since a name is the caller's to choose.
#[completion(EntryVaultBind)]
pub fn complete_vault_bind(ctx: ShellContext, history: &mut LazyRes<ResAddressHistory>) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_SET_DEFAULT: t!("vault_bind.complete.set_default"),
            },
        );
    }

    if positional(&ctx, "bind") != 1 {
        return suggest!();
    }

    offer(&ctx, history.get_ref().iter().map(str::to_string))
}

/// Result: a name was bound to an address.
#[derive(Grouped)]
pub struct ResultVaultBound {
    /// The name that was bound.
    name: String,
    /// The address it is bound to now.
    address: String,
    /// The address it was bound to before, if it was bound at all.
    replaced: Option<String>,
    /// Whether the name was also chosen to be reached for.
    made_default: bool,
}

#[renderer(buffer)]
pub fn render_result_vault_bound(result: ResultVaultBound) {
    let rendered = if let Some(previous) = result.replaced {
        t!(
            "vault_bind.result_changed",
            name = result.name,
            previous = previous,
            address = result.address
        )
    } else {
        t!(
            "vault_bind.result_bound",
            name = result.name,
            address = result.address
        )
    };

    r_println!("{}", rendered.trim());

    if result.made_default {
        r_println!(
            "{}",
            t!("vault_set_default.result_chosen", name = result.name).trim()
        );
    }
}

/// Error: the address a `rola vault bind` was given is missing.
///
/// It is shared with `rola bind`, which reads an address the same way; what is missing is the same
/// thing either time, and only the example a person is shown is one command's.
#[derive(Grouped)]
pub struct ErrorVaultAddressMissing;

impl Failure for ErrorVaultAddressMissing {
    fn name(&self) -> &'static str {
        "error_vault_address_missing"
    }

    fn reason(&self) -> String {
        t!("vault_bind.err_address_missing").trim().to_string()
    }
}

failure!(ErrorVaultAddressMissing);

#[renderer(buffer)]
pub fn render_error_vault_address_missing(error: ErrorVaultAddressMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("vault_bind.err_address_missing_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}

/// Error: the address a `rola vault bind` was given would not read as one.
///
/// It is shared with `rola bind`, which reads an address the same way; the field is visible so the
/// failure can be built where the address was read, and the words a person is shown stay this
/// command's.
#[derive(Grouped)]
pub struct ErrorVaultAddressInvalid {
    /// The address that would not read.
    pub(crate) address: String,
}

impl Failure for ErrorVaultAddressInvalid {
    fn name(&self) -> &'static str {
        "error_vault_address_invalid"
    }

    fn reason(&self) -> String {
        t!("vault_bind.err_address_invalid", address = self.address)
            .trim()
            .to_string()
    }
}

failure!(ErrorVaultAddressInvalid);

#[renderer(buffer)]
pub fn render_error_vault_address_invalid(error: ErrorVaultAddressInvalid, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("vault_bind.err_address_invalid_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}
