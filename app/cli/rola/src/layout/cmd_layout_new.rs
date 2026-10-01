//! The `rola layout new` command: make an empty Layout and work in it.

use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace, ResWorkspaceConfig};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{offer, strip_written, typing_flag, vault_names};
use crate::exit_codes::EC_HELP;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutTrackNotBound, failed};

/// The flags `rola layout new` takes.
#[derive(Pickable)]
struct NewFlags {
    /// The Vault upstream the new Layout tracks.
    #[arg(long)]
    track: Option<String>,
}

#[help(buffer)]
pub fn help_layout_new(_: EntryLayoutNew, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_new.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutNew)]
pub fn desc_layout_new() -> Description {
    t!("cmd_layout_new.description").to_string().into()
}

/// Completes what `rola layout new` can be given next.
///
/// The name is new, so there is nothing to offer for it; `--track` names a Vault the Workspace has
/// bound.
#[completion(EntryLayoutNew)]
pub fn complete_layout_new(
    ctx: ShellContext,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if ctx.previous_word == "--track" {
        return offer(&ctx, vault_names(remote.get_ref()));
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                "--track": t!("cmd_layout_new.complete.track"),
            },
        );
    }

    // The name is the caller's own, so nothing here can guess at it.
    suggest!()
}

/// Makes an empty Layout
///
/// The name has to be one a Layout may be given — words of letters and digits joined by `-` — since
/// it is the name of a directory. `--track` names a Vault the Workspace has bound, which the Layout
/// is to track; naming none leaves it tracking nothing. What is worked in does not change: a new
/// Layout is made beside the one being worked in, and `rola layout force-switch` is what switches.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when no name was given, [`ErrorLayoutTrackNotBound`] when `--track` names
/// a Vault the Workspace has not bound, and the name-already-taken or name-not-allowed failures
/// otherwise.
#[command(node = "layout.new", entry = EntryLayoutNew)]
pub fn layout_new(args: EntryLayoutNew) -> Next {
    let picked = args
        .pick(&arg![NewFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, name) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutNew {
        name,
        track: flags.track,
    }
    .into()
}

/// The state of making a Layout.
#[derive(Grouped)]
pub struct StateLayoutNew {
    /// The name the Layout is to be given.
    name: String,
    /// The Vault it is to track, when one was named.
    track: Option<String>,
}

#[chain]
pub fn handle_layout_new(
    state: StateLayoutNew,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
    let StateLayoutNew { name, track } = state;

    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorShouldInWorkspace.into();
    };

    if let Some(track) = &track
        && !config
            .get_ref()
            .config()
            .is_some_and(|config| config.vaults().contains(track))
    {
        return ErrorLayoutTrackNotBound {
            track: track.clone(),
        }
        .into();
    }

    let layouts = workspace.layouts();
    if let Err(error) = layouts.create(&name) {
        return failed(&error);
    }
    if let Some(track) = &track
        && let Err(error) = layouts.set_track(&name, track)
    {
        return failed(&error);
    }

    ResultLayoutNew { name }.into()
}

/// Result: a Layout was made.
#[derive(Grouped)]
pub struct ResultLayoutNew {
    /// The name it was given.
    name: String,
}

#[renderer(buffer)]
pub fn render_result_layout_new(result: ResultLayoutNew) {
    r_println!(
        "{}",
        t!("cmd_layout_new.result_created", name = result.name).trim()
    );
}
