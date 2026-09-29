//! The `rola layout ls` command: the Layouts a run works in.
//!
//! A Workspace holds many, each with a name, and works in one of them at a time; a Vault holds one,
//! which is `truth` because it has no name of its own to be read from disk. What is listed is each
//! Layout by its name, whether it is the one checked out, and the Vault upstream it tracks.

use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{buffer, chain, command, help, metadata, r_eprintln, r_print, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{err_line, trd};
use rorolala_utils_constants::{LAYOUT_TRACK_FILE, VAULT_LAYOUT_NAME};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::EC_ERR_FORMAT;
use crate::format::ResFormat;
use crate::layout::{ErrorLayoutFailed, Place, place};

/// How a listing of the Layouts is drawn when no template is named.
///
/// One Layout a line: its name, whether it is the one checked out, and the Vault it tracks when it
/// tracks one. The three are the same names `--json` writes, so a template and a program read the
/// same fields.
pub const DEFAULT_FORMAT: &str = "{{ layouts.name }}{% if layouts.is_current %}  (checked out){% endif %}{% if layouts.tracking %}  -> {{ layouts.tracking }}{% endif %}";

#[help(buffer)]
pub fn help_layout_ls(_: EntryLayoutLs, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_ls.help")).trim());
    ec.exit_code = crate::exit_codes::EC_HELP;
}

#[metadata(EntryLayoutLs)]
pub fn desc_layout_ls() -> Description {
    t!("cmd_layout_ls.description").to_string().into()
}

/// Lists the Layouts a run works in
///
/// A Workspace's Layouts are listed, or a Vault's one. Each carries its name, whether it is the one
/// checked out (`is_current`), and the Vault upstream it tracks, which is empty when it tracks none.
#[command(node = "layout.ls")]
pub fn layout_ls(format: &mut ResFormat) -> StateLayoutLs {
    format.default_template(DEFAULT_FORMAT);
    StateLayoutLs
}

/// The state a listing of the Layouts starts in.
///
/// A listing names nothing: there is one set to read, and it is the run's own.
#[derive(Grouped, Clone, Copy)]
pub struct StateLayoutLs;

#[chain]
pub fn handle_layout_ls(
    _state: StateLayoutLs,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    format: &mut ResFormat,
) -> Next {
    let place = match place(workspace.get_ref(), vault.get_ref()) {
        Ok(place) => place,
        Err(next) => return next,
    };

    let layouts = match place {
        Place::Workspace(set) => {
            let current = match set.current() {
                Ok(current) => current,
                Err(error) => return failed(&error),
            };
            let names = match set.names() {
                Ok(names) => names,
                Err(error) => return failed(&error),
            };

            names
                .into_iter()
                .map(|name| {
                    let tracking = set.track(&name).ok().flatten().unwrap_or_default();
                    LayoutItem {
                        is_current: current.as_deref() == Some(name.as_str()),
                        tracking,
                        name,
                    }
                })
                .collect()
        }
        Place::Vault(layout) => vec![LayoutItem {
            name: VAULT_LAYOUT_NAME.to_owned(),
            is_current: true,
            tracking: track_of(&layout),
        }],
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
    /// The name it is known by.
    name: String,
    /// Whether it is the one the run has checked out.
    is_current: bool,
    /// The Vault upstream it tracks, empty when it tracks none.
    tracking: String,
}

/// Result: the run's Layouts were listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayouts {
    /// Each Layout, in name order.
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

            if layout.is_current {
                r_println!(
                    "{}",
                    t!("cmd_layout_ls.result_current", layout = shown).trim()
                );
            } else {
                r_println!("{shown}");
            }
        }
    }
}
