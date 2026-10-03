//! The `rola layout import` command: make a Layout from a `.rolayout` file.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::PathBuf;

use librorolala::layout::{LayoutFile, MutableData};
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::VCSIndex;
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
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResWorkspace, ResWorkspaceConfig};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::{filling_flag, positional, strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_LAYOUT, EC_HELP};
use crate::failure::failure;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutFailed, ErrorLayoutTrackNotBound, failed};

/// The name to give the Layout; the file's own name when none is given.
const ARG_NAME: PickerArg<'static, Option<String>> = arg![name: Option<String>];

/// The Vault upstream the imported Layout tracks; the file's own when none is given.
const ARG_TRACK: PickerArg<'static, Option<String>> = arg![track: Option<String>];

/// Import without checking that the content the file names is here.
const ARG_NO_CHECK: PickerArg<'static, Flag> = arg![no_check: Flag];

/// Leave the Layout being worked in where it is, even when the Workspace had none.
const ARG_NO_SET_LAYOUT: PickerArg<'static, Flag> = arg![no_set_layout: Flag];

#[help(buffer)]
pub fn help_layout_import(_: EntryLayoutImport, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_import.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutImport)]
pub fn desc_layout_import() -> Description {
    t!("cmd_layout_import.description").to_string().into()
}

/// Completes what `rola layout import` can be given next.
///
/// The input is a file or directory the run can reach, so the filesystem answers it; `--name` and
/// `--track` are the caller's own words, and `--no-check` and `--no-set-layout` are flags.
#[completion(EntryLayoutImport)]
pub fn complete_layout_import(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_NAME: t!("cmd_layout_import.complete.name"),
                ARG_TRACK: t!("cmd_layout_import.complete.track"),
                ARG_NO_CHECK: t!("cmd_layout_import.complete.no_check"),
                ARG_NO_SET_LAYOUT: t!("cmd_layout_import.complete.no_set_layout"),
            },
        );
    }

    if filling_flag(&ctx, &ARG_NAME) {
        return suggest!();
    }

    if positional(&ctx, "import") == 0 {
        Suggest::file_comp()
    } else {
        suggest!()
    }
}

/// Makes a Layout from a `.rolayout` file
///
/// What the file names is written into a new Layout. The name is the one given, or the file's own
/// name without its extension. What the file was tracking, when it was written by a Layout that
/// tracked something, is tracked again here, unless `--track` names a Vault to track instead.
/// Making the first Layout in a Workspace is also choosing what is worked in, since there is then
/// no other; `--no-set-layout` leaves the Workspace working in nothing. Otherwise what is worked in
/// does not change: a new Layout is made beside the one being worked in, and
/// `rola layout force-switch` is what switches.
///
/// A file that names the keys a checkout needs is checked first: every one of them has to be in the
/// index or the store this run works in, and a file that names one that is not is refused rather
/// than imported half-checkable. `--no-check` imports it anyway.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when no name could be worked out, [`ErrorLayoutTrackNotBound`] when
/// `--track` names a Vault the Workspace has not bound, [`ErrorLayoutIncomplete`] when the content
/// the file names is not all here, and the name-already-taken or name-not-allowed failures
/// otherwise.
#[command(node = "layout.import", entry = EntryLayoutImport)]
pub fn layout_import(args: EntryLayoutImport) -> Next {
    let picked = args
        .pick(&ARG_NAME)
        .pick(&ARG_TRACK)
        .pick(&ARG_NO_CHECK)
        .pick(&ARG_NO_SET_LAYOUT)
        .pick_or_route(&arg![PathBuf], || ErrorLayoutArgument.into())
        .to_result();
    let (name, track, no_check, no_set_layout, input) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutImport {
        input,
        name,
        track,
        check: matches!(no_check, Flag::Inactive),
        set_current: matches!(no_set_layout, Flag::Inactive),
    }
    .into()
}

/// The state of importing a Layout.
#[derive(Grouped)]
pub struct StateLayoutImport {
    /// The file the Layout is read from.
    input: PathBuf,
    /// The name to give it, when one was named.
    name: Option<String>,
    /// The Vault it is to track, when one was named.
    track: Option<String>,
    /// Whether the content the file names is checked to be here.
    check: bool,
    /// Whether making it may also choose it as the one worked in.
    set_current: bool,
}

#[chain]
pub fn handle_layout_import(
    state: StateLayoutImport,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let StateLayoutImport {
        input,
        name,
        track,
        check,
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

    let file = match LayoutFile::read(&input) {
        Ok(file) => file,
        Err(error) => {
            return ErrorLayoutFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let Some(name) = name.or_else(|| input.file_stem().and_then(OsStr::to_str).map(str::to_owned))
    else {
        return ErrorLayoutArgument.into();
    };

    if check && file.is_packed() {
        let (Some(store), Some(index)) = (storage.get_ref().as_ref(), index.get_ref().as_ref())
        else {
            return ErrorLayoutFailed {
                cause: t!("cmd_layout_import.err_no_store").trim().to_string(),
            }
            .into();
        };

        if let Err(next) = content_is_here(&file, store, index) {
            return next;
        }
    }

    let layouts = workspace.layouts();

    // Whether there is a Layout to work in is read before the new one is made: a Workspace that had
    // none is one the new Layout is the first of, and the first is what gets worked in.
    let was_empty = match layouts.names() {
        Ok(names) => names.is_empty(),
        Err(error) => return failed(&error),
    };

    let layout = match layouts.create(&name) {
        Ok(layout) => layout,
        Err(error) => return failed(&error),
    };

    for entry in file.entries() {
        let id = entry.id();
        if let Err(error) =
            layout.create_entry(id, MutableData::new(None, entry.version(), String::new()))
        {
            return failed(&error);
        }
        if let Err(error) = layout.create_path(entry.path(), id) {
            return failed(&error);
        }
    }

    // What is tracked is `--track` when one was named, and what the file says otherwise: a file
    // carries the Vault its Layout was tracking, so an import does not have to be told again, and
    // naming one is how a run says it is to be somewhere else.
    let track = track.or_else(|| file.track().map(str::to_owned));
    if let Some(track) = &track
        && let Err(error) = layouts.set_track(&name, track)
    {
        return failed(&error);
    }
    if was_empty
        && set_current
        && let Err(error) = layouts.set_current(&name)
    {
        return failed(&error);
    }

    ResultLayoutImport {
        name,
        entries: file.entries().len(),
    }
    .into()
}

/// Refuses a packed file whose keys are not all reachable here, answering with the failure to
/// render when one is not.
///
/// What a packed file names is what a checkout of it needs: the versions and variants from the
/// index, and the content and its chunks from the store. All of them have to be reachable before
/// the import starts, so that nothing is made checkable by halves.
fn content_is_here(
    file: &LayoutFile,
    store: &RorolalaStorage,
    index: &VCSIndex,
) -> Result<(), Next> {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return Err(ErrorLayoutFailed {
                cause: error.to_string(),
            }
            .into());
        }
    };

    let present = match runtime.block_on(async {
        let mut present: BTreeSet<Key> = BTreeSet::new();
        match index.object_keys().await {
            Ok(keys) => present.extend(keys),
            Err(error) => return Err(error.reason()),
        }
        match store.list_exist_keys().await {
            Ok(keys) => present.extend(keys),
            Err(error) => return Err(error.to_string()),
        }

        Ok(present)
    }) {
        Ok(present) => present,
        Err(cause) => return Err(ErrorLayoutFailed { cause }.into()),
    };

    let missing: Vec<Key> = file
        .requires()
        .unwrap_or_default()
        .iter()
        .filter(|key| !present.contains(*key))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(ErrorLayoutIncomplete { missing }.into());
    }

    Ok(())
}

/// Error: the content a `.rolayout` file names is not all here.
#[derive(Grouped)]
pub struct ErrorLayoutIncomplete {
    /// The keys the file needs that are not here.
    missing: Vec<Key>,
}

impl Failure for ErrorLayoutIncomplete {
    fn name(&self) -> &'static str {
        "error_layout_incomplete"
    }

    fn reason(&self) -> String {
        let keys: Vec<String> = self.missing.iter().map(Key::to_string).collect();

        t!(
            "cmd_layout_import.err_incomplete",
            count = keys.len(),
            keys = keys.join(", ")
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorLayoutIncomplete);

#[renderer(buffer)]
pub fn render_error_layout_incomplete(error: ErrorLayoutIncomplete, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout_import.err_incomplete_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT;
}

/// Result: a Layout was made from a file.
#[derive(Grouped)]
pub struct ResultLayoutImport {
    /// The name it was given.
    name: String,
    /// How many paths it names.
    entries: usize,
}

#[renderer(buffer)]
pub fn render_result_layout_import(result: ResultLayoutImport) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_import.result_imported",
            name = result.name,
            count = result.entries
        )
        .trim()
    );
}
