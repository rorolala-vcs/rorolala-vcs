//! The `rola layout shot` command: write a Layout down as a `.rolayout` file.

use std::path::PathBuf;

use librorolala::layout::LayoutFile;
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rorolala_utils_constants::LAYOUT_TRACK_FILE;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutFailed, ErrorLayoutMissing, Place, failed, place,
};

#[help(buffer)]
pub fn help_layout_shot(_: EntryLayoutShot, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_shot.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutShot)]
pub fn desc_layout_shot() -> Description {
    t!("cmd_layout_shot.description").to_string().into()
}

/// Writes a Layout down as a `.rolayout` file
///
/// What is written is the Layout as it is now: each path, the file it names upstream, and the
/// version it is at. Written beside the index and the store the run works in, it also names every
/// key a checkout of it needs — the versions and variants from the index, and the content and its
/// chunks from the store — so a Layout that cannot be checked out whole is not written at all.
/// What the Layout tracks goes in with it, so importing it elsewhere names the same Vault.
///
/// The Layout is the one being worked in, or the one named.
///
/// # Errors
///
/// Renders the run-not-in-a-place failure when it is in neither a Workspace nor a Vault,
/// [`ErrorLayoutArgument`] when there is no Layout being worked in and none was named,
/// [`ErrorLayoutMissing`] when the one named is not there, and [`ErrorLayoutFailed`] when the
/// Layout cannot be packed or the file cannot be written.
#[command(node = "layout.shot", entry = EntryLayoutShot)]
pub fn layout_shot(args: EntryLayoutShot) -> Next {
    let picked = args
        .pick_or_route(&arg![PathBuf], || ErrorLayoutArgument.into())
        .pick(&arg![Option<String>])
        .to_result();
    let (output, name) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutShot { output, name }.into()
}

/// The state of writing a Layout down.
#[derive(Grouped)]
pub struct StateLayoutShot {
    /// The file the Layout is written down as.
    output: PathBuf,
    /// The Layout to write down, when one was named.
    name: Option<String>,
}

#[chain]
pub fn handle_layout_shot(
    state: StateLayoutShot,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let StateLayoutShot { output, name } = state;

    let place = match place(workspace.get_ref(), vault.get_ref()) {
        Ok(place) => place,
        Err(next) => return next,
    };

    let (layout, track) = match place {
        Place::Workspace(layouts) => {
            let name = match name {
                Some(name) => name,
                None => match layouts.current() {
                    Ok(Some(name)) => name,
                    Ok(None) => return ErrorLayoutArgument.into(),
                    Err(error) => return failed(&error),
                },
            };

            let layout = match layouts.get(&name) {
                Ok(Some(layout)) => layout,
                Ok(None) => return ErrorLayoutMissing.into(),
                Err(error) => return failed(&error),
            };
            let track = layouts.track(&name).ok().flatten();

            (layout, track)
        }
        Place::Vault(layout) => {
            let track = std::fs::read_to_string(layout.dir().join(LAYOUT_TRACK_FILE))
                .ok()
                .map(|track| track.trim().to_owned())
                .filter(|track| !track.is_empty());

            (layout, track)
        }
    };

    let (Some(store), Some(index)) = (storage.get_ref().as_ref(), index.get_ref().as_ref()) else {
        return ErrorLayoutFailed {
            cause: t!("cmd_layout_shot.err_no_store").trim().to_string(),
        }
        .into();
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorLayoutFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let mut file = match runtime.block_on(LayoutFile::pack(&layout, index, store)) {
        Ok(file) => file,
        Err(error) => {
            return ErrorLayoutFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };
    file.set_track(track);

    if let Err(error) = file.write(&output) {
        return ErrorLayoutFailed {
            cause: error.to_string(),
        }
        .into();
    }

    ResultLayoutShot { output }.into()
}

/// Result: a Layout was written down.
#[derive(Grouped)]
pub struct ResultLayoutShot {
    /// The file it was written to.
    output: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_layout_shot(result: ResultLayoutShot) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_shot.result_written",
            path = result.output.display().to_string()
        )
        .trim()
    );
}
