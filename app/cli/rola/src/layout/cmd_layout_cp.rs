//! The `rola layout cp` command: copy a Layout to a new name and work in the copy.

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
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace, ResWorkspaceConfig};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{
    filling_flag, offer, positional, strip_written, typing_flag, vault_names,
    workspace_layout_names,
};
use crate::exit_codes::EC_HELP;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutTrackNotBound, failed};

/// The Vault upstream the copy tracks.
const ARG_TRACK: PickerArg<'static, Option<String>> = arg![track: Option<String>];

/// Leave the Layout being worked in where it is, even when the Workspace had none.
const ARG_NO_SET_LAYOUT: PickerArg<'static, Flag> = arg![no_set_layout: Flag];

#[help(buffer)]
pub fn help_layout_cp(_: EntryLayoutCp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_cp.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutCp)]
pub fn desc_layout_cp() -> Description {
    t!("cmd_layout_cp.description").to_string().into()
}

/// Completes what `rola layout cp` can be given next.
///
/// The Layout copied is one that is here, so those are offered; the name the copy is given is new,
/// so there is nothing to offer for it. `--track` names a Vault the Workspace has bound.
#[completion(EntryLayoutCp)]
pub fn complete_layout_cp(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if filling_flag(&ctx, &ARG_TRACK) {
        return offer(&ctx, vault_names(remote.get_ref()));
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_TRACK: t!("cmd_layout_cp.complete.track"),
                ARG_NO_SET_LAYOUT: t!("cmd_layout_cp.complete.no_set_layout"),
            },
        );
    }

    if positional(&ctx, "cp") == 0 {
        offer(&ctx, workspace_layout_names(workspace.get_ref()))
    } else {
        suggest!()
    }
}

/// Copies a Layout to a new name
///
/// What is copied is the Layout's own files. What it was tracking is not: a copy is a new place to
/// work, so it tracks only what `--track` names, or nothing when it names none. Making the first
/// Layout in a Workspace is also choosing what is worked in, since there is then no other;
/// `--no-set-layout` leaves the Workspace working in nothing. Otherwise what is worked in does not
/// change: `rola layout force-switch` is what switches.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when either name was left out, [`ErrorLayoutTrackNotBound`] when
/// `--track` names a Vault the Workspace has not bound, and the name-not-there, name-already-taken
/// or name-not-allowed failures otherwise.
#[command(node = "layout.cp", entry = EntryLayoutCp)]
pub fn layout_cp(args: EntryLayoutCp) -> Next {
    let picked = args
        .pick(&ARG_TRACK)
        .pick(&ARG_NO_SET_LAYOUT)
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (track, no_set_layout, from, to) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutCp {
        from,
        to,
        track,
        set_current: matches!(no_set_layout, Flag::Inactive),
    }
    .into()
}

/// The state of copying a Layout.
#[derive(Grouped)]
pub struct StateLayoutCp {
    /// The Layout that is copied.
    from: String,
    /// The name the copy is given.
    to: String,
    /// The Vault the copy is to track, when one was named.
    track: Option<String>,
    /// Whether making it may also choose it as the one worked in.
    set_current: bool,
}

#[chain]
pub fn handle_layout_cp(
    state: StateLayoutCp,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
    let StateLayoutCp {
        from,
        to,
        track,
        set_current,
    } = state;

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

    // Whether there is a Layout to work in is read before the copy is made: a Workspace that had
    // none is one the copy is the first Layout of, and the first is what gets worked in.
    let was_empty = match layouts.names() {
        Ok(names) => names.is_empty(),
        Err(error) => return failed(&error),
    };

    if let Err(error) = layouts.copy(&from, &to) {
        return failed(&error);
    }
    if let Some(track) = &track
        && let Err(error) = layouts.set_track(&to, track)
    {
        return failed(&error);
    }
    if was_empty
        && set_current
        && let Err(error) = layouts.set_current(&to)
    {
        return failed(&error);
    }

    ResultLayoutCp { name: to }.into()
}

/// Result: a Layout was copied.
#[derive(Grouped)]
pub struct ResultLayoutCp {
    /// The name the copy was given.
    name: String,
}

#[renderer(buffer)]
pub fn render_result_layout_cp(result: ResultLayoutCp) {
    r_println!(
        "{}",
        t!("cmd_layout_cp.result_copied", name = result.name).trim()
    );
}
