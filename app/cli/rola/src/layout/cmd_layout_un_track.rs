//! The `rola layout un-track` command: make a Layout track no Vault.

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
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, failed};

#[help(buffer)]
pub fn help_layout_un_track(_: EntryLayoutUnTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_un_track.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutUnTrack)]
pub fn desc_layout_un_track() -> Description {
    t!("cmd_layout_un_track.description").to_string().into()
}

/// Completes what `rola layout un-track` can be given next.
///
/// The one word names a Layout the Workspace holds, so those are what is offered.
#[completion(EntryLayoutUnTrack)]
pub fn complete_layout_un_track(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "un-track") != 0 {
        return suggest!();
    }

    offer(&ctx, workspace_layout_names(workspace.get_ref()))
}

/// Makes a Layout track no Vault
///
/// What it tracked is let go of; the Layout itself is left as it was.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when no name was given, and the Layout-not-there failure otherwise.
#[command(node = "layout.un-track", entry = EntryLayoutUnTrack)]
pub fn layout_un_track(args: EntryLayoutUnTrack) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let name = match picked {
        Ok(name) => name,
        Err(next) => return next,
    };

    StateLayoutUnTrack { name }.into()
}

/// The state of letting go of what a Layout tracks.
#[derive(Grouped)]
pub struct StateLayoutUnTrack {
    /// The Layout whose tracking is let go of.
    name: String,
}

#[chain]
pub fn handle_layout_un_track(
    state: StateLayoutUnTrack,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Next {
    let StateLayoutUnTrack { name } = state;

    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorShouldInWorkspace.into();
    };

    if let Err(error) = workspace.layouts().un_track(&name) {
        return failed(&error);
    }

    ResultLayoutUnTrack { name }.into()
}

/// Result: a Layout was made to track no Vault.
#[derive(Grouped)]
pub struct ResultLayoutUnTrack {
    /// The Layout whose tracking was let go of.
    name: String,
}

#[renderer(buffer)]
pub fn render_result_layout_un_track(result: ResultLayoutUnTrack) {
    r_println!(
        "{}",
        t!("cmd_layout_un_track.result_untracked", name = result.name).trim()
    );
}
