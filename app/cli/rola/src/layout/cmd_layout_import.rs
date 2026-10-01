//! The `rola layout import` command: make a Layout from a `.rolayout` file.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::PathBuf;

use librorolala::layout::{LayoutFile, MutableData};
use librorolala::storage::{Key, StorageBackend as _};
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::{positional, strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_LAYOUT, EC_HELP};
use crate::failure::failure;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutFailed, failed};

/// The flags `rola layout import` takes.
#[derive(Pickable)]
struct ImportFlags {
    /// The name to give the Layout; the file's own name when none is given.
    #[arg(long)]
    name: Option<String>,
    /// Import without checking that the content the file names is here.
    #[arg(long)]
    no_check: Flag,
}

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
/// The input is a file or directory the run can reach, so the filesystem answers it; `--name` is the
/// caller's own words, and `--no-check` is a flag.
#[completion(EntryLayoutImport)]
pub fn complete_layout_import(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                "--name": t!("cmd_layout_import.complete.name"),
                "--no-check": t!("cmd_layout_import.complete.no_check"),
            },
        );
    }

    if ctx.previous_word == "--name" {
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
/// tracked something, is tracked again here. What is worked in does not change: a new Layout is
/// made beside the one being worked in, and `rola layout force-switch` is what switches.
///
/// A file that names the keys a checkout needs is checked first: every one of them has to be in the
/// index or the store this run works in, and a file that names one that is not is refused rather
/// than imported half-checkable. `--no-check` imports it anyway.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when no name could be worked out, [`ErrorLayoutIncomplete`] when the
/// content the file names is not all here, and the name-already-taken or name-not-allowed failures
/// otherwise.
#[command(node = "layout.import", entry = EntryLayoutImport)]
pub fn layout_import(args: EntryLayoutImport) -> Next {
    let picked = args
        .pick(&arg![ImportFlags])
        .pick_or_route(&arg![PathBuf], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, input) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutImport {
        input,
        name: flags.name,
        check: matches!(flags.no_check, Flag::Inactive),
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
    /// Whether the content the file names is checked to be here.
    check: bool,
}

#[chain]
pub fn handle_layout_import(
    state: StateLayoutImport,
    workspace: &mut LazyRes<ResWorkspace>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let StateLayoutImport { input, name, check } = state;

    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorShouldInWorkspace.into();
    };

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

        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                return ErrorLayoutFailed {
                    cause: error.to_string(),
                }
                .into();
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
            Err(cause) => return ErrorLayoutFailed { cause }.into(),
        };

        let missing: Vec<Key> = file
            .requires()
            .unwrap_or_default()
            .iter()
            .filter(|key| !present.contains(*key))
            .copied()
            .collect();
        if !missing.is_empty() {
            return ErrorLayoutIncomplete { missing }.into();
        }
    }

    let layouts = workspace.layouts();
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

    if let Some(track) = file.track()
        && let Err(error) = layouts.set_track(&name, track)
    {
        return failed(&error);
    }

    ResultLayoutImport {
        name,
        entries: file.entries().len(),
    }
    .into()
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
