//! The `rola layout entries` command: what a Layout's entries hold.

use librorolala::storage::Key;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_print, r_println, renderer,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{err_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::format::ResFormat;
use crate::layout::{LayoutOnlyFlags, chosen};

/// How a listing of the entries is drawn when no template is named.
///
/// One entry a line: the path it is at, the `Uuid` it is known by, and the version it is at. What
/// it is held by and what it says are in the data a template or `--json` reads.
pub const DEFAULT_FORMAT: &str = "{{ entries.path }}  {{ entries.uuid }}  {{ entries.version }}";

#[help(buffer)]
pub fn help_layout_entries(_: EntryLayoutEntries, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_entries.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutEntries)]
pub fn desc_layout_entries() -> Description {
    t!("cmd_layout_entries.description").to_string().into()
}

/// Lists what a Layout's entries hold
///
/// Every entry is listed — one that is at no path is listed with an empty one — with the `Uuid` it
/// is known by, the version it is at, the account that holds it, and what it says about itself.
///
/// # Errors
///
/// Renders the run-not-in-a-place failure when the run is in neither a Workspace nor a Vault, and
/// the argument or not-there failures when the Layout named could not be read.
#[command(node = "layout.entries", entry = EntryLayoutEntries)]
pub fn layout_entries(args: EntryLayoutEntries, format: &mut ResFormat) -> StateLayoutEntries {
    // Picking flags cannot fail: a flag that is absent is `None`, not an error.
    let flags = args.pick(&arg![LayoutOnlyFlags]).unwrap();

    format.default_template(DEFAULT_FORMAT);
    StateLayoutEntries {
        layout: flags.layout,
    }
}

/// The state a listing of the entries starts in.
#[derive(Grouped)]
pub struct StateLayoutEntries {
    /// The Layout to read, when one was named.
    layout: Option<String>,
}

#[chain]
pub fn handle_layout_entries(
    state: StateLayoutEntries,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    format: &mut ResFormat,
) -> Next {
    let StateLayoutEntries { layout } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), layout.as_deref()) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let mut entries: Vec<EntryItem> = layout
        .entries()
        .into_iter()
        .map(|(id, data)| EntryItem {
            path: layout
                .path_of(id)
                .map_or_else(String::new, |path| path.as_str().to_owned()),
            uuid: id.to_string(),
            version: Key::new(data.version()).hex(),
            owner: data.owner().unwrap_or_default().to_owned(),
            description: data.description().to_owned(),
        })
        .collect();
    entries.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.uuid.cmp(&right.uuid))
    });

    format.set(
        "entries",
        entries
            .iter()
            .map(|entry| serde_json::json!(entry))
            .collect(),
    );

    ResultLayoutEntries { entries }.into()
}

/// One entry, as `layout entries` shows it.
#[derive(Serialize)]
pub struct EntryItem {
    /// The path it is at, empty when it is at none.
    path: String,
    /// The `Uuid` it is known by.
    uuid: String,
    /// The version it is at, as hex.
    version: String,
    /// The account that holds it, empty when none does.
    owner: String,
    /// What it says about itself.
    description: String,
}

/// Result: a Layout's entries were listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutEntries {
    /// Each entry, in path order.
    entries: Vec<EntryItem>,
}

#[renderer(buffer)]
pub fn render_result_layout_entries(
    result: ResultLayoutEntries,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!(
                    "{}",
                    err_line!(t!("format.err_format", reason = error).trim())
                );
                ec.exit_code = EC_ERR_FORMAT;
            }
        }
    } else {
        for entry in &result.entries {
            r_println!("{}  {}  {}", entry.path, entry.uuid, entry.version);
        }
    }
}
