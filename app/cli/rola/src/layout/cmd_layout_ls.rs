//! The `rola layout ls` command: the Layout a run works in.
//!
//! A Workspace has one Layout and a Vault keeps one, so the listing is one line: the label the
//! Layout is listed under, that it is the one being worked in, and the Vault upstream it tracks.
//! The label is what a place addresses the Layout by rather than a thing there is a second of.

use mingling::{
    Grouped, LazyRes, StructuralData, Suggest,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, suggest,
    },
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{err_line, trd};
use rorolala_utils_constants::{DEFAULT_LAYOUT_NAME, LAYOUT_TRACK_FILE, VAULT_LAYOUT_NAME};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::EC_ERR_FORMAT;
use crate::format::ResFormat;
use crate::layout::{ErrorLayoutFailed, ErrorLayoutNoCurrent, ErrorNoLayouts};

/// How a listing of the Layout is drawn when no template is named.
///
/// One Layout a line: the label it goes by, that it is the one being worked in, and the Vault it
/// tracks when it tracks one. The three are the same names `--json` writes, so a template and a
/// program read the same fields.
pub const DEFAULT_FORMAT: &str =
    "{{ layouts.name }}{% if layouts.tracking %}  -> {{ layouts.tracking }}{% endif %}";

#[help(buffer)]
pub fn help_layout_ls(_: EntryLayoutLs, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_ls.help")).trim());
    ec.exit_code = crate::exit_codes::EC_HELP;
}

#[metadata(EntryLayoutLs)]
pub fn desc_layout_ls() -> Description {
    t!("cmd_layout_ls.description").to_string().into()
}

/// Completes what `rola layout ls` can be given next.
///
/// The listing names nothing, so there is nothing to offer.
#[completion(EntryLayoutLs)]
pub fn complete_layout_ls() -> Suggest {
    suggest!()
}

/// Lists the Layout a run works in
///
/// A Workspace's one Layout is listed, or a Vault's one. It carries the label it is listed under,
/// that it is the one being worked in (`is_current`), and the Vault upstream it tracks, which is
/// empty when it tracks none.
#[command(node = "layout.ls")]
pub fn layout_ls(format: &mut ResFormat) -> StateLayoutLs {
    format.default_template(DEFAULT_FORMAT);
    StateLayoutLs
}

/// The state a listing of the Layout starts in.
///
/// A listing names nothing: there is one Layout to read, and it is the run's own.
#[derive(Grouped, Clone, Copy)]
pub struct StateLayoutLs;

#[chain]
pub fn handle_layout_ls(
    _state: StateLayoutLs,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    format: &mut ResFormat,
) -> Next {
    let layouts = if let Some(workspace) = workspace.get_ref().as_ref() {
        // The Layout is read before what it tracks, so a Workspace made by an older version has the
        // Layout it worked in adopted before the reading says anything about it.
        match workspace.layout_if_there() {
            Ok(Some(_)) => {}
            Ok(None) => return ErrorLayoutNoCurrent.into(),
            Err(error) => return failed(&error),
        }

        let tracking = match workspace.layout_track() {
            Ok(tracking) => tracking.unwrap_or_default(),
            Err(error) => return failed(&error),
        };

        vec![LayoutItem {
            name: DEFAULT_LAYOUT_NAME.to_owned(),
            is_current: true,
            tracking,
        }]
    } else if let Some(vault) = vault.get_ref().as_ref() {
        let layout = match vault.layout() {
            Ok(layout) => layout,
            Err(error) => return failed(&error),
        };

        vec![LayoutItem {
            name: VAULT_LAYOUT_NAME.to_owned(),
            is_current: true,
            tracking: track_of(&layout),
        }]
    } else {
        return ErrorNoLayouts.into();
    };

    format.set(
        "layouts",
        layouts
            .iter()
            .map(|layout| serde_json::json!(layout))
            .collect(),
    );

    ResultLayouts { layouts }.into()
}

/// The Vault a Layout tracks, as read from the file beside it, or nothing when it tracks none.
fn track_of(layout: &librorolala::layout::Layout) -> String {
    std::fs::read_to_string(layout.dir().join(LAYOUT_TRACK_FILE))
        .map(|track| track.trim().to_owned())
        .unwrap_or_default()
}

/// What a Layout that could not be read is answered with.
fn failed(error: &librorolala::layout::LayoutError) -> Next {
    ErrorLayoutFailed {
        cause: error.to_string(),
    }
    .into()
}

/// One Layout, as `layout ls` shows it.
#[derive(Serialize)]
pub struct LayoutItem {
    /// The label it is listed under.
    name: String,
    /// Whether it is the one the run is working in.
    is_current: bool,
    /// The Vault upstream it tracks, empty when it tracks none.
    tracking: String,
}

/// Result: the run's Layout was listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayouts {
    /// The one Layout.
    layouts: Vec<LayoutItem>,
}

#[renderer(buffer)]
pub fn render_result_layouts(result: ResultLayouts, format: &ResFormat, ec: &mut ResExitCode) {
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
        for layout in &result.layouts {
            let shown = if layout.tracking.is_empty() {
                layout.name.clone()
            } else {
                format!("{}  {}", layout.name, layout.tracking)
            };

            r_println!("{shown}");
        }
    }
}
