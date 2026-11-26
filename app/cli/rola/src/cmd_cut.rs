//! The `rola cut` command: cut a file's old history away and start a new file at a kept version.
//!
//! A run names one file or many. A target chooses the first version the new chain keeps, and is
//! allowed only when one path is named. `--keep-uuid` keeps the entry's `Uuid`, which forks it from
//! the Vault, and `--strip-join` drops every merge in the kept range. The cut itself is the
//! library's, see [`librorolala::write::cut`]: what is here is the shape of the command, the paths it
//! resolves, the words it prints and the exit code it ends on.

// A cut reads a path, a Layout, a chain, a target and the account it may reach before it hands the
// work over, and every step of that is a refusal of its own to say. The handler's parameters are the
// resources the framework injects, so how many of them there are is what the command needs of the run
// rather than a signature this program shaped.
#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::trivially_copy_pass_by_ref
)]

use librorolala::layout::LayoutPath;
use librorolala::status::tracked_vault;
use librorolala::write::cut::{self, CutOptions, CutRefusal};
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
use crate::cmd_collapse::{Candidate, TargetKind};
use crate::complete::{offer, positional, strip_written, target_words, typing_flag};
use crate::exit_codes::{EC_ERR_TWEAK, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::keys::account_named;
use crate::layout::{ErrorLayoutShouldInWorkspace, chosen};
use crate::progress::Reporting;

/// The words named: the paths, and the target when one is named.
const ARG_FILES: PickerArg<'static, Vec<String>> = arg![Vec<String>];

/// Keep the entry's `Uuid`, which forks it from the Vault.
const ARG_KEEP_UUID: PickerArg<'static, Flag> = arg![keep_uuid: Flag];

/// Drop every merge in the kept range.
const ARG_STRIP_JOIN: PickerArg<'static, Flag> = arg![strip_join: Flag];

#[help(buffer)]
pub fn help_cut(_: EntryCut, ec: &mut ResExitCode) {
    eprintln!("{}", trd!(t!("cut.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryCut)]
pub fn desc_cut() -> Description {
    t!("cut.description").to_string().into()
}

/// Completes what `rola cut` can be given next.
#[completion(EntryCut)]
pub fn complete_cut(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_KEEP_UUID: t!("cut.complete.keep_uuid"),
                ARG_STRIP_JOIN: t!("cut.complete.strip_join"),
            },
        );
    }

    match positional(&ctx, "cut") {
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

/// Cuts a file's old history away and starts a new file at a kept version
///
/// `PATH` is one or more local paths in this Workspace, each of which the Layout records. `TARGET`
/// names the first version the new chain keeps, and is allowed only when one path is named: `~N` N
/// versions back, a version's hash or the head of one, the hash of the variant that produced a
/// version, or text that names one creator or one message.
///
/// The kept versions are replayed onto the root, each carrying its content, creator, message and
/// merge. `--strip-join` clears every merge and leaves the joined variants out. The entry is given a
/// fresh `Uuid` unless `--keep-uuid` is given, which forks it from the Vault.
///
/// # Errors
///
/// Renders [`ErrorCutNoFile`] when no file is named, [`ErrorCutTargetManyFiles`] when a target is
/// given with more than one path, [`ErrorCutFile`] when a word names no path of this Workspace,
/// [`ErrorCutNotRecorded`] when the Layout records no version for it, [`ErrorCutNoChain`] when what
/// it records is not the top of a chain, [`ErrorCutMerging`] when a variant waits to be joined into
/// it, [`ErrorCutTarget`] when a target names no version or more than one,
/// [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace, and [`ErrorCutFailed`]
/// when the store, the index or the Layout refuses.
#[command(node = "cut", entry = EntryCut)]
pub fn cut(args: EntryCut) -> Next {
    let picked = args
        .pick(ARG_KEEP_UUID)
        .pick(ARG_STRIP_JOIN)
        .pick_or_route(ARG_FILES, || ErrorCutNoFile.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (keep_uuid, strip_join, files, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "cut") {
        return next;
    }

    StateCut {
        files,
        keep_uuid: matches!(keep_uuid, Flag::Active),
        strip_join: matches!(strip_join, Flag::Active),
    }
    .into()
}

/// The state a cut starts in.
#[derive(Grouped)]
pub struct StateCut {
    /// The words named, as they were written: the paths, and the target when one is named.
    files: Vec<String>,
    /// Whether the entry keeps its `Uuid`.
    keep_uuid: bool,
    /// Whether every merge in the kept range is dropped.
    strip_join: bool,
}

#[chain(routeify)]
pub fn handle_cut(
    state: StateCut,
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
        Err(error) => {
            return failed(
                &state.files.first().cloned().unwrap_or_default(),
                error.to_string(),
            );
        }
    };

    let Some(index) = index.get_ref().as_ref() else {
        return failed(
            &state.files.first().cloned().unwrap_or_default(),
            t!("tweak.err_no_index").trim().to_owned(),
        );
    };

    // A target is a last word that names a version on the first path's chain, and only when one path
    // is named. Everything else is a path, which is what lets `cut a.psd` and `cut a.psd ~1` read the
    // way a person writes them.
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
        return ErrorCutTargetManyFiles { count: files.len() }.into();
    }

    // Every path is read before any is written, so a file the run cannot cut stops it before an
    // earlier one has moved.
    let mut targets: Vec<(String, LayoutPath, Uuid, Chain)> = Vec::new();
    for file in &files {
        let Ok(path) = local_path(&root, &cwd, file) else {
            return ErrorCutFile { file: file.clone() }.into();
        };
        let path = tree_above(&layout, &path).unwrap_or(path);

        let Some(id) = layout.id_of(&path) else {
            return ErrorCutNotRecorded { file: file.clone() }.into();
        };
        if layout.entry(id).is_none() {
            return ErrorCutNotRecorded { file: file.clone() }.into();
        }

        let chain = match tweak::chain_of(index, &layout, &path) {
            Ok(chain) => chain,
            Err(TweakRefusal::NotRecorded | TweakRefusal::Nothing) => {
                return ErrorCutNotRecorded { file: file.clone() }.into();
            }
            Err(TweakRefusal::NotAChain) => {
                return ErrorCutNoChain { file: file.clone() }.into();
            }
            Err(TweakRefusal::Merging { variant }) => {
                return ErrorCutMerging {
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
    let options = CutOptions {
        target,
        keep_uuid: state.keep_uuid,
        strip_join: state.strip_join,
    };
    let mut journal = librorolala::write::Journal::open(held, "cut");
    let cancellation = crate::cancel::on_ctrl_c();

    for (file, path, id, chain) in targets {
        let reporting = Reporting::start(*progress);
        let outcome = cut::cut(
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
            Ok(outcome) => {
                let text = if state.keep_uuid {
                    t!(
                        "cut.result_kept_uuid",
                        file = file,
                        point = outcome.point,
                        uuid = outcome.uuid,
                        to = outcome.to,
                        count = outcome.kept
                    )
                } else {
                    t!(
                        "cut.result",
                        file = file,
                        point = outcome.point,
                        to = outcome.to,
                        count = outcome.kept
                    )
                };
                println!("{}", trd!(text).trim());
            }
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

/// Draws what a refused cut is told as.
fn refused(file: &str, chain: &Chain, refusal: CutRefusal) -> Next {
    match refusal {
        CutRefusal::NotRecorded | CutRefusal::Nothing => ErrorCutNotRecorded {
            file: file.to_owned(),
        }
        .into(),
        CutRefusal::NotAChain => ErrorCutNoChain {
            file: file.to_owned(),
        }
        .into(),
        CutRefusal::Merging { variant } => ErrorCutMerging {
            file: file.to_owned(),
            variant,
        }
        .into(),
        CutRefusal::Target(miss) => target_miss(file, chain, miss),
        CutRefusal::Sheet(reason) => failed(file, reason),
        CutRefusal::Failed(error) => failed(file, error.to_string()),
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

    ErrorCutTarget {
        file: file.to_owned(),
        target,
        kind,
    }
    .into()
}

fn failed(file: &str, cause: String) -> Next {
    ErrorCutFailed {
        file: file.to_owned(),
        cause,
    }
    .into()
}

/// Error: no file was named.
#[derive(Grouped)]
pub struct ErrorCutNoFile;

impl Failure for ErrorCutNoFile {
    fn name(&self) -> &'static str {
        "error_cut_no_file"
    }

    fn reason(&self) -> String {
        t!("cut.err_no_file").trim().to_string()
    }
}

failure!(ErrorCutNoFile);

#[renderer(buffer)]
pub fn render_error_cut_no_file(error: ErrorCutNoFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("cut.err_no_file_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a target was given with more than one path.
#[derive(Grouped)]
pub struct ErrorCutTargetManyFiles {
    /// How many paths were named.
    count: usize,
}

impl Failure for ErrorCutTargetManyFiles {
    fn name(&self) -> &'static str {
        "error_cut_target_many_files"
    }

    fn reason(&self) -> String {
        t!("cut.err_target_many_files", count = self.count)
            .trim()
            .to_string()
    }
}

failure!(ErrorCutTargetManyFiles);

#[renderer(buffer)]
pub fn render_error_cut_target_many_files(error: ErrorCutTargetManyFiles, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cut.err_target_many_files_help").trim())
    );
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a word names no path of this Workspace.
#[derive(Grouped)]
pub struct ErrorCutFile {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorCutFile {
    fn name(&self) -> &'static str {
        "error_cut_file"
    }

    fn reason(&self) -> String {
        t!("cut.err_not_local", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorCutFile);

#[renderer(buffer)]
pub fn render_error_cut_file(error: ErrorCutFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("cut.err_not_local_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the Layout records no version for the path.
#[derive(Grouped)]
pub struct ErrorCutNotRecorded {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorCutNotRecorded {
    fn name(&self) -> &'static str {
        "error_cut_not_recorded"
    }

    fn reason(&self) -> String {
        t!("cut.err_not_recorded", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorCutNotRecorded);

#[renderer(buffer)]
pub fn render_error_cut_not_recorded(error: ErrorCutNotRecorded, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("cut.err_not_recorded_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the recorded version is not the top of a chain.
#[derive(Grouped)]
pub struct ErrorCutNoChain {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorCutNoChain {
    fn name(&self) -> &'static str {
        "error_cut_no_chain"
    }

    fn reason(&self) -> String {
        t!("cut.err_no_chain", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorCutNoChain);

#[renderer(buffer)]
pub fn render_error_cut_no_chain(error: ErrorCutNoChain, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("cut.err_no_chain_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a variant waits to be joined into the entry.
#[derive(Grouped)]
pub struct ErrorCutMerging {
    /// The file as it was written.
    file: String,
    /// The variant waiting, as hex.
    variant: String,
}

impl Failure for ErrorCutMerging {
    fn name(&self) -> &'static str {
        "error_cut_merging"
    }

    fn reason(&self) -> String {
        t!("cut.err_merging", file = self.file, variant = self.variant)
            .trim()
            .to_string()
    }
}

failure!(ErrorCutMerging);

#[renderer(buffer)]
pub fn render_error_cut_merging(error: ErrorCutMerging, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("cut.err_merging_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the target names no one version.
#[derive(Grouped)]
pub struct ErrorCutTarget {
    /// The file as it was written.
    file: String,
    /// The target as it was written.
    target: String,
    /// Why it named no one version.
    kind: TargetKind,
}

impl Failure for ErrorCutTarget {
    fn name(&self) -> &'static str {
        "error_cut_target"
    }

    fn reason(&self) -> String {
        match &self.kind {
            TargetKind::Many(candidates) => t!(
                "cut.err_target_many",
                file = self.file,
                target = self.target,
                count = candidates.len()
            )
            .trim()
            .to_string(),
            TargetKind::Other => t!("cut.err_target_other", target = self.target)
                .trim()
                .to_string(),
            TargetKind::None => t!(
                "cut.err_target_none",
                file = self.file,
                target = self.target
            )
            .trim()
            .to_string(),
        }
    }
}

failure!(ErrorCutTarget);

#[renderer(buffer)]
pub fn render_error_cut_target(error: ErrorCutTarget, ec: &mut ResExitCode) {
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
            r_eprintln!("{}", help_line!(t!("cut.err_target_many_help").trim()));
        }
        TargetKind::Other => {
            r_eprintln!(
                "{}",
                help_line!(t!("cut.err_target_other_help", file = error.file).trim())
            );
        }
        TargetKind::None => {
            r_eprintln!("{}", help_line!(t!("cut.err_target_none_help").trim()));
        }
    }
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the cut could not be carried out.
#[derive(Grouped)]
pub struct ErrorCutFailed {
    /// The file as it was written.
    file: String,
    /// What stopped it.
    cause: String,
}

impl Failure for ErrorCutFailed {
    fn name(&self) -> &'static str {
        "error_cut_failed"
    }

    fn reason(&self) -> String {
        t!("cut.err_failed", file = self.file, cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorCutFailed);

#[renderer(buffer)]
pub fn render_error_cut_failed(error: ErrorCutFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("cut.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}
