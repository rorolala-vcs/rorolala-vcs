//! The `rola status` command: how the working tree stands beside the Layout it works from.
//!
//! It is the reading behind the two [porcelain](crate) commands that act on the tree, said for a
//! person rather than for a program: what is lost or untagged needs `rola align`, what changed in
//! place needs `rola track`, and a move is one `rola align` confirms and `track` confirms by
//! itself. What it is drawn as is a few lines — what moved, what is gone, what appeared, what
//! changed — and the one thing to do about each.
//!
//! A clean tree says so and nothing else. A run that asked for `--json` is answered with the same
//! reading as data, since a reading is what it is either way; a template is not offered, as what a
//! person is shown here is not a list of one shape but a sentence about several.

use std::collections::BTreeSet;

use librorolala::tree_analyze::tree_diff;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{buffer, chain, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{help_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::layout::{ErrorLayoutFailed, ErrorLayoutShouldInWorkspace, chosen};

/// How alike two text files have to be to count as the same file moved, when nothing is said.
const DEFAULT_ALIKE: f32 = 0.6;

#[help(buffer)]
pub fn help_status(_: EntryStatus, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("status.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStatus)]
pub fn desc_status() -> Description {
    t!("status.description").to_string().into()
}

/// Shows how the working tree stands beside the Layout being worked in
///
/// What moved is shown first, then what the Layout names and the tree does not hold, then what the
/// tree holds and the Layout does not name; a line with none of the marks is content that changed
/// where it was. A `(*)` marks a line that is also a content change, which for a move is a move
/// whose file was edited as well.
///
/// What is lost is what `rola align` settles and what changed is what `rola track` records; a move
/// is confirmed by `rola align` or, by naming the file where it is now, by `rola track`.
///
/// # Errors
///
/// Renders the run-not-in-a-workspace failure when the run is not inside a Workspace, the argument
/// or not-there failures when there is no Layout to read, and [`ErrorLayoutFailed`] when the tree
/// or the Layout could not be read.
#[command(node = "status", entry = EntryStatus)]
pub fn status() -> StateStatus {
    StateStatus
}

/// The state a reading of the tree starts in: it names nothing.
#[derive(Grouped)]
pub struct StateStatus;

#[chain]
pub fn handle_status(
    _state: StateStatus,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Next {
    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let diff = {
        let Some(workspace) = workspace.get_ref().as_ref() else {
            return ErrorLayoutShouldInWorkspace.into();
        };

        match tree_diff(&layout, workspace, DEFAULT_ALIKE) {
            Ok(diff) => diff,
            Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
        }
    };

    let moved: BTreeSet<&str> = diff
        .renamed
        .iter()
        .map(|rename| rename.to.as_str())
        .collect();

    ResultStatus {
        lost: diff
            .lost
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect(),
        untagged: diff
            .untagged
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect(),
        modified: diff
            .modified
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect(),
        renamed: diff
            .renamed
            .iter()
            .map(|rename| RenameItem {
                from: rename.from.as_str().to_owned(),
                to: rename.to.as_str().to_owned(),
                modified: moved.contains(rename.to.as_str()),
                strong: rename.strong,
            })
            .collect(),
    }
    .into()
}

/// One path that moved, as `status` shows it.
#[derive(Serialize)]
pub struct RenameItem {
    /// The path the Layout names it by.
    from: String,
    /// The path the tree holds it at.
    to: String,
    /// Whether it was edited as well as moved.
    modified: bool,
    /// Whether the move is one the two being the same bytes makes rather than one their likeness
    /// suggests.
    strong: bool,
}

/// Result: how the tree stood beside the Layout.
///
/// The reading is the same one `rola layout tree-diff` gives, so what a `--json` run reads here is
/// what a reader of that command would: every path the two disagree about, with `modified` holding
/// the ones that changed where they were as well as the destinations of moves.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultStatus {
    /// Paths the Layout names that the tree does not hold.
    lost: Vec<String>,
    /// Paths the tree holds that the Layout does not name.
    untagged: Vec<String>,
    /// Paths that changed where they were, including the destinations of moves.
    modified: Vec<String>,
    /// Paths that moved.
    renamed: Vec<RenameItem>,
}

#[renderer(buffer)]
pub fn render_result_status(result: ResultStatus) {
    let structural = result.renamed.len() + result.lost.len() + result.untagged.len();

    // A move's destination is already a line of its own among the moves, so what is left for the
    // content that changed is everything modified that did not move.
    let moved: BTreeSet<&str> = result
        .renamed
        .iter()
        .map(|rename| rename.to.as_str())
        .collect();
    let changed: Vec<&str> = result
        .modified
        .iter()
        .filter(|path| !moved.contains(path.as_str()))
        .map(String::as_str)
        .collect();

    if structural == 0 && changed.is_empty() {
        r_println!("{}", trd!(t!("status.clean")).trim());
    } else {
        if structural > 0 {
            let header = if changed.is_empty() || result.renamed.is_empty() {
                trd!(t!("status.header_structural", count = structural))
            } else {
                trd!(t!(
                    "status.header_structural_matched",
                    count = structural,
                    renamed = result.renamed.len()
                ))
            };

            r_println!("{}", header.trim());
            r_println!("");

            for rename in &result.renamed {
                let mark = if rename.modified {
                    t!("status.mark_modified").to_string()
                } else {
                    String::new()
                };

                let line = format!("[[cyan]]>[[/]] {} -> {}{}", rename.from, rename.to, mark);
                r_println!("{}", trd!(line));
            }
            for path in &result.lost {
                r_println!("{}", trd!(format!("[[red]]-[[/]] {path}")));
            }
            for path in &result.untagged {
                r_println!("{}", trd!(format!("[[green]]+[[/]] {path}")));
            }
        }

        if !changed.is_empty() {
            // A blank line separates the two blocks; the first block has none before it.
            if structural > 0 {
                r_println!("");
            }

            let header = if structural == 0 {
                trd!(t!("status.header_content", count = changed.len()))
            } else {
                trd!(t!("status.header_content_more", count = changed.len()))
            };
            r_println!("{}", header.trim());
            r_println!("");

            let mark = t!("status.mark_modified");
            for path in &changed {
                r_println!("{}", trd!(format!("  {path} {mark}")));
            }
        }

        r_println!("");

        if structural > 0 {
            let said = if changed.is_empty() {
                t!("status.help_align")
            } else {
                t!("status.help_align_both")
            };
            r_println!("{}", help_line!(said.trim()));
        }
        if !changed.is_empty() {
            r_println!("{}", help_line!(t!("status.help_track").trim()));
        }
    }
}
