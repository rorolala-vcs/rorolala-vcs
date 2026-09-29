//! The `rola layout tree-diff` command: what the working tree holds beside a Layout.

use librorolala::tree_analyze::tree_diff;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_print, r_println, renderer,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable},
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_utils_cli_theme::{err_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::format::ResFormat;
use crate::layout::ErrorLayoutShouldInWorkspace as ErrorShouldInWorkspace;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutFailed, ErrorLayoutMissing, failed};

/// How alike two text files have to be to count as the same file moved, when nothing is said.
const DEFAULT_ALIKE: f32 = 0.6;

/// The flags `rola layout tree-diff` takes.
#[derive(Pickable)]
struct DiffFlags {
    /// How alike two text files have to be to count as the same file moved, from 0 to 1.
    #[arg(long)]
    alike: Option<f32>,
}

#[help(buffer)]
pub fn help_layout_tree_diff(_: EntryLayoutTreeDiff, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_tree_diff.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutTreeDiff)]
pub fn desc_layout_tree_diff() -> Description {
    t!("cmd_layout_tree_diff.description").to_string().into()
}

/// Shows what the working tree holds beside a Layout
///
/// The Layout is the one being worked in, or the one named. Each line is a mode and a path: `lost`
/// for a path the Layout names that the tree does not hold, `untagged` for one the tree holds that
/// the Layout does not name, `modified` for one that stayed and changed, `rename` for one that
/// moved, and `failed` for one that could not be read.
///
/// `--alike` is how alike two text files have to be to count as the same file moved, from `0` to
/// `1`; it is `0.6` when nothing is said. A binary file is the same file moved only when its
/// content is unchanged.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when there is no Layout being worked in and none was named,
/// [`ErrorLayoutMissing`] when the one named is not there, and [`ErrorLayoutFailed`] when the tree
/// or the Layout could not be read.
#[command(node = "layout.tree-diff", entry = EntryLayoutTreeDiff)]
pub fn layout_tree_diff(args: EntryLayoutTreeDiff) -> Next {
    let picked = args
        .pick(&arg![DiffFlags])
        .pick(&arg![Option<String>])
        .to_result();
    let (flags, name) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutTreeDiff {
        name,
        alike: flags.alike.unwrap_or(DEFAULT_ALIKE),
    }
    .into()
}

/// The state of reading a tree beside a Layout.
#[derive(Grouped)]
pub struct StateLayoutTreeDiff {
    /// The Layout to read beside, when one was named.
    name: Option<String>,
    /// How alike two text files have to be to count as moved.
    alike: f32,
}

#[chain]
pub fn handle_layout_tree_diff(
    state: StateLayoutTreeDiff,
    workspace: &mut LazyRes<ResWorkspace>,
    format: &mut ResFormat,
) -> Next {
    let StateLayoutTreeDiff { name, alike } = state;

    let Some(workspace) = workspace.get_ref().as_ref() else {
        return ErrorShouldInWorkspace.into();
    };

    let layouts = workspace.layouts();
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

    let diff = match tree_diff(&layout, workspace, alike) {
        Ok(diff) => diff,
        Err(error) => {
            return ErrorLayoutFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    format.set(
        "lost",
        diff.lost
            .iter()
            .map(|path| serde_json::json!(path.as_str()))
            .collect(),
    );
    format.set(
        "untagged",
        diff.untagged
            .iter()
            .map(|path| serde_json::json!(path.as_str()))
            .collect(),
    );
    format.set(
        "modified",
        diff.modified
            .iter()
            .map(|path| serde_json::json!(path.as_str()))
            .collect(),
    );
    format.set(
        "renamed",
        diff.renamed
            .iter()
            .map(|rename| {
                serde_json::json!({
                    "from": rename.from.as_str(),
                    "to": rename.to.as_str(),
                    "strong": rename.strong,
                })
            })
            .collect(),
    );
    format.set(
        "failed",
        diff.failed
            .iter()
            .map(|failed| serde_json::json!({ "path": failed.path, "reason": failed.reason }))
            .collect(),
    );

    ResultLayoutTreeDiff::of(&diff).into()
}

/// Result: the tree was read beside a Layout.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutTreeDiff {
    /// Paths the Layout names that the tree does not hold.
    lost: Vec<String>,
    /// Paths the tree holds that the Layout does not name.
    untagged: Vec<String>,
    /// Paths that stayed where they were and changed.
    modified: Vec<String>,
    /// Paths that moved.
    renamed: Vec<RenameItem>,
    /// Paths that could not be read.
    failed: Vec<FailedItem>,
}

impl ResultLayoutTreeDiff {
    /// The result the reading described.
    fn of(diff: &librorolala::tree_analyze::TreeDiff) -> Self {
        Self {
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
                    strong: rename.strong,
                })
                .collect(),
            failed: diff
                .failed
                .iter()
                .map(|failed| FailedItem {
                    path: failed.path.clone(),
                    reason: failed.reason.clone(),
                })
                .collect(),
        }
    }
}

/// One path that moved.
#[derive(Serialize)]
pub struct RenameItem {
    /// The path the Layout named it by.
    from: String,
    /// The path the tree holds it at.
    to: String,
    /// Whether the move is one the two being the same bytes makes rather than one their likeness
    /// suggests.
    strong: bool,
}

/// One path the reading could not look at.
#[derive(Serialize)]
pub struct FailedItem {
    /// The path as it was seen.
    path: String,
    /// Why it could not be read.
    reason: String,
}

#[renderer(buffer)]
pub fn render_result_layout_tree_diff(
    result: ResultLayoutTreeDiff,
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
        for path in &result.lost {
            r_println!("lost {path}");
        }
        for path in &result.untagged {
            r_println!("untagged {path}");
        }
        for path in &result.modified {
            r_println!("modified {path}");
        }
        for rename in &result.renamed {
            r_println!("rename {} -> {}", rename.from, rename.to);
        }
        for failed in &result.failed {
            r_println!("failed {}", failed.path);
        }
    }
}
