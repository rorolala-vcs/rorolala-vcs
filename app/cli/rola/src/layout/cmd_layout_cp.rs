//! The `rola layout cp` command: copy a Layout to a new name and work in the copy.

use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::{EntryPicker, Pickable},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResWorkspace, ResWorkspaceConfig};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutTrackNotBound, failed};

/// The flags `rola layout cp` takes.
#[derive(Pickable)]
struct CpFlags {
    /// The Vault upstream the copy tracks.
    #[arg(long)]
    track: Option<String>,
}

#[help(buffer)]
pub fn help_layout_cp(_: EntryLayoutCp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_cp.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutCp)]
pub fn desc_layout_cp() -> Description {
    t!("cmd_layout_cp.description").to_string().into()
}

/// Copies a Layout to a new name
///
/// What is copied is the Layout's own files. What it was tracking is not: a copy is a new place to
/// work, so it tracks only what `--track` names, or nothing when it names none. What is worked in
/// does not change: `rola layout force-switch` is what switches.
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
        .pick(&arg![CpFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, from, to) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutCp {
        from,
        to,
        track: flags.track,
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
}

#[chain]
pub fn handle_layout_cp(
    state: StateLayoutCp,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
    let StateLayoutCp { from, to, track } = state;

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
    if let Err(error) = layouts.copy(&from, &to) {
        return failed(&error);
    }
    if let Some(track) = &track
        && let Err(error) = layouts.set_track(&to, track)
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
