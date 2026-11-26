//! The `rola collapse` command: fold the local versions above the tracked Vault's version into one.
//!
//! A run names one file or many. A target chooses the first version folded, and names a version only
//! when one path is named; `--keep` holds a version apart from the fold, and `--keep-join` holds a
//! merge apart the same way. The fold itself is the library's, see [`librorolala::write::collapse`]:
//! what is here is the shape of the command, the paths it resolves, the words it prints and the exit
//! code it ends on.

// A collapse reads a path, a Layout, a chain, a target and the account it may reach before it hands
// the work over, and every step of that is a refusal of its own to say. The handler's parameters are
// the resources the framework injects, so how many of them there are is what the command needs of the
// run rather than a signature this program shaped.
#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::trivially_copy_pass_by_ref
)]

use librorolala::layout::LayoutPath;
use librorolala::status::tracked_vault;
use librorolala::write::collapse::{self, CollapseOptions, CollapseRefusal};
use librorolala::write::path::local_path;
use librorolala::write::retrack::tree_above;
use librorolala::write::tweak::{self, Chain, Request as TweakRequest, TargetMiss, TweakRefusal};
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, renderer, routeify,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResConcurrency, ResCurrentRemoteVault, ResOffline, ResProgressSetting, ResStorageSet,
    ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{offer, positional, strip_written, target_words, typing_flag};
use crate::exit_codes::{EC_ERR_TWEAK, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::keys::account_named;
use crate::layout::{ErrorLayoutShouldInWorkspace, chosen};
use crate::progress::Reporting;

/// The files whose local versions are folded.
const ARG_FILES: PickerArg<'static, Vec<String>> = arg![Vec<String>];


/// A version to keep apart from the fold.
const ARG_KEEP: PickerArg<'static, Vec<String>> = arg![keep: Vec<String>];

/// Keep the merge versions in the range as checkpoints.
const ARG_KEEP_JOIN: PickerArg<'static, Flag> = arg![keep_join: Flag];

#[help(buffer)]
pub fn help_collapse(_: EntryCollapse, ec: &mut ResExitCode) {
    eprintln!("{}", trd!(t!("collapse.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryCollapse)]
pub fn desc_collapse() -> Description {
    t!("collapse.description").to_string().into()
}

/// Completes what `rola collapse` can be given next.
#[completion(EntryCollapse)]
pub fn complete_collapse(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_KEEP: t!("collapse.complete.keep"),
                ARG_KEEP_JOIN: t!("collapse.complete.keep_join"),
            },
        );
    }

    match positional(&ctx, "collapse") {
        0 => Suggest::file_comp(),
        _ => offer(
            &ctx,
            target_words(
                workspace.get_ref(),
                vault.get_ref(),
                index.get_ref().as_ref(),
            ),
        ),
    }
}

/// Folds the local versions above the tracked Vault's version into one
///
/// `PATH` is one or more local paths in this Workspace, each of which the Layout records. `TARGET`
/// names the first version folded, and is allowed only when one path is named: `~N` N versions back,
/// a version's hash or the head of one, the hash of the variant that produced a version, or text
/// that names one creator or one message.
///
/// `--keep` names a version to hold apart from the fold, and `--keep-join` holds every merge version
/// in the range apart. A held version stays as it is; the versions between two held ones, or between
/// the lower bound and the record, fold into the first of them.
///
/// # Errors
///
/// Renders [`ErrorCollapseNoFile`] when no file is named, [`ErrorCollapseTargetManyFiles`] when a
/// target or a `--keep` is given with more than one path, [`ErrorCollapseFile`] when a word names no
/// path of this Workspace, [`ErrorCollapseNotRecorded`] when the Layout records no version for it,
/// [`ErrorCollapseNoChain`] when what it records is not the top of a chain, [`ErrorCollapseMerging`]
/// when a variant waits to be joined into it, [`ErrorCollapseTarget`] when a target names no version
/// or more than one, [`ErrorCollapseNothing`] when there is nothing above the origin to fold,
/// [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorCollapseFailed`] when the store, the index or the Layout refuses.
#[command(node = "collapse", entry = EntryCollapse)]
pub fn collapse(args: EntryCollapse) -> Next {
    let picked = args
        .pick(ARG_KEEP)
        .pick(ARG_KEEP_JOIN)
        .pick_or_route(ARG_FILES, || ErrorCollapseNoFile.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (keep, keep_join, files, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "collapse") {
        return next;
    }

    StateCollapse {
        files,
        keep,
        keep_join: matches!(keep_join, Flag::Active),
    }
    .into()
}

/// The state a collapse starts in.
#[derive(Grouped)]
pub struct StateCollapse {
    /// The words named, as they were written: the paths, and the target when one is named.
    files: Vec<String>,
    /// The versions to keep apart from the fold.
    keep: Vec<String>,
    /// Whether a merge version in the range is kept as a checkpoint.
    keep_join: bool,
}

#[chain(routeify)]
pub fn handle_collapse(
    state: StateCollapse,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResStorageSet>,
    index: &mut LazyRes<ResVCSIndex>,
    current: &mut LazyRes<ResCurrentAccount>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    offline: &ResOffline,
    concurrency: &ResConcurrency,
    progress: &ResProgressSetting,
) -> Next {
    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };
    let remote = remote.get_ref();
    let current = current.get_ref();
    let root = held.get_root().to_path_buf();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return failed(&state.files.first().cloned().unwrap_or_default(), error.to_string()),
    };

    let Some(index) = index.get_ref().as_ref() else {
        return failed(
            &state.files.first().cloned().unwrap_or_default(),
            t!("tweak.err_no_index").trim().to_owned(),
        );
    };

    // A target is a last word that names a version on the first path's chain, and only when one path
    // is named. Everything else is a path, which is what lets `collapse a.psd` and `collapse a.psd ~1`
    // read the way a person writes them.
    let mut files = state.files.clone();
    let mut target = None;
    if files.len() >= 2 {
        let first = files.first().cloned().unwrap_or_default();
        let last = files.last().cloned().unwrap_or_default();

        if let Ok(path) = local_path(&root, &cwd, &first) {
            let path = tree_above(&layout, &path).unwrap_or(path);

            if layout.id_of(&path).is_some()
                && let Ok(chain) = tweak::chain_of(index, &layout, &path)
                && tweak::resolve(&chain, Some(&last)).is_ok()
            {
                target = files.pop();
            }
        }
    }
    if target.is_some() && files.len() > 1 {
        return ErrorCollapseTargetManyFiles { count: files.len() }.into();
    }
    if target.is_none() && !state.keep.is_empty() && files.len() > 1 {
        return ErrorCollapseTargetManyFiles { count: files.len() }.into();
    }

    // Every path is read before any is written, so a file the run cannot fold stops it before an
    // earlier one has moved.
    let mut targets: Vec<(String, LayoutPath, Uuid, Chain)> = Vec::new();
    for file in &files {
        let Ok(path) = local_path(&root, &cwd, file) else {
            return ErrorCollapseFile { file: file.clone() }.into();
        };
        let path = tree_above(&layout, &path).unwrap_or(path);

        let Some(id) = layout.id_of(&path) else {
            return ErrorCollapseNotRecorded { file: file.clone() }.into();
        };
        if layout.entry(id).is_none() {
            return ErrorCollapseNotRecorded { file: file.clone() }.into();
        }

        let chain = match tweak::chain_of(index, &layout, &path) {
            Ok(chain) => chain,
            Err(TweakRefusal::NotRecorded | TweakRefusal::Nothing) => {
                return ErrorCollapseNotRecorded { file: file.clone() }.into();
            }
            Err(TweakRefusal::NotAChain) => {
                return ErrorCollapseNoChain { file: file.clone() }.into();
            }
            Err(TweakRefusal::Merging { variant }) => {
                return ErrorCollapseMerging {
                    file: file.clone(),
                    variant,
                }
                .into();
            }
            Err(refusal) => return failed(file, refusal.to_string()),
        };

        targets.push((file.clone(), path, id, chain));
    }

    let tracked = tracked_vault(held);
    let reach = tracked.as_deref().and_then(|tracked| {
        remote.name_or_default(tracked).ok().and_then(|name| {
            remote
                .vault_or_default(name.clone())
                .ok()
                .map(|address| (name, address))
        })
    });
    let candidates = fetch::candidates(remote, reach);
    let account = current
        .must_bind()
        .ok()
        .and_then(|name| account_named(&name, Some(held), None).ok());
    let request = TweakRequest {
        workspace: held,
        layout: &layout,
        index: Some(index),
        store: storage.get_ref().storage(),
        account: account.as_ref(),
        candidates: &candidates,
        offline: **offline,
        concurrency: **concurrency,
    };
    let options = CollapseOptions {
        target,
        keep: state.keep.clone(),
        keep_join: state.keep_join,
    };
    let mut journal = librorolala::write::Journal::open(held, "collapse");
    let cancellation = crate::cancel::on_ctrl_c();

    for (file, path, id, chain) in targets {
        let reporting = Reporting::start(*progress);
        let outcome = collapse::collapse(
            &request,
            id,
            &path,
            &options,
            &reporting.progress(),
            cancellation,
            &mut journal,
        );
        reporting.finish();

        match outcome {
            Ok(outcome) => println!(
                "{}",
                trd!(t!(
                    "collapse.result",
                    file = file,
                    from = outcome.from,
                    to = outcome.to,
                    count = outcome.folded
                ))
                .trim()
            ),
            Err(refusal) => return refused(&file, &chain, refusal),
        }
    }

    if let Err(error) = journal.finish() {
        return failed(
            &state.files.first().cloned().unwrap_or_default(),
            error.to_string(),
        );
    }

    ().into()
}

/// Draws what a refused fold is told as.
fn refused(file: &str, chain: &Chain, refusal: CollapseRefusal) -> Next {
    match refusal {
        CollapseRefusal::NotRecorded => ErrorCollapseNotRecorded {
            file: file.to_owned(),
        }
        .into(),
        CollapseRefusal::Nothing => ErrorCollapseNothing {
            file: file.to_owned(),
        }
        .into(),
        CollapseRefusal::NotAChain => ErrorCollapseNoChain {
            file: file.to_owned(),
        }
        .into(),
        CollapseRefusal::Merging { variant } => ErrorCollapseMerging {
            file: file.to_owned(),
            variant,
        }
        .into(),
        CollapseRefusal::Target(miss) => target_miss(file, chain, miss),
        CollapseRefusal::Sheet(reason) => failed(file, reason),
        CollapseRefusal::Failed(error) => failed(file, error.to_string()),
    }
}

/// Draws what a target that names no one version is told as.
fn target_miss(file: &str, chain: &Chain, miss: TargetMiss) -> Next {
    let (target, kind) = match miss {
        TargetMiss::NoMatch { target } => (target, TargetKind::None),
        TargetMiss::Ambiguous { target, candidates } => {
            let candidates = candidates
                .iter()
                .filter_map(|number| {
                    let found = chain.found(*number).ok()?;

                    Some(Candidate {
                        variant: found.variant,
                        creator: found.creator,
                        message: found.message,
                        number: found.number,
                    })
                })
                .collect();

            (target, TargetKind::Many(candidates))
        }
        TargetMiss::Elsewhere { target } => (target, TargetKind::Other),
        TargetMiss::OutOfChain { target } => (target, TargetKind::None),
        TargetMiss::Removed { version } => (version, TargetKind::None),
    };

    ErrorCollapseTarget {
        file: file.to_owned(),
        target,
        kind,
    }
    .into()
}

fn failed(file: &str, cause: String) -> Next {
    ErrorCollapseFailed {
        file: file.to_owned(),
        cause,
    }
    .into()
}

/// One version a target matched, as the failure names it.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The variant's hash, as hex.
    pub variant: String,
    /// Who made it, as text.
    pub creator: String,
    /// What it says, as text.
    pub message: String,
    /// Its number in the chain.
    pub number: usize,
}

/// The ways a target names no one version.
#[derive(Debug, Clone)]
pub enum TargetKind {
    /// Nothing on the chain matches.
    None,
    /// More than one does, each named.
    Many(Vec<Candidate>),
    /// The hash names an object that is not on this chain.
    Other,
}

/// Error: no file was named.
#[derive(Grouped)]
pub struct ErrorCollapseNoFile;

impl Failure for ErrorCollapseNoFile {
    fn name(&self) -> &'static str {
        "error_collapse_no_file"
    }

    fn reason(&self) -> String {
        t!("collapse.err_no_file").trim().to_string()
    }
}

failure!(ErrorCollapseNoFile);

#[renderer(buffer)]
pub fn render_error_collapse_no_file(error: ErrorCollapseNoFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("collapse.err_no_file_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a target or a `--keep` was given with more than one path.
#[derive(Grouped)]
pub struct ErrorCollapseTargetManyFiles {
    /// How many paths were named.
    count: usize,
}

impl Failure for ErrorCollapseTargetManyFiles {
    fn name(&self) -> &'static str {
        "error_collapse_target_many_files"
    }

    fn reason(&self) -> String {
        t!("collapse.err_target_many_files", count = self.count)
            .trim()
            .to_string()
    }
}

failure!(ErrorCollapseTargetManyFiles);

#[renderer(buffer)]
pub fn render_error_collapse_target_many_files(
    error: ErrorCollapseTargetManyFiles,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("collapse.err_target_many_files_help").trim())
    );
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a word names no path of this Workspace.
#[derive(Grouped)]
pub struct ErrorCollapseFile {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorCollapseFile {
    fn name(&self) -> &'static str {
        "error_collapse_file"
    }

    fn reason(&self) -> String {
        t!("collapse.err_not_local", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorCollapseFile);

#[renderer(buffer)]
pub fn render_error_collapse_file(error: ErrorCollapseFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("collapse.err_not_local_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the Layout records no version for the path.
#[derive(Grouped)]
pub struct ErrorCollapseNotRecorded {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorCollapseNotRecorded {
    fn name(&self) -> &'static str {
        "error_collapse_not_recorded"
    }

    fn reason(&self) -> String {
        t!("collapse.err_not_recorded", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorCollapseNotRecorded);

#[renderer(buffer)]
pub fn render_error_collapse_not_recorded(error: ErrorCollapseNotRecorded, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("collapse.err_not_recorded_help").trim())
    );
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the recorded version is not the top of a chain.
#[derive(Grouped)]
pub struct ErrorCollapseNoChain {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorCollapseNoChain {
    fn name(&self) -> &'static str {
        "error_collapse_no_chain"
    }

    fn reason(&self) -> String {
        t!("collapse.err_no_chain", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorCollapseNoChain);

#[renderer(buffer)]
pub fn render_error_collapse_no_chain(error: ErrorCollapseNoChain, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("collapse.err_no_chain_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a variant waits to be joined into the entry.
#[derive(Grouped)]
pub struct ErrorCollapseMerging {
    /// The file as it was written.
    file: String,
    /// The variant waiting, as hex.
    variant: String,
}

impl Failure for ErrorCollapseMerging {
    fn name(&self) -> &'static str {
        "error_collapse_merging"
    }

    fn reason(&self) -> String {
        t!(
            "collapse.err_merging",
            file = self.file,
            variant = self.variant
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorCollapseMerging);

#[renderer(buffer)]
pub fn render_error_collapse_merging(error: ErrorCollapseMerging, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("collapse.err_merging_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: there is nothing above the origin to fold.
#[derive(Grouped)]
pub struct ErrorCollapseNothing {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorCollapseNothing {
    fn name(&self) -> &'static str {
        "error_collapse_nothing"
    }

    fn reason(&self) -> String {
        t!("collapse.err_nothing", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorCollapseNothing);

#[renderer(buffer)]
pub fn render_error_collapse_nothing(error: ErrorCollapseNothing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("collapse.err_nothing_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the target names no one version.
#[derive(Grouped)]
pub struct ErrorCollapseTarget {
    /// The file as it was written.
    file: String,
    /// The target as it was written.
    target: String,
    /// Why it named no one version.
    kind: TargetKind,
}

impl Failure for ErrorCollapseTarget {
    fn name(&self) -> &'static str {
        "error_collapse_target"
    }

    fn reason(&self) -> String {
        match &self.kind {
            TargetKind::Many(candidates) => t!(
                "collapse.err_target_many",
                file = self.file,
                target = self.target,
                count = candidates.len()
            )
            .trim()
            .to_string(),
            TargetKind::Other => t!("collapse.err_target_other", target = self.target)
                .trim()
                .to_string(),
            TargetKind::None => t!(
                "collapse.err_target_none",
                file = self.file,
                target = self.target
            )
            .trim()
            .to_string(),
        }
    }
}

failure!(ErrorCollapseTarget);

#[renderer(buffer)]
pub fn render_error_collapse_target(error: ErrorCollapseTarget, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    match &error.kind {
        TargetKind::Many(candidates) => {
            for candidate in candidates {
                r_eprintln!(
                    "{}",
                    trd!(t!(
                        "tweak.query_row",
                        variant = candidate.variant,
                        creator = candidate.creator,
                        message = candidate.message,
                        number = candidate.number
                    ))
                    .trim()
                );
            }
            r_eprintln!(
                "{}",
                help_line!(t!("collapse.err_target_many_help").trim())
            );
        }
        TargetKind::Other => {
            r_eprintln!(
                "{}",
                help_line!(t!("collapse.err_target_other_help", file = error.file).trim())
            );
        }
        TargetKind::None => {
            r_eprintln!(
                "{}",
                help_line!(t!("collapse.err_target_none_help").trim())
            );
        }
    }
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the fold could not be carried out.
#[derive(Grouped)]
pub struct ErrorCollapseFailed {
    /// The file as it was written.
    file: String,
    /// What stopped it.
    cause: String,
}

impl Failure for ErrorCollapseFailed {
    fn name(&self) -> &'static str {
        "error_collapse_failed"
    }

    fn reason(&self) -> String {
        t!("collapse.err_failed", file = self.file, cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorCollapseFailed);

#[renderer(buffer)]
pub fn render_error_collapse_failed(error: ErrorCollapseFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("collapse.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}
