//! The `rola tree diff-stat` command: what two tree structures hold apart.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use librorolala::layout::Layout;
use librorolala::storage::RorolalaStorage;
use librorolala::tree::{Change, ChangeKind, diff};
use librorolala::vcs::VCSIndex;
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::strip_written;
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::format::ResFormat;
use crate::layout::chosen;
use crate::tree::cmd_tree::{ErrorTreeShouldInWorkspace, locate};
use crate::tree::cmd_tree_ls_tree::resolve_structure;

/// Compare the tree's staged structure rather than the version it records.
const ARG_STAGED: PickerArg<'static, Flag> = arg![staged: Flag];

/// What a change is drawn as when the run names no template.
const DEFAULT_FORMAT: &str = "{{ changes.status }} {{ changes.path }}";

#[help(buffer)]
pub fn help_tree_diff_stat(_: EntryTreeDiffStat, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.diff_stat.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeDiffStat)]
pub fn desc_tree_diff_stat() -> Description {
    t!("tree.diff_stat.description").to_string().into()
}

/// Completes what `rola tree diff-stat` can be given next.
#[completion(EntryTreeDiffStat)]
pub fn complete_tree_diff_stat(ctx: ShellContext) -> Suggest {
    if crate::complete::typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_STAGED: t!("tree.diff_stat.complete.staged"),
            },
        );
    }

    Suggest::file_comp()
}

/// Compares tree structures and says what holds apart
///
/// With no operand, the version the tree records is compared with the work on disk. With one, it is
/// compared with that operand, and with two, the operands are compared with each other. An operand
/// is a tree, a version hash or a variant hash.
///
/// `--staged` compares the staged structure in place of the recorded version on the left.
///
/// Each path is one line, with the letter of what changed: `A` added, `M` content, `X` metadata,
/// `D` deleted, `T` type changed.
///
/// # Errors
///
/// Renders [`ErrorTreeDiffStatArgument`] when more than two operands are named or one does not read,
/// [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorTreeDiffStatFailed`] when a structure or the work cannot be read.
#[command(node = "tree.diff-stat", entry = EntryTreeDiffStat)]
pub fn tree_diff_stat(args: EntryTreeDiffStat, format: &mut ResFormat) -> Next {
    format.default_template(DEFAULT_FORMAT);

    let picked = args
        .pick(ARG_STAGED)
        .pick(arg![Vec<String>])
        .pick(&UNMATCHED)
        .to_result();
    let (staged, operands, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree diff-stat") {
        return next;
    }

    if operands.len() > 2 {
        return ErrorTreeDiffStatArgument.into();
    }

    StateTreeDiffStat {
        staged: matches!(staged, Flag::Active),
        operands,
    }
    .into()
}

/// The state a comparison starts in.
#[derive(Grouped)]
pub struct StateTreeDiffStat {
    /// Whether the left side is the staged structure.
    staged: bool,
    /// The operands named, as they were written.
    operands: Vec<String>,
}

#[chain(routeify)]
pub fn handle_tree_diff_stat(
    state: StateTreeDiffStat,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    format: &mut ResFormat,
) -> Next {
    let StateTreeDiffStat { staged, operands } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };
    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorTreeDiffStatFailed::new(t!("tree.err_no_store").trim().to_string()).into();
    };
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorTreeDiffStatFailed::new(t!("tree.err_no_index").trim().to_string()).into();
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return ErrorTreeDiffStatFailed::new(error.to_string()).into(),
    };

    match compare(held, &layout, store, index, &operands, staged, &runtime) {
        Ok(result) => {
            let rows = result
                .changes
                .iter()
                .map(|change| {
                    serde_json::json!({
                        "status": letter(change.change),
                        "path": change.path,
                        "kind": serde_json::to_value(change.kind).unwrap_or_default(),
                    })
                })
                .collect();
            format.set("changes", rows);

            result.into()
        }
        Err(next) => next,
    }
}

/// The letter one kind of change is drawn by.
const fn letter(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "A",
        ChangeKind::Modified => "M",
        ChangeKind::Metadata => "X",
        ChangeKind::Deleted => "D",
        ChangeKind::TypeChanged => "T",
        ChangeKind::Renamed => "R",
    }
}

/// The two structures a run asked to compare, and what holds apart.
// What a comparison is asked is a reading rather than a value it takes apart, so it is passed as the
// small record it is.
#[allow(clippy::needless_pass_by_value)]
fn compare(
    held: &Workspace,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    operands: &[String],
    staged: bool,
    runtime: &tokio::runtime::Runtime,
) -> Result<ResultTreeDiffStat, Next> {
    let (before, before_name, after, after_name) = match operands {
        [] => {
            let tree = locate(held, None)?;

            let recorded = match runtime.block_on(tree.recorded_structure(layout, index, store)) {
                Ok(recorded) => recorded,
                Err(error) => {
                    return Err(ErrorTreeDiffStatFailed::new(error.to_string()).into());
                }
            };

            let before = if staged {
                match tree.staging().structure() {
                    Ok(Some(staged)) => staged,
                    Ok(None) => recorded,
                    Err(error) => {
                        return Err(ErrorTreeDiffStatFailed::new(error.to_string()).into());
                    }
                }
            } else {
                recorded
            };

            let rules = match tree.rules(crate::ignore::rules(held, false)) {
                Ok(rules) => rules,
                Err(error) => {
                    return Err(ErrorTreeDiffStatFailed::new(error.to_string()).into());
                }
            };
            let scanned = match tree.scan(&rules, false) {
                Ok(scanned) => scanned,
                Err(error) => {
                    return Err(ErrorTreeDiffStatFailed::new(error.to_string()).into());
                }
            };

            (
                before,
                if staged {
                    "staged".to_owned()
                } else {
                    "recorded".to_owned()
                },
                scanned.structure,
                "working".to_owned(),
            )
        }
        [one] => {
            let tree = locate(held, Some(one))?;
            let recorded = match runtime.block_on(tree.recorded_structure(layout, index, store)) {
                Ok(recorded) => recorded,
                Err(error) => {
                    return Err(ErrorTreeDiffStatFailed::new(error.to_string()).into());
                }
            };

            let (before, before_name) = if staged {
                match tree.staging().structure() {
                    Ok(Some(staged)) => (staged, "staged".to_owned()),
                    Ok(None) => (recorded, "staged".to_owned()),
                    Err(error) => {
                        return Err(ErrorTreeDiffStatFailed::new(error.to_string()).into());
                    }
                }
            } else {
                (recorded, "recorded".to_owned())
            };

            let after = resolve_structure(held, layout, store, index, Some(one.as_str()), runtime)?;

            (before, before_name, after, one.clone())
        }
        [one, two] => {
            let before =
                resolve_structure(held, layout, store, index, Some(one.as_str()), runtime)?;
            let after = resolve_structure(held, layout, store, index, Some(two.as_str()), runtime)?;

            (before, one.clone(), after, two.clone())
        }
        _ => return Err(ErrorTreeDiffStatArgument.into()),
    };

    Ok(ResultTreeDiffStat {
        before: before_name,
        after: after_name,
        changes: diff(&before, &after),
    })
}

/// Result: two tree structures were compared.
#[derive(Grouped, StructuralData, serde::Serialize)]
pub struct ResultTreeDiffStat {
    /// What the left side is.
    before: String,
    /// What the right side is.
    after: String,
    /// The paths that hold apart.
    changes: Vec<Change>,
}

#[renderer(buffer)]
pub fn render_result_tree_diff_stat(
    result: ResultTreeDiffStat,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!("{}", err_line!(error.to_string()));
                ec.exit_code = EC_ERR_TREE_FAILED;
            }
        }
    } else {
        r_println!(
            "{}",
            t!(
                "tree.diff_stat.header",
                before = result.before,
                after = result.after,
                count = result.changes.len().to_string()
            )
            .trim()
        );

        let mut added = 0;
        let mut modified = 0;
        let mut metadata = 0;
        let mut deleted = 0;
        let mut changed_type = 0;
        for change in &result.changes {
            match change.change {
                ChangeKind::Added => added += 1,
                ChangeKind::Modified => modified += 1,
                ChangeKind::Metadata => metadata += 1,
                ChangeKind::Deleted => deleted += 1,
                ChangeKind::TypeChanged => changed_type += 1,
                ChangeKind::Renamed => {}
            }
        }

        r_println!(
            "{}",
            t!(
                "tree.diff_stat.counts",
                added = added.to_string(),
                modified = modified.to_string(),
                deleted = deleted.to_string(),
                metadata = metadata.to_string(),
                type_changed = changed_type.to_string()
            )
            .trim()
        );

        for change in &result.changes {
            r_println!("  {} {}", letter(change.change), change.path);
        }
    }
}

/// Error: the comparison cannot be made.
#[derive(Grouped)]
pub struct ErrorTreeDiffStatArgument;

impl Failure for ErrorTreeDiffStatArgument {
    fn name(&self) -> &'static str {
        "error_tree_diff_stat_argument"
    }

    fn reason(&self) -> String {
        t!("tree.diff_stat.err_argument").trim().to_string()
    }
}

failure!(ErrorTreeDiffStatArgument);

#[renderer(buffer)]
pub fn render_error_tree_diff_stat_argument(_: ErrorTreeDiffStatArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.diff_stat.err_argument").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tree.diff_stat.err_argument_help").trim())
    );
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: a structure or the work cannot be read.
#[derive(Grouped)]
pub struct ErrorTreeDiffStatFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeDiffStatFailed {
    /// The failure of a comparison, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeDiffStatFailed {
    fn name(&self) -> &'static str {
        "error_tree_diff_stat_failed"
    }

    fn reason(&self) -> String {
        t!("tree.diff_stat.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeDiffStatFailed);

#[renderer(buffer)]
pub fn render_error_tree_diff_stat_failed(error: ErrorTreeDiffStatFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("tree.diff_stat.err_failed_help").trim())
    );
    ec.exit_code = EC_ERR_TREE_FAILED;
}
