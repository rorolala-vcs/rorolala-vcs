//! The `rola layout force-switch` command: work in another Layout.

use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{offer, positional, typing_flag, workspace_layout_names};
use crate::exit_codes::EC_HELP;
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace, failed,
};

#[help(buffer)]
pub fn help_layout_force_switch(_: EntryLayoutForceSwitch, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_force_switch.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutForceSwitch)]
pub fn desc_layout_force_switch() -> Description {
    t!("cmd_layout_force_switch.description").to_string().into()
}

/// Completes what `rola layout force-switch` can be given next.
///
/// What is named is a Layout the Workspace holds, so those are what is offered. A Vault's fetched
/// copy is not: the command works in a Layout, and a fetched copy is read rather than worked in.
#[completion(EntryLayoutForceSwitch)]
pub fn complete_layout_force_switch(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "force-switch") != 0 {
        return suggest!();
    }

    offer(&ctx, workspace_layout_names(workspace.get_ref()))
}

/// Works in another Layout
///
/// What is changed is which Layout the Workspace works in — the name written down, and nothing
/// else. No file is touched: the tree is left exactly as it is, whether or not it is what the
/// Layout it is switched to names. That is why it is forcible: switching is a thing about the
/// Workspace, not about the work, and whatever the tree holds is left for `rola layout tree-diff`
/// to say.
///
/// A Vault keeps one Layout and has nothing to switch to, so this is a Workspace's alone.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when no name was given, and the Layout-not-there failure when there is
/// none by that name.
#[command(node = "layout.force-switch", entry = EntryLayoutForceSwitch)]
pub fn layout_force_switch(args: EntryLayoutForceSwitch) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let name = match picked {
        Ok(name) => name,
        Err(next) => return next,
    };

    StateLayoutForceSwitch { name }.into()
}

/// The state of switching which Layout is worked in.
#[derive(Grouped)]
pub struct StateLayoutForceSwitch {
    /// The Layout to work in.
    name: String,
}

#[chain]
pub fn handle_layout_force_switch(
    state: StateLayoutForceSwitch,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Next {
    let StateLayoutForceSwitch { name } = state;

    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorShouldInWorkspace.into();
    };

    if let Err(error) = workspace.layouts().set_current(&name) {
        return failed(&error);
    }

    ResultLayoutForceSwitch { name }.into()
}

/// Result: the Layout worked in was changed.
#[derive(Grouped)]
pub struct ResultLayoutForceSwitch {
    /// The Layout now worked in.
    name: String,
}

#[renderer(buffer)]
pub fn render_result_layout_force_switch(result: ResultLayoutForceSwitch) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_force_switch.result_switched",
            name = result.name
        )
        .trim()
    );
}
