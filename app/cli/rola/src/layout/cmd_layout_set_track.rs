//! The `rola layout set-track` command: name the Vault upstream the Layout tracks.

use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace, ResWorkspaceConfig};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{offer, typing_flag, vault_names};
use crate::exit_codes::EC_HELP;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutTrackNotBound, failed};

#[help(buffer)]
pub fn help_layout_set_track(_: EntryLayoutSetTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_set_track.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutSetTrack)]
pub fn desc_layout_set_track() -> Description {
    t!("cmd_layout_set_track.description").to_string().into()
}

/// Completes what `rola layout set-track` can be given next.
///
/// The one word names a Vault the Workspace has bound, so those are what is offered.
#[completion(EntryLayoutSetTrack)]
pub fn complete_layout_set_track(
    ctx: ShellContext,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    offer(&ctx, vault_names(remote.get_ref()))
}

/// Names the Vault upstream the Layout tracks
///
/// The name has to be one the Workspace has bound with `rola vault bind`, since a Layout tracks a
/// Vault by the name the Workspace knows it by rather than by its address.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when no name was given, [`ErrorLayoutTrackNotBound`] when the Vault named
/// is not one the Workspace has bound, and the Layout-not-readable failure otherwise.
#[command(node = "layout.set-track", entry = EntryLayoutSetTrack)]
pub fn layout_set_track(args: EntryLayoutSetTrack) -> Next {
    let picked = args
        .pick_or_route(arg![String], || ErrorLayoutArgument.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (track, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "layout set-track") {
        return next;
    }

    StateLayoutSetTrack { track }.into()
}

/// The state of naming what the Layout tracks.
#[derive(Grouped)]
pub struct StateLayoutSetTrack {
    /// The Vault it is to track.
    track: String,
}

#[chain]
pub fn handle_layout_set_track(
    state: StateLayoutSetTrack,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
    let StateLayoutSetTrack { track } = state;

    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorShouldInWorkspace.into();
    };

    if !config
        .get_ref()
        .config()
        .is_some_and(|config| config.vaults().contains(&track))
    {
        return ErrorLayoutTrackNotBound { track }.into();
    }

    if let Err(error) = workspace.set_layout_track(&track) {
        return failed(&error);
    }

    ResultLayoutSetTrack { track }.into()
}

/// Result: the Layout was made to track a Vault.
#[derive(Grouped)]
pub struct ResultLayoutSetTrack {
    /// The Vault it now tracks.
    track: String,
}

#[renderer(buffer)]
pub fn render_result_layout_set_track(result: ResultLayoutSetTrack) {
    r_println!(
        "{}",
        t!("cmd_layout_set_track.result_set", track = result.track).trim()
    );
}
