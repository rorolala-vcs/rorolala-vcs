//! The `rola layout un-track` command: make the Layout track no Vault.

use mingling::{
    Grouped, LazyRes, Suggest,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::failed;

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
/// The command names nothing: it lets go of whatever the one Layout tracks, so there is nothing to
/// offer.
#[completion(EntryLayoutUnTrack)]
pub fn complete_layout_un_track() -> Suggest {
    suggest!()
}

/// Makes the Layout track no Vault
///
/// What it tracked is let go of; the Layout itself is left as it was.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace, and the
/// Layout-not-readable failure otherwise.
#[command(node = "layout.un-track", entry = EntryLayoutUnTrack)]
pub fn layout_un_track() -> StateLayoutUnTrack {
    StateLayoutUnTrack
}

/// The state of letting go of what the Layout tracks.
#[derive(Grouped, Clone, Copy)]
pub struct StateLayoutUnTrack;

#[chain]
pub fn handle_layout_un_track(
    _state: StateLayoutUnTrack,
    workspace: &mut LazyRes<ResWorkspace>,
) -> Next {
    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorShouldInWorkspace.into();
    };

    if let Err(error) = workspace.un_track_layout() {
        return failed(&error);
    }

    ResultLayoutUnTrack.into()
}

/// Result: the Layout was made to track no Vault.
#[derive(Grouped)]
pub struct ResultLayoutUnTrack;

#[renderer(buffer)]
pub fn render_result_layout_un_track(_: ResultLayoutUnTrack) {
    r_println!("{}", t!("cmd_layout_un_track.result_untracked").trim());
}
