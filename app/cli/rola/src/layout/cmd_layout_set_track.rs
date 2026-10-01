//! The `rola layout set-track` command: name the Vault upstream a Layout tracks.

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
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace, ResWorkspaceConfig};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{offer, positional, typing_flag, vault_names, workspace_layout_names};
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
/// The Layout is one the Workspace holds and the Vault is one it has bound, so both words are read
/// from what the run already knows.
#[completion(EntryLayoutSetTrack)]
pub fn complete_layout_set_track(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    match positional(&ctx, "set-track") {
        0 => offer(&ctx, workspace_layout_names(workspace.get_ref())),
        1 => offer(&ctx, vault_names(remote.get_ref())),
        _ => suggest!(),
    }
}

/// Names the Vault upstream a Layout tracks
///
/// The name has to be one the Workspace has bound with `rola vault bind`, since a Layout tracks a
/// Vault by the name the Workspace knows it by rather than by its address.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when either name was left out, [`ErrorLayoutTrackNotBound`] when the
/// Vault named is not one the Workspace has bound, and the Layout-not-there failure otherwise.
#[command(node = "layout.set-track", entry = EntryLayoutSetTrack)]
pub fn layout_set_track(args: EntryLayoutSetTrack) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (name, track) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutSetTrack { name, track }.into()
}

/// The state of naming what a Layout tracks.
#[derive(Grouped)]
pub struct StateLayoutSetTrack {
    /// The Layout whose tracking is set.
    name: String,
    /// The Vault it is to track.
    track: String,
}

#[chain]
pub fn handle_layout_set_track(
    state: StateLayoutSetTrack,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
    let StateLayoutSetTrack { name, track } = state;

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

    if let Err(error) = workspace.layouts().set_track(&name, &track) {
        return failed(&error);
    }

    ResultLayoutSetTrack { name, track }.into()
}

/// Result: a Layout was made to track a Vault.
#[derive(Grouped)]
pub struct ResultLayoutSetTrack {
    /// The Layout whose tracking was set.
    name: String,
    /// The Vault it now tracks.
    track: String,
}

#[renderer(buffer)]
pub fn render_result_layout_set_track(result: ResultLayoutSetTrack) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_set_track.result_set",
            name = result.name,
            track = result.track
        )
        .trim()
    );
}
