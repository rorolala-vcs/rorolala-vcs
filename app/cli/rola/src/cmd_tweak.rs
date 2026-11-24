//! The `rola tweak` command: adjust a chain's history, and rebuild what follows the point adjusted.
//!
//! A version is content addressed, so what a run adjusts is never a version that already exists:
//! the run writes a new variant and a new version at the point it names, replays every version
//! above it, and makes the Layout record the tail of the replayed chain. What is adjusted is a
//! sheet, one row a version, which is what the editor writes, what waits in the Workspace as a
//! plan, and what a run reads back to list what is still to do.
//!
//! The order of the work is the library's, see [`librorolala::write::tweak`]: what is read before
//! what is written, which refusal stops the run, and what happens to the file on disk. What is left
//! here is the shape of the command: the words it takes, the resources it resolves, the words it
//! prints and the exit codes it ends on.

// A tweak resolves a file, a Layout, a chain, a target, a plan and the account it may reach before
// it hands the work over, and every step of that is a refusal of its own to say. The handler's
// parameters are the resources the framework injects, so how many of them there are is what the
// command needs of the run rather than a signature this program shaped.
#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::trivially_copy_pass_by_ref
)]
// What a run does is a handful of independent answers, and making each one a two-variant enum
// would say no more than the bool does while writing every use twice.
#![allow(clippy::struct_excessive_bools)]

use std::fs;

use librorolala::layout::{Layout, LayoutPath};
use librorolala::status::tracked_vault;
use librorolala::storage::{Blake3Hash, Key, StorageSet};
use librorolala::vcs::VCSIndex;
use librorolala::workspace::Workspace;
use librorolala::write::path::local_path;
use librorolala::write::tweak::{
    self, Adjustment, Chain, Change, Found, Operation, Request as TweakRequest, Sheet, TargetMiss,
    TweakRefusal, TweakWarning,
};
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
use rorolala_utils_cli_theme::{err_line, help_line, trd, warn_line};
use rorolala_utils_constants::WORKSPACE_TWEAKING_PATH;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{offer, positional, strip_written, target_words, typing_flag};
use crate::editor::{ResEditor, open};
use crate::exit_codes::{EC_ERR_TWEAK, EC_ERR_UNMATCHED, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::keys::account_named;
use crate::layout::{ErrorLayoutShouldInWorkspace, chosen};
use crate::progress::Reporting;

/// The description to give the version.
const ARG_DESCRIPTION: PickerArg<'static, Option<String>> = arg![description: Option<String>, 'd'];

/// The creator to give the version.
const ARG_CREATOR: PickerArg<'static, Option<String>> = arg![creator: Option<String>, 'C'];

/// The content to point the version at.
const ARG_CONTENT: PickerArg<'static, Option<String>> = arg![content: Option<String>, 'c'];

/// The variant to join the version with, or nothing to unjoin it.
const ARG_JOIN: PickerArg<'static, Option<String>> = arg![join: Option<String>, 'J'];

/// Insert a copy of the target version above it.
const ARG_INSERT: PickerArg<'static, Flag> = arg![insert: Flag];

/// Remove the version the value names.
const ARG_DROP: PickerArg<'static, Option<String>> = arg![drop: Option<String>];

/// Adjust the whole range in an editor and apply the result at once.
const ARG_INTERACT: PickerArg<'static, Flag> = arg![interact: Flag, 'i'];

/// Apply every pending adjustment at once.
const ARG_NOW: PickerArg<'static, Flag> = arg![now: Flag];

/// Drop one pending adjustment, by the number the list gives it.
const ARG_CANCEL: PickerArg<'static, Option<String>> = arg![cancel: Option<String>];

/// The file, which is a path of the Layout being worked in.
const ARG_FILE: PickerArg<'static, String> = arg![String];

/// The version to adjust, which is the version the entry records when it is not given.
const ARG_TARGET: PickerArg<'static, Option<String>> = arg![Option<String>];

#[help(buffer)]
pub fn help_tweak(_: EntryTweak, ec: &mut ResExitCode) {
    eprintln!("{}", trd!(t!("tweak.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTweak)]
pub fn desc_tweak() -> Description {
    t!("tweak.description").to_string().into()
}

/// Completes what `rola tweak` can be given next.
///
/// `FILE` is a local path of the Layout being worked in, so the filesystem answers it. `TARGET`
/// names a version, which is what the index holds. A step count and text are not offered, since
/// nothing here can say what a run means to name.
#[completion(EntryTweak)]
pub fn complete_tweak(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_DESCRIPTION: t!("tweak.complete.description"),
                ARG_CREATOR: t!("tweak.complete.creator"),
                ARG_CONTENT: t!("tweak.complete.content"),
                ARG_JOIN: t!("tweak.complete.join"),
                ARG_INSERT: t!("tweak.complete.insert"),
                ARG_DROP: t!("tweak.complete.drop"),
                ARG_INTERACT: t!("tweak.complete.interact"),
                ARG_NOW: t!("tweak.complete.now"),
                ARG_CANCEL: t!("tweak.complete.cancel"),
            },
        );
    }

    match positional(&ctx, "tweak") {
        0 => Suggest::file_comp(),
        1 => offer(
            &ctx,
            target_words(
                workspace.get_ref(),
                vault.get_ref(),
                index.get_ref().as_ref(),
            ),
        ),
        _ => suggest!(),
    }
}

/// Adjusts a file's chain, rebuilding every version after the point adjusted
///
/// `FILE` is one local path in this Workspace (an entry, a hash and a remote name are refused) and
/// it has to be a path the Layout records. `TARGET` names the version to adjust, and is the version
/// the entry records when it is not given: `~N` N versions back, a version's hash or the head of
/// one, the hash of the variant that produced a version, or text that names one creator or one
/// message. A target that names more than one version is listed rather than guessed at.
///
/// An adjustment is one of these, and only one may be given at a time: any of the four fields
/// together on the same target, `--insert`, which adds a copy of the target above it, or `--drop`,
/// which names the version to remove.
///
/// An adjustment is written into the plan the Workspace keeps for the file and the version it
/// records, and what the plan holds is listed. `--now` applies the whole plan, `--cancel` drops one
/// adjustment by the number the list gives it, and `--interact` writes the whole range into
/// `./.rola/TWEAKING.md`, opens the editor, and applies what comes back.
///
/// Applying a plan writes the adjusted version and replays every version above it, points the
/// Layout at the replayed tail, and leaves the old chain in the index. The file is brought up to
/// the version the entry records when it stood as recorded, and left alone when it held changes.
///
/// # Errors
///
/// Renders [`ErrorTweakNoFile`] when no file is named, [`ErrorTweakFile`] when the word names no
/// path of this Workspace, [`ErrorTweakNotRecorded`] when the Layout records no version for it,
/// [`ErrorTweakNoChain`] when what it records is not the top of a chain, [`ErrorTweakMerging`] when
/// a variant is waiting to be joined into it, [`ErrorTweakTarget`] when the target names no version
/// or more than one, [`ErrorTweakFlags`] when the flags describe more than one adjustment,
/// [`ErrorTweakInsertTarget`] when `--insert` has no target, [`ErrorTweakInteract`] when
/// `--interact` is given with another adjustment, [`ErrorTweakNoPlan`] when there is nothing to
/// apply or to cancel, [`ErrorTweakCancel`] when the number names no pending adjustment,
/// [`ErrorTweakSheet`] when the edited sheet cannot be read, [`ErrorLayoutShouldInWorkspace`] when
/// the run is not inside a Workspace, and [`ErrorTweakFailed`] when the store, the index or the
/// Layout refuses.
#[command(node = "tweak", entry = EntryTweak)]
pub fn tweak(args: EntryTweak) -> Next {
    let picked = args
        .pick(ARG_DESCRIPTION)
        .pick(ARG_CREATOR)
        .pick(ARG_CONTENT)
        .pick(ARG_JOIN)
        .pick(ARG_INSERT)
        .pick(ARG_DROP)
        .pick(ARG_INTERACT)
        .pick(ARG_NOW)
        .pick(ARG_CANCEL)
        .pick_or_route(ARG_FILE, || ErrorTweakNoFile.to_chain())
        .pick(ARG_TARGET)
        .pick(&UNMATCHED)
        .to_result();
    let (
        description,
        creator,
        content,
        join,
        insert,
        drop,
        interact,
        now,
        cancel,
        file,
        target,
        unmatched,
    ) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tweak") {
        return next;
    }

    let insert = matches!(insert, Flag::Active);
    let interact = matches!(interact, Flag::Active);
    let now = matches!(now, Flag::Active);

    let adjustment = Adjustment {
        creator,
        message: description,
        content,
        join: join.map(|join| (!join.is_empty()).then_some(join)),
    };

    // What a run may say at once is one adjustment, or one of the ways of running the plan a file
    // already has. Each pair below is a run that says two things, and the two are refused rather
    // than one of them winning quietly.
    let named = u8::from(!adjustment.is_empty()) + u8::from(insert) + u8::from(drop.is_some());
    if named > 1 || (cancel.is_some() && (now || named > 0)) {
        return ErrorTweakFlags { file }.into();
    }
    if interact && (named > 0 || now || cancel.is_some()) {
        return ErrorTweakInteract { file }.into();
    }
    if insert && target.is_none() {
        return ErrorTweakInsertTarget.into();
    }
    if drop.is_some() && target.is_some() {
        return ErrorTweakFlags { file }.into();
    }

    StateTweak {
        file,
        target,
        adjustment,
        insert,
        drop,
        interact,
        now,
        cancel,
    }
    .into()
}

/// The state a tweak starts in.
#[derive(Grouped)]
pub struct StateTweak {
    /// The file named, as it was written.
    file: String,
    /// The version to adjust, when one was named.
    target: Option<String>,
    /// The fields the run sets.
    adjustment: Adjustment,
    /// Whether a copy of the version is added above it.
    insert: bool,
    /// The version to remove, when one was named.
    drop: Option<String>,
    /// Whether the range is edited in an editor and applied at once.
    interact: bool,
    /// Whether the plan is applied at once.
    now: bool,
    /// The pending adjustment to drop, as it was written.
    cancel: Option<String>,
}

#[chain(routeify)]
pub fn handle_tweak(
    state: StateTweak,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResStorageSet>,
    index: &mut LazyRes<ResVCSIndex>,
    current: &mut LazyRes<ResCurrentAccount>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    editor: &ResEditor,
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
    let root = held.get_root().to_path_buf();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return failed(&state.file, error.to_string()),
    };
    let Ok(path) = local_path(&root, &cwd, &state.file) else {
        return ErrorTweakFile { file: state.file }.into();
    };

    let Some(index) = index.get_ref().as_ref() else {
        return failed(&state.file, t!("tweak.err_no_index").trim().to_owned());
    };
    let chain = match tweak::chain_of(index, &layout, &path) {
        Ok(chain) => chain,
        Err(TweakRefusal::NotRecorded | TweakRefusal::Nothing) => {
            return ErrorTweakNotRecorded { file: state.file }.into();
        }
        Err(TweakRefusal::NotAChain) => return ErrorTweakNoChain { file: state.file }.into(),
        Err(refusal) => return refused(&state.file, refusal),
    };

    let Some(id) = layout.id_of(&path) else {
        return ErrorTweakNotRecorded { file: state.file }.into();
    };
    let Some(data) = layout.entry(id) else {
        return ErrorTweakNotRecorded { file: state.file }.into();
    };
    let local = data.version();

    let plan = match tweak::read_plan(&root, id, &local) {
        Ok(Some(sheet)) if !sheet.is_empty() => Some(sheet),
        Ok(_) => None,
        Err(refusal) => return refused(&state.file, refusal),
    };

    // Cancelling is a run of its own, so it is answered before a target is asked for: a run that
    // cancels the first pending adjustment said what it meant without naming a version.
    if let Some(written) = state.cancel.clone() {
        let Some(plan) = plan else {
            return ErrorTweakNoPlan { file: state.file }.into();
        };

        return drop_pending(&state.file, &root, id, &local, &chain, &plan, &written);
    }

    let target = match tweak::resolve(&chain, state.target.as_deref()) {
        Ok(number) => number,
        Err(miss) => {
            if state.interact {
                earliest(&miss)
            } else {
                let asked = !state.adjustment.is_empty() || state.insert || state.drop.is_some();
                if let TargetMiss::Ambiguous { candidates, .. } = &miss
                    && !asked
                {
                    return say_ambiguous(&state.file, &chain, candidates);
                }

                return target_miss(&state.file, &chain, miss);
            }
        }
    };

    // An editor is opened on the range from the earliest version named to the one recorded, and
    // on the whole history when the run named no version at all.
    if state.interact {
        let anchor = if state.target.is_none() { 0 } else { target };

        return edit(&state, &root, &chain, anchor, editor, |sheet| {
            applied(
                &state,
                held,
                &layout,
                index,
                storage.get_ref().storage(),
                current.get_ref(),
                remote.get_ref(),
                **offline,
                **concurrency,
                progress,
                &path,
                id,
                sheet,
            )
        });
    }

    let request = if state.insert {
        Some((target, Change::Insert))
    } else if let Some(written) = state.drop.as_deref() {
        match tweak::resolve(&chain, Some(written)) {
            Ok(number) => Some((number, Change::Drop)),
            Err(miss) => return target_miss(&state.file, &chain, miss),
        }
    } else if state.adjustment.is_empty() {
        None
    } else {
        Some((target, Change::Fields(state.adjustment.clone())))
    };

    if let Some((target, change)) = request {
        let sheet = match tweak::adjust(&chain, plan.as_ref(), target, &change) {
            Ok(sheet) => sheet,
            Err(refusal) => return refused(&state.file, refusal),
        };

        if sheet.is_empty() {
            if let Err(refusal) = tweak::remove_plan(&root, id, &local) {
                return refused(&state.file, refusal);
            }
        } else if let Err(refusal) =
            tweak::write_plan(&root, id, &local, &sheet, &chain, &[&plan_head(id, &local)])
        {
            return refused(&state.file, refusal);
        }

        let listed = match tweak::operations(&chain, &sheet) {
            Ok(operations) => operations,
            Err(refusal) => return refused(&state.file, refusal),
        };
        if !listed.is_empty() {
            say_pending(&listed);
        }

        if !state.now {
            return ().into();
        }
        if sheet.is_empty() {
            return ErrorTweakNoPlan { file: state.file }.into();
        }

        return applied(
            &state,
            held,
            &layout,
            index,
            storage.get_ref().storage(),
            current.get_ref(),
            remote.get_ref(),
            **offline,
            **concurrency,
            progress,
            &path,
            id,
            &sheet,
        );
    }

    // `--now` on its own applies what the file already has waiting, which is the run the plan's
    // own help points at.
    if state.now {
        let Some(plan) = plan else {
            return ErrorTweakNoPlan { file: state.file }.into();
        };

        return applied(
            &state,
            held,
            &layout,
            index,
            storage.get_ref().storage(),
            current.get_ref(),
            remote.get_ref(),
            **offline,
            **concurrency,
            progress,
            &path,
            id,
            &plan,
        );
    }

    // Nothing was said about what to adjust, so the run is a question: what the target names, and
    // what is still waiting for the file.
    let found = match chain.found(target) {
        Ok(found) => found,
        Err(refusal) => return refused(&state.file, refusal),
    };
    println!(
        "{}",
        trd!(t!(
            "tweak.query_ready",
            file = state.file,
            version = &found.version
        ))
        .trim()
    );
    println!();
    println!("{}", row_text(&found));
    println!();
    println!("{}", help_line!(t!("tweak.query_help").trim()));

    if let Some(plan) = plan {
        let listed = match tweak::operations(&chain, &plan) {
            Ok(operations) => operations,
            Err(refusal) => return refused(&state.file, refusal),
        };
        if !listed.is_empty() {
            println!();
            say_pending(&listed);
        }
    }

    ().into()
}

/// Applies `sheet` to the chain the path's entry records, and says what came of it.
fn applied(
    state: &StateTweak,
    workspace: &Workspace,
    layout: &Layout,
    index: &VCSIndex,
    store: Option<&StorageSet>,
    current: &ResCurrentAccount,
    remote: &ResCurrentRemoteVault,
    offline: bool,
    concurrency: usize,
    progress: &ResProgressSetting,
    path: &LayoutPath,
    id: Uuid,
    sheet: &Sheet,
) -> Next {
    let tracked = tracked_vault(workspace);
    let reach = tracked.as_deref().and_then(|tracked| {
        let name = remote.name_or_default(tracked).ok()?;
        let address = remote.vault_or_default(name.clone()).ok()?;

        Some((name, address))
    });
    let candidates = fetch::candidates(remote, reach);
    let account = current
        .must_bind()
        .ok()
        .and_then(|name| account_named(&name, Some(workspace), None).ok());

    let request = TweakRequest {
        workspace,
        layout,
        index: Some(index),
        store,
        account: account.as_ref(),
        candidates: &candidates,
        offline,
        concurrency,
    };

    let reporting = Reporting::start(*progress);
    let cancellation = crate::cancel::on_ctrl_c();
    let outcome = tweak::apply(
        &request,
        id,
        path,
        sheet,
        &reporting.progress(),
        cancellation,
    );
    reporting.finish();

    match outcome {
        Ok(outcome) => {
            println!(
                "{}",
                trd!(t!(
                    "tweak.applied",
                    file = state.file,
                    from = outcome.from,
                    to = outcome.to,
                    count = outcome.rebuilt
                ))
                .trim()
            );
            for warning in outcome.warnings {
                match warning {
                    TweakWarning::ContentUnavailable => {
                        eprintln!("{}", warn_line!(t!("tweak.warn_content").trim()));
                        eprintln!("{}", help_line!(t!("tweak.warn_content_help").trim()));
                    }
                }
            }

            ().into()
        }
        Err(refusal) => refused(&state.file, refusal),
    }
}

/// Writes the range into the editor, and applies what comes back.
fn edit<F>(
    state: &StateTweak,
    root: &std::path::Path,
    chain: &Chain,
    anchor: usize,
    editor: &ResEditor,
    apply: F,
) -> Next
where
    F: FnOnce(&Sheet) -> Next,
{
    let sheet = match Sheet::plain(chain, anchor) {
        Ok(sheet) => sheet,
        Err(refusal) => return refused(&state.file, refusal),
    };
    let head = plan_head_text(state, chain);
    let comments: Vec<&str> = head.iter().map(String::as_str).collect();
    let text = match sheet.render(chain, &comments) {
        Ok(text) => text,
        Err(refusal) => return refused(&state.file, refusal),
    };

    let path: std::path::PathBuf = root.join(WORKSPACE_TWEAKING_PATH).components().collect();
    if let Err(error) = fs::write(&path, text) {
        return failed(&state.file, error.to_string());
    }

    let editor = match editor.must_bind() {
        Ok(editor) => editor,
        Err(error) => return error.into(),
    };
    if let Err(error) = open(editor, &path) {
        return error.into();
    }

    let edited = match fs::read_to_string(&path) {
        Ok(edited) => edited,
        Err(error) => return failed(&state.file, error.to_string()),
    };
    let sheet = match Sheet::parse(&edited) {
        Ok(sheet) => sheet,
        Err(error) => {
            return ErrorTweakSheet {
                file: state.file.clone(),
                reason: error.to_string(),
            }
            .into();
        }
    };

    // What was edited is read against the chain before anything is written, so a sheet that does
    // not fit it is told rather than applied to some other range. A sheet that says nothing is an
    // editor closed without an edit, which is a run with nothing to write.
    let listed = match tweak::operations(chain, &sheet) {
        Ok(listed) => listed,
        Err(refusal) => return refused(&state.file, refusal),
    };
    if listed.is_empty() {
        return ().into();
    }

    apply(&sheet)
}

/// The head the editor opens on.
fn plan_head_text(state: &StateTweak, chain: &Chain) -> Vec<String> {
    let Some(top) = chain.top().ok().and_then(|top| chain.found(top).ok()) else {
        return Vec::new();
    };

    vec![
        t!(
            "tweak.edit_header",
            file = state.file,
            version = top.version,
            local = &top.version[..top.version.len().min(12)]
        )
        .trim()
        .to_owned(),
        t!("tweak.edit_columns").trim().to_owned(),
        t!("tweak.edit_markers").trim().to_owned(),
    ]
}

/// The one comment a plan file opens with.
fn plan_head(id: Uuid, local: &Blake3Hash) -> String {
    t!("tweak.plan_header", uuid = id, local = hex(local))
        .trim()
        .to_owned()
}

/// Drops the pending adjustment `written` names, and says what is left.
fn drop_pending(
    file: &str,
    root: &std::path::Path,
    id: Uuid,
    local: &Blake3Hash,
    chain: &Chain,
    plan: &Sheet,
    written: &str,
) -> Next {
    let number: usize = written.parse().unwrap_or(0);
    let (sheet, operation) = match tweak::cancel(chain, plan, number) {
        Ok(cancelled) => cancelled,
        Err(TweakRefusal::Target(TargetMiss::NoMatch { .. })) => {
            let count = tweak::operations(chain, plan).map_or(0, |operations| operations.len());

            return ErrorTweakCancel {
                number: written.to_owned(),
                count,
            }
            .into();
        }
        Err(refusal) => return refused(file, refusal),
    };

    if sheet.is_empty() {
        if let Err(refusal) = tweak::remove_plan(root, id, local) {
            return refused(file, refusal);
        }
    } else if let Err(refusal) =
        tweak::write_plan(root, id, local, &sheet, chain, &[&plan_head(id, local)])
    {
        return refused(file, refusal);
    }

    println!(
        "{}",
        trd!(t!(
            "tweak.cancelled",
            operation = operation_text(&operation)
        ))
        .trim()
    );

    let listed = match tweak::operations(chain, &sheet) {
        Ok(operations) => operations,
        Err(refusal) => return refused(file, refusal),
    };
    if !listed.is_empty() {
        println!();
        say_pending(&listed);
    }

    ().into()
}

/// Draws the list of what a plan would do.
fn say_pending(operations: &[Operation]) {
    println!("{}", trd!(t!("tweak.pending_header")).trim());
    for (at, operation) in operations.iter().enumerate() {
        println!(
            "{}",
            trd!(t!(
                "tweak.pending_item",
                number = at + 1,
                operation = operation_text(operation)
            ))
            .trim()
        );
    }
    println!();
    println!("{}", help_line!(t!("tweak.pending_help").trim()));
}

/// The words one adjustment is said in.
fn operation_text(operation: &Operation) -> String {
    match operation {
        Operation::Creator { version, text } => {
            t!("tweak.op_creator", version = version, text = text)
                .trim()
                .to_owned()
        }
        Operation::Message { version, text } => {
            t!("tweak.op_message", version = version, text = text)
                .trim()
                .to_owned()
        }
        Operation::Content { version, hash } => {
            t!("tweak.op_content", version = version, hash = hash)
                .trim()
                .to_owned()
        }
        Operation::Join { version, variant } => {
            t!("tweak.op_join", version = version, variant = variant)
                .trim()
                .to_owned()
        }
        Operation::Unjoin { version, variant } => {
            t!("tweak.op_unjoin", version = version, variant = variant)
                .trim()
                .to_owned()
        }
        Operation::Insert { below, above } => t!("tweak.op_insert", below = below, above = above)
            .trim()
            .to_owned(),
        Operation::Remove { version } => t!("tweak.op_remove", version = version).trim().to_owned(),
    }
}

/// Draws the versions an ambiguous target names, for a run that asked what it names.
fn say_ambiguous(file: &str, chain: &Chain, candidates: &[usize]) -> Next {
    println!("{}", trd!(t!("tweak.query_ambiguous", file = file)).trim());
    println!();
    for number in candidates {
        if let Ok(found) = chain.found(*number) {
            println!("{}", row_text(&found));
        }
    }
    println!();
    println!("{}", help_line!(t!("tweak.query_help").trim()));

    ().into()
}

/// One version drawn as a listing draws it.
fn row_text(found: &Found) -> String {
    trd!(t!(
        "tweak.query_row",
        variant = head(&found.variant),
        creator = found.creator,
        message = found.message,
        number = found.number
    ))
    .trim()
    .to_owned()
}

/// The head of a hash, as a listing writes it.
fn head(hash: &str) -> &str {
    &hash[..hash.len().min(12)]
}

/// The failure a target naming no version is told as.
fn target_miss(file: &str, chain: &Chain, miss: TargetMiss) -> Next {
    match miss {
        TargetMiss::NoMatch { target } | TargetMiss::OutOfChain { target } => ErrorTweakTarget {
            file: file.to_owned(),
            target,
            kind: TargetKind::None,
        }
        .into(),
        TargetMiss::Elsewhere { target } => ErrorTweakTarget {
            file: file.to_owned(),
            target,
            kind: TargetKind::Other,
        }
        .into(),
        TargetMiss::Ambiguous { target, candidates } => ErrorTweakTarget {
            file: file.to_owned(),
            target,
            kind: TargetKind::Many(
                candidates
                    .into_iter()
                    .filter_map(|number| chain.found(number).ok())
                    .map(|found| Candidate {
                        variant: head(&found.variant).to_owned(),
                        creator: found.creator,
                        message: found.message,
                        number: found.number,
                    })
                    .collect(),
            ),
        }
        .into(),
        TargetMiss::Removed { .. } => ErrorTweakFlags {
            file: file.to_owned(),
        }
        .into(),
    }
}

/// The earliest version an ambiguous target names, which is what an editor is opened on.
fn earliest(miss: &TargetMiss) -> usize {
    match miss {
        TargetMiss::Ambiguous { candidates, .. } => {
            candidates.iter().copied().min().unwrap_or_default()
        }
        _ => 0,
    }
}

/// The failure a library refusal is told as.
fn refused(file: &str, refusal: TweakRefusal) -> Next {
    match refusal {
        TweakRefusal::NotRecorded | TweakRefusal::Nothing => ErrorTweakNotRecorded {
            file: file.to_owned(),
        }
        .into(),
        // A tree is not a file of the work in the sense this command takes one, and a path of the
        // work is what it says it wants.
        TweakRefusal::Tree => ErrorTweakFile {
            file: file.to_owned(),
        }
        .into(),
        TweakRefusal::NotAChain => ErrorTweakNoChain {
            file: file.to_owned(),
        }
        .into(),
        TweakRefusal::Merging { variant } => ErrorTweakMerging {
            file: file.to_owned(),
            variant,
        }
        .into(),
        TweakRefusal::Target(miss) => match miss {
            TargetMiss::NoMatch { target } | TargetMiss::OutOfChain { target } => {
                ErrorTweakTarget {
                    file: file.to_owned(),
                    target,
                    kind: TargetKind::None,
                }
                .into()
            }
            TargetMiss::Elsewhere { target } => ErrorTweakTarget {
                file: file.to_owned(),
                target,
                kind: TargetKind::Other,
            }
            .into(),
            TargetMiss::Ambiguous { target, .. } => ErrorTweakTarget {
                file: file.to_owned(),
                target,
                kind: TargetKind::Many(Vec::new()),
            }
            .into(),
            TargetMiss::Removed { .. } => ErrorTweakFlags {
                file: file.to_owned(),
            }
            .into(),
        },
        TweakRefusal::Sheet { reason } => ErrorTweakSheet {
            file: file.to_owned(),
            reason,
        }
        .into(),
        TweakRefusal::Failed(error) => failed(file, error.to_string()),
    }
}

/// The failure a store, an index or a Layout refusing is told as.
fn failed(file: &str, cause: String) -> Next {
    ErrorTweakFailed {
        file: file.to_owned(),
        cause,
    }
    .into()
}

/// A digest written out in hex.
fn hex(digest: &Blake3Hash) -> String {
    Key::new(*digest).hex()
}

/// Error: no file was named.
#[derive(Grouped)]
pub struct ErrorTweakNoFile;

impl Failure for ErrorTweakNoFile {
    fn name(&self) -> &'static str {
        "error_tweak_no_file"
    }

    fn reason(&self) -> String {
        t!("tweak.err_no_file").trim().to_string()
    }
}

failure!(ErrorTweakNoFile);

#[renderer(buffer)]
pub fn render_error_tweak_no_file(_: ErrorTweakNoFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tweak.err_no_file").trim()));
    r_eprintln!("{}", help_line!(t!("tweak.err_no_file_help").trim()));
    ec.exit_code = EC_ERR_UNMATCHED;
}

/// Error: the word names no path of this Workspace.
#[derive(Grouped)]
pub struct ErrorTweakFile {
    /// The word as it was written.
    file: String,
}

impl Failure for ErrorTweakFile {
    fn name(&self) -> &'static str {
        "error_tweak_file"
    }

    fn reason(&self) -> String {
        t!("tweak.err_not_local", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorTweakFile);

#[renderer(buffer)]
pub fn render_error_tweak_file(error: ErrorTweakFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_not_local_help").trim()));
    ec.exit_code = EC_ERR_UNMATCHED;
}

/// Error: the Layout records no version for the file.
#[derive(Grouped)]
pub struct ErrorTweakNotRecorded {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorTweakNotRecorded {
    fn name(&self) -> &'static str {
        "error_tweak_not_recorded"
    }

    fn reason(&self) -> String {
        t!("tweak.err_not_recorded", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorTweakNotRecorded);

#[renderer(buffer)]
pub fn render_error_tweak_not_recorded(error: ErrorTweakNotRecorded, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_not_recorded_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: what the file records is not the top of a chain.
#[derive(Grouped)]
pub struct ErrorTweakNoChain {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorTweakNoChain {
    fn name(&self) -> &'static str {
        "error_tweak_no_chain"
    }

    fn reason(&self) -> String {
        t!("tweak.err_no_chain", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorTweakNoChain);

#[renderer(buffer)]
pub fn render_error_tweak_no_chain(error: ErrorTweakNoChain, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_no_chain_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a variant is waiting to be joined into the file.
#[derive(Grouped)]
pub struct ErrorTweakMerging {
    /// The file as it was written.
    file: String,
    /// The variant that is waiting, as hex.
    variant: String,
}

impl Failure for ErrorTweakMerging {
    fn name(&self) -> &'static str {
        "error_tweak_merging"
    }

    fn reason(&self) -> String {
        t!(
            "tweak.err_merging",
            file = self.file,
            variant = self.variant
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorTweakMerging);

#[renderer(buffer)]
pub fn render_error_tweak_merging(error: ErrorTweakMerging, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_merging_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// One version a target matched, as the failure names it.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The variant's hash, as hex.
    variant: String,
    /// Who made it, as text.
    creator: String,
    /// What it says, as text.
    message: String,
    /// Its number in the chain.
    number: usize,
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

/// Error: the target names no one version.
#[derive(Grouped)]
pub struct ErrorTweakTarget {
    /// The file as it was written.
    file: String,
    /// The target as it was written.
    target: String,
    /// Why it named no one version.
    kind: TargetKind,
}

impl Failure for ErrorTweakTarget {
    fn name(&self) -> &'static str {
        "error_tweak_target"
    }

    fn reason(&self) -> String {
        match &self.kind {
            TargetKind::Many(candidates) => t!(
                "tweak.err_target_many",
                file = self.file,
                target = self.target,
                count = candidates.len()
            )
            .trim()
            .to_string(),
            TargetKind::Other => t!("tweak.err_target_other", target = self.target)
                .trim()
                .to_string(),
            TargetKind::None => t!(
                "tweak.err_target_none",
                file = self.file,
                target = self.target
            )
            .trim()
            .to_string(),
        }
    }
}

failure!(ErrorTweakTarget);

#[renderer(buffer)]
pub fn render_error_tweak_target(error: ErrorTweakTarget, ec: &mut ResExitCode) {
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
            r_eprintln!("{}", help_line!(t!("tweak.err_target_many_help").trim()));
        }
        TargetKind::Other => {
            r_eprintln!(
                "{}",
                help_line!(t!("tweak.err_target_other_help", file = error.file).trim())
            );
        }
        TargetKind::None => {
            r_eprintln!("{}", help_line!(t!("tweak.err_target_none_help").trim()));
        }
    }
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the flags describe more than one adjustment.
#[derive(Grouped)]
pub struct ErrorTweakFlags {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorTweakFlags {
    fn name(&self) -> &'static str {
        "error_tweak_flags"
    }

    fn reason(&self) -> String {
        t!("tweak.err_flags", file = self.file).trim().to_string()
    }
}

failure!(ErrorTweakFlags);

#[renderer(buffer)]
pub fn render_error_tweak_flags(error: ErrorTweakFlags, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_flags_help").trim()));
    ec.exit_code = EC_ERR_UNMATCHED;
}

/// Error: `--insert` has no target to insert above.
#[derive(Grouped)]
pub struct ErrorTweakInsertTarget;

impl Failure for ErrorTweakInsertTarget {
    fn name(&self) -> &'static str {
        "error_tweak_insert_target"
    }

    fn reason(&self) -> String {
        t!("tweak.err_insert_target").trim().to_string()
    }
}

failure!(ErrorTweakInsertTarget);

#[renderer(buffer)]
pub fn render_error_tweak_insert_target(_: ErrorTweakInsertTarget, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tweak.err_insert_target").trim()));
    r_eprintln!("{}", help_line!(t!("tweak.err_insert_target_help").trim()));
    ec.exit_code = EC_ERR_UNMATCHED;
}

/// Error: `--interact` was given with another adjustment.
#[derive(Grouped)]
pub struct ErrorTweakInteract {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorTweakInteract {
    fn name(&self) -> &'static str {
        "error_tweak_interact"
    }

    fn reason(&self) -> String {
        t!("tweak.err_interact", file = self.file)
            .trim()
            .to_string()
    }
}

failure!(ErrorTweakInteract);

#[renderer(buffer)]
pub fn render_error_tweak_interact(error: ErrorTweakInteract, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_interact_help").trim()));
    ec.exit_code = EC_ERR_UNMATCHED;
}

/// Error: the file has nothing waiting and nothing was said.
#[derive(Grouped)]
pub struct ErrorTweakNoPlan {
    /// The file as it was written.
    file: String,
}

impl Failure for ErrorTweakNoPlan {
    fn name(&self) -> &'static str {
        "error_tweak_no_plan"
    }

    fn reason(&self) -> String {
        t!("tweak.err_no_plan", file = self.file).trim().to_string()
    }
}

failure!(ErrorTweakNoPlan);

#[renderer(buffer)]
pub fn render_error_tweak_no_plan(error: ErrorTweakNoPlan, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_no_plan_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the number names no pending adjustment.
#[derive(Grouped)]
pub struct ErrorTweakCancel {
    /// The number as it was written.
    number: String,
    /// How many adjustments the plan holds.
    count: usize,
}

impl Failure for ErrorTweakCancel {
    fn name(&self) -> &'static str {
        "error_tweak_cancel"
    }

    fn reason(&self) -> String {
        t!(
            "tweak.err_cancel_range",
            number = self.number,
            count = self.count
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorTweakCancel);

#[renderer(buffer)]
pub fn render_error_tweak_cancel(error: ErrorTweakCancel, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_cancel_range_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the edited sheet cannot be read.
#[derive(Grouped)]
pub struct ErrorTweakSheet {
    /// The file being adjusted.
    file: String,
    /// Why the sheet cannot be read.
    reason: String,
}

impl Failure for ErrorTweakSheet {
    fn name(&self) -> &'static str {
        "error_tweak_sheet"
    }

    fn reason(&self) -> String {
        t!("tweak.err_sheet", file = self.file, reason = self.reason)
            .trim()
            .to_string()
    }
}

failure!(ErrorTweakSheet);

#[renderer(buffer)]
pub fn render_error_tweak_sheet(error: ErrorTweakSheet, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tweak.err_sheet_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the store, the index or the Layout refused.
#[derive(Grouped)]
pub struct ErrorTweakFailed {
    /// The file being adjusted.
    file: String,
    /// What refused, as the library says it.
    cause: String,
}

impl Failure for ErrorTweakFailed {
    fn name(&self) -> &'static str {
        "error_tweak_failed"
    }

    fn reason(&self) -> String {
        t!("tweak.err_failed", file = self.file, cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTweakFailed);

#[renderer(buffer)]
pub fn render_error_tweak_failed(error: ErrorTweakFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_TWEAK;
}
