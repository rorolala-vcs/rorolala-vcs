//! The `rola layout rm` command: take a Layout away, or empty the one a Vault keeps.

// `ResConfirm` is a value small enough to copy, but it is the resource the injection hands in and
// not something a command owns: taking a copy would read a state of its own, so it is taken by
// reference, which is what the injection gives it.
#![allow(clippy::trivially_copy_pass_by_ref)]

use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    confirm::YesConfirm,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::{ResConfirm, ResExitCode},
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rust_i18n::t;

use crate::Next;
use crate::complete::{offer, positional, typing_flag, workspace_layout_names};
use crate::exit_codes::{EC_ERR_LAYOUT, EC_HELP};
use crate::failure::failure;
use crate::layout::{ErrorLayoutArgument, Place, failed, place};

#[help(buffer)]
pub fn help_layout_rm(_: EntryLayoutRm, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_rm.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutRm)]
pub fn desc_layout_rm() -> Description {
    t!("cmd_layout_rm.description").to_string().into()
}

/// Completes what `rola layout rm` can be given next.
///
/// What is named is a Layout the Workspace holds; a Vault holds one and has nothing to name, so a
/// run there has nothing to offer either.
#[completion(EntryLayoutRm)]
pub fn complete_layout_rm(ctx: ShellContext, workspace: &mut LazyRes<ResWorkspace>) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "rm") != 0 {
        return suggest!();
    }

    offer(&ctx, workspace_layout_names(workspace.get_ref()))
}

/// Takes a Layout away, or empties the one a Vault keeps
///
/// In a Workspace the named Layout is taken away, and everything it held with it. The Layout that
/// is being worked in is not one of them: choosing another first is what says the work has moved.
/// In a Vault there is one Layout and nothing to name it by but `truth`, so `rola layout rm` there
/// empties it — every path it named is dropped and the Layout is left to work in again.
///
/// Either is asked about before anything is done.
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when a Workspace run named no Layout or a Vault run named one
/// that is not `truth`, the run-not-in-a-place failure when it is in neither, and the
/// Layout-not-there failure otherwise.
#[command(node = "layout.rm", entry = EntryLayoutRm)]
pub fn layout_rm(args: EntryLayoutRm) -> Next {
    // Picking cannot fail: a positional that is absent is `None`, and what a place makes of a name
    // that is missing is its own to say.
    let name: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateLayoutRm { name }.into()
}

/// The state of taking a Layout away.
#[derive(Grouped)]
pub struct StateLayoutRm {
    /// The Layout to take away, when one was named.
    name: Option<String>,
}

#[chain]
pub fn handle_layout_rm(
    state: StateLayoutRm,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    confirm: &ResConfirm,
) -> Next {
    let StateLayoutRm { name } = state;

    let place = match place(workspace.get_ref(), vault.get_ref()) {
        Ok(place) => place,
        Err(next) => return next,
    };

    match place {
        Place::Workspace(layouts) => {
            let Some(name) = name else {
                return ErrorLayoutArgument.into();
            };

            match layouts.current() {
                Ok(current) if current.as_deref() == Some(name.as_str()) => {
                    return ErrorLayoutCurrent.into();
                }
                Ok(_) => {}
                Err(error) => return failed(&error),
            }

            if !confirm.ask::<YesConfirm>(&t!("cmd_layout_rm.confirm_remove", name = name)) {
                return ResultLayoutRmDeclined.into();
            }

            if let Err(error) = layouts.remove(&name) {
                return failed(&error);
            }

            ResultLayoutRm { name: Some(name) }.into()
        }
        Place::Vault(layout) => {
            if let Some(name) = &name
                && name != VAULT_LAYOUT_NAME
            {
                return ErrorLayoutArgument.into();
            }

            if !confirm.ask::<YesConfirm>(&t!("cmd_layout_rm.confirm_clean")) {
                return ResultLayoutRmDeclined.into();
            }

            if let Err(error) = layout.clear() {
                return failed(&error);
            }

            ResultLayoutRm { name: None }.into()
        }
    }
}

/// Error: the Layout a Workspace run named is the one being worked in.
#[derive(Grouped)]
pub struct ErrorLayoutCurrent;

impl Failure for ErrorLayoutCurrent {
    fn name(&self) -> &'static str {
        "error_layout_current"
    }

    fn reason(&self) -> String {
        t!("cmd_layout_rm.err_current").trim().to_string()
    }
}

failure!(ErrorLayoutCurrent);

#[renderer(buffer)]
pub fn render_error_layout_current(_: ErrorLayoutCurrent, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout_rm.err_current").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout_rm.err_current_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT;
}

/// Result: a Layout was taken away, or the one a Vault keeps was emptied.
#[derive(Grouped)]
pub struct ResultLayoutRm {
    /// The Layout that was taken away, or nothing when a Vault's was emptied.
    name: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_layout_rm(result: ResultLayoutRm) {
    let said = result.name.map_or_else(
        || t!("cmd_layout_rm.result_cleaned"),
        |name| t!("cmd_layout_rm.result_removed", name = name),
    );

    r_println!("{}", said.trim());
}

/// Result: the run was asked and was not confirmed.
#[derive(Grouped)]
pub struct ResultLayoutRmDeclined;

#[renderer(buffer)]
pub fn render_result_layout_rm_declined(_: ResultLayoutRmDeclined) {
    r_println!("{}", t!("cmd_layout_rm.result_declined").trim());
}
