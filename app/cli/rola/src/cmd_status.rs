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

use std::str::FromStr as _;

use librorolala::layout::{Layout, LayoutPath};
use librorolala::storage::{Blake3Hash, Key};
use librorolala::tree_analyze::tree_diff;
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{filling_flag, positional, strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_LAYOUT_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::layout::{ErrorLayoutFailed, ErrorLayoutShouldInWorkspace, chosen, readonly_layout_dir};
use crate::ownership;
use crate::vcs_index::cmd_vcs_index_lookback::{ResLookback, from_object};
use crate::vcs_index::{ErrorVcsIndexNoIndex, ErrorVcsIndexRead, parse_hash, runtime};

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

/// Completes what `rola status` can be given next.
///
/// What is read is the tree, so a target that names a path is answered by the filesystem. The flags
/// are the look-back ones the command shares with `vcs-index lookback`; the length one takes a
/// number, which is nothing a list of names can offer.
#[completion(EntryStatus)]
pub fn complete_status(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                crate::vcs_index::cmd_vcs_index_lookback::ARG_COMPACT: t!("status.complete.compact"),
                crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_MESSAGE: t!("status.complete.no_message"),
                crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_CREATOR: t!("status.complete.no_creator"),
                crate::vcs_index::cmd_vcs_index_lookback::ARG_MAX_MESSAGE_LENGTH: t!("status.complete.max_message_length"),
            },
        );
    }

    if filling_flag(
        &ctx,
        &crate::vcs_index::cmd_vcs_index_lookback::ARG_MAX_MESSAGE_LENGTH,
    ) {
        return suggest!();
    }

    if positional(&ctx, "status") == 0 {
        Suggest::file_comp()
    } else {
        suggest!()
    }
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
pub fn status(args: EntryStatus, lookback: &mut ResLookback) -> Next {
    let picked = args
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_COMPACT)
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_MESSAGE)
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_CREATOR)
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_MAX_MESSAGE_LENGTH)
        .pick(&arg![Option<String>])
        .to_result();
    let (compact, no_message, no_creator, max_message_length, target) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    lookback.asked(compact, no_message, no_creator, max_message_length);

    StateStatus { target }.into()
}

/// The state a reading of the tree starts in.
#[derive(Grouped)]
pub struct StateStatus {
    /// What to look back from, when the run named something rather than asking how the work stands.
    target: Option<String>,
}

#[chain(routeify)]
pub fn handle_status(
    state: StateStatus,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    account: &mut LazyRes<ResCurrentAccount>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };

    if let Some(target) = state.target {
        return lookback_of(&target, &layout, held, index);
    }

    let diff = match tree_diff(&layout, held, DEFAULT_ALIKE) {
        Ok(diff) => diff,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    // What is not this account's to change is read before anything is said, and kept out of the
    // content changes: a run that read them as its own work would be reading past the one thing it
    // is not to do.
    let me = account.get_ref().must_bind().ok();
    let tracked = ownership::tracked_vault(held, &layout);
    let unowned = match ownership::unowned(held, &layout, tracked.as_deref(), me.as_deref(), &diff)
    {
        Ok(unowned) => unowned,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };
    let not_mine: BTreeSet<&str> = unowned.iter().map(String::as_str).collect();

    // Which of the moves were edited as well: a move's destination is among the modified when what
    // it holds is not what the Layout agreed with at the path it came from.
    let edited: BTreeSet<&str> = diff.modified.iter().map(LayoutPath::as_str).collect();

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
            .filter(|path| !not_mine.contains(path.as_str()))
            .map(|path| path.as_str().to_owned())
            .collect(),
        renamed: diff
            .renamed
            .iter()
            .map(|rename| RenameItem {
                from: rename.from.as_str().to_owned(),
                to: rename.to.as_str().to_owned(),
                modified: edited.contains(rename.to.as_str()),
                strong: rename.strong,
            })
            .collect(),
        unowned,
    }
    .into()
}

/// One thing a target named: the index object to look back from, and whether the work on it has
/// changes that were never recorded.
struct Found {
    /// The hash of the object the chain is drawn from.
    key: Key,
    /// Whether the file the version belongs to has changes that were never recorded.
    editing: bool,
}

/// Draws the chain what `target` names sits at the top of.
///
/// What a target may name is read in the order a run is likely to have meant: a path in the Layout
/// being worked in, where a path of the Layout has moved to in the tree, a path in the Vault's own
/// Layout as the copy here has it, a `Uuid`, then the hash of an index object. The first that names
/// something is what is drawn — so a name that is a path of this work is the work's, even when some
/// object of the index happens to hash to it — and a target that names nothing at all is refused
/// rather than guessed at.
#[routeify]
fn lookback_of(
    target: &str,
    layout: &Layout,
    held: &Workspace,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let remote = tracked_layout(held);
    let found = match found(target, layout, remote.as_ref(), held) {
        Ok(found) => found,
        Err(cause) => {
            return ErrorStatusTarget {
                target: target.to_owned(),
                cause,
            }
            .into();
        }
    };

    let mut result = match from_object(index, &runtime, found.key) {
        Ok(result) => result,
        Err(cause) => return ErrorVcsIndexRead::new(cause).into(),
    };
    if found.editing {
        result.editing();
    }

    result.into()
}

/// The copy of the Vault's own Layout the Layout being worked in tracks, when there is one here.
///
/// A target that names a path of the Vault's is read from it. A Layout that tracks no Vault, or one
/// nobody has fetched, has no copy: what the run named is then read as something else, or refused.
fn tracked_layout(held: &Workspace) -> Option<Layout> {
    let layouts = held.layouts();
    let name = layouts.current().ok().flatten()?;
    let track = layouts.track(&name).ok().flatten()?;
    let dir = readonly_layout_dir(held, &track, VAULT_LAYOUT_NAME);

    Layout::open(&dir).ok()
}

/// Reads what `target` names, in the order a run is likely to have meant it.
fn found(
    target: &str,
    layout: &Layout,
    remote: Option<&Layout>,
    held: &Workspace,
) -> Result<Found, String> {
    let path = LayoutPath::new(target).ok();

    if let Some(path) = &path
        && let Some(id) = layout.id_of(path)
    {
        return Ok(Found {
            key: Key::new(version_of(layout, id)?),
            editing: editing(held, layout, path)?,
        });
    }

    // A path the Layout does not name is still a path of this work when the tree reading finds a
    // path it does name moved there: a move's destination is how a run asks after the file that
    // moved, since that is where it is now. What is on disk is asked before the reading is, so a
    // hash — which also reads as a path — does not walk the tree for nothing.
    if let Some(path) = &path
        && held.get_root().join(path.to_path_buf()).is_file()
    {
        let diff = tree_diff(layout, held, DEFAULT_ALIKE).map_err(|error| error.to_string())?;

        if let Some(rename) = diff.renamed.iter().find(|rename| rename.to == *path)
            && let Some(id) = layout.id_of(&rename.from)
        {
            return Ok(Found {
                key: Key::new(version_of(layout, id)?),
                editing: diff.modified.contains(&rename.to),
            });
        }
    }

    if let Some(remote) = remote
        && let Some(path) = &path
        && let Some(id) = remote.id_of(path)
    {
        return Ok(Found {
            key: Key::new(version_of(remote, id)?),
            editing: false,
        });
    }

    if let Ok(id) = Uuid::from_str(target) {
        if let Some(path) = layout.path_of(id) {
            return Ok(Found {
                key: Key::new(version_of(layout, id)?),
                editing: editing(held, layout, &path)?,
            });
        }

        if let Some(remote) = remote
            && remote.entry(id).is_some()
        {
            return Ok(Found {
                key: Key::new(version_of(remote, id)?),
                editing: false,
            });
        }
    }

    // A `Uuid` and a path are read by shape, so what is left is a hash: which kind of object it is
    // is what reading it settles, and one that is no chain is refused where it is read.
    if let Some(key) = parse_hash(target) {
        return Ok(Found {
            key,
            editing: false,
        });
    }

    Err(t!("status.err_target_names_nothing").trim().to_owned())
}

/// The version the Layout names the entry `id` at.
///
/// A `Uuid` the Layout names but was never given a version for is one no chain hangs from: what was
/// recorded is what a lookback reads, and there is nothing recorded.
fn version_of(layout: &Layout, id: Uuid) -> Result<Blake3Hash, String> {
    let Some(data) = layout.entry(id) else {
        return Err(t!("status.err_target_no_version").trim().to_owned());
    };
    let version = data.version();

    if version == [0; 32] {
        return Err(t!("status.err_target_no_version").trim().to_owned());
    }

    Ok(version)
}

/// Whether the file at `path` has changes the Layout does not name yet.
///
/// A move whose file was edited is already among the modified: the tree reading counts a move's
/// destination there as well, since what changed is the content at the path the file is at now.
fn editing(held: &Workspace, layout: &Layout, path: &LayoutPath) -> Result<bool, String> {
    let diff = tree_diff(layout, held, DEFAULT_ALIKE).map_err(|error| error.to_string())?;

    Ok(diff.modified.contains(path))
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

/// Error: what the run named is nothing a chain can be looked back from.
#[derive(Grouped)]
pub struct ErrorStatusTarget {
    /// What the run named.
    pub target: String,
    /// Why nothing was read from it.
    pub cause: String,
}

impl Failure for ErrorStatusTarget {
    fn name(&self) -> &'static str {
        "error_status_target"
    }

    fn reason(&self) -> String {
        t!(
            "status.err_target",
            target = self.target,
            cause = self.cause
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStatusTarget);

#[renderer(buffer)]
pub fn render_error_status_target(error: ErrorStatusTarget, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("status.err_target_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Result: how the tree stood beside the Layout.
///
/// The reading is the same one `rola layout tree-diff` gives, so what a `--json` run reads here is
/// what a reader of that command would: every path the two disagree about, with `modified` holding
/// the ones that changed where they were as well as the destinations of moves — except the ones the
/// fetched copy says are not this account's, which are held apart in `unowned` rather than counted
/// among the run's own work.
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
    /// Paths the fetched copy of the Vault's Layout says another account holds.
    unowned: Vec<String>,
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

    if structural == 0 && changed.is_empty() && result.unowned.is_empty() {
        r_println!("{}", trd!(t!("status.clean")).trim());
    } else {
        let mark = t!("status.mark_modified");
        let unowned = !result.unowned.is_empty();

        // What must not be recorded is said first: a run that reads the rest as its work has read past
        // the one thing it is not to do.
        if unowned {
            r_println!(
                "{}",
                trd!(t!("status.header_unowned", count = result.unowned.len())).trim()
            );
            r_println!("{}", trd!(t!("status.body_unowned")).trim());
            r_println!("");

            for path in &result.unowned {
                r_println!("{}", trd!(format!("  {path} {mark}")));
            }
        }

        if structural > 0 {
            // A blank line separates the blocks; the first block has none before it.
            if unowned {
                r_println!("");
            }

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
            // A blank line separates the blocks; the first block has none before it.
            if unowned || structural > 0 {
                r_println!("");
            }

            let header = if structural == 0 {
                trd!(t!("status.header_content", count = changed.len()))
            } else {
                trd!(t!("status.header_content_more", count = changed.len()))
            };
            r_println!("{}", header.trim());
            r_println!("");

            for path in &changed {
                r_println!("{}", trd!(format!("  {path} {mark}")));
            }
        }

        r_println!("");

        if !result.unowned.is_empty() {
            r_println!("{}", help_line!(t!("status.help_unowned").trim()));
        }
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
