//! The `rola bring` command: bringing a `Uuid` the Vault holds into the Layout being worked in.
//!
//! A `Uuid` enters a Layout by being tracked and enters the Vault's Layout by going up, which
//! leaves what only the Vault holds out of reach: a sync names it rather than bringing it, since a
//! `Uuid` is not something a sync may invent a local path for. This is how one is asked for by
//! name: the operands read the way `mv`'s do (what is to come in first, where it is to land last)
//! so a reference the Vault's Layout names is put at a path here, under a directory here, or at the
//! path the Vault itself names it by when no destination was written.
//!
//! The variant merge `--join` asks for is the library's, see [`BringVariantRequest::variant`]: what
//! stays here is the operands, the run's resources and the words the answer is said in.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates,
// so a lint allowed on the handler below is reported as defined twice. The allow lives here,
// where it covers the one signature that needs it.
//
// The handler's parameters are the resources the framework injects, so how many of them there are
// is what the command needs of the run rather than a signature this program shaped. It is long
// because it reads every reference, the Variant mode included, before anything is written.
#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use librorolala::daemon::{action_fetch_layout, action_sync_index_all_async};
use librorolala::fetch::Sources;
use librorolala::protocol::ActionError;
use librorolala::workspace::Workspace;
use librorolala::write::bring::{
    BringVariantRequest, HashMiss as VariantHashMiss, Messages, Refusal, VariantRefusal, bring_in,
    resolve,
};
use librorolala::write::request::{Common, Target};
use librorolala::write::{Labels, WriteError};
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResConcurrency, ResCurrentRemoteVault, ResForce, ResOffline, ResProgressSetting,
    ResRorolalaStorage, ResVCSIndex, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd, warn_line};
use rorolala_utils_constants::DEFAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{
    IndexObject, filling_flag, flag_value, index_hashes, offer, positional, strip_written,
    typing_flag,
};
use crate::exit_codes::{EC_ERR_BRING, EC_ERR_BRING_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::hash::HashMiss;
use crate::keys::account_named;
use crate::layout::{ErrorLayoutNoCurrent, cached_vault_copy, failed};
use crate::progress::Reporting;
use crate::vcs_index::{ErrorVcsIndexNoIndex, runtime as index_runtime};

/// The file a variant is to be joined into, for the mode that brings a variant in rather than
/// bringing a `Uuid` the Vault holds into this Layout.
const ARG_JOIN: PickerArg<'static, Option<String>> = arg![join: Option<String>];

/// Whether each reference keeps only its own name under the destination.
///
/// A directory destination is a prefix by default, so `Assets/hero.psd` lands at
/// `<TO>/Assets/hero.psd`, the path the Vault gives it, kept whole. A run that brings entries the
/// user chose elsewhere (a drop on a directory, a paste) means them to land where they were put
/// rather than where the Vault has them, so this makes the destination the parent and the
/// reference the name: `<TO>/hero.psd`, and a folder keeps the tree it is under its own name.
const ARG_BY_NAME: PickerArg<'static, Flag> = arg![by_name: Flag];

/// Merge a tree variant into its tree rather than putting it beside the work.
const ARG_TREE_MERGE: PickerArg<'static, Flag> = arg![tree_merge: Flag];

#[help(buffer)]
pub fn help_bring(_: EntryBring, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("bring.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryBring)]
pub fn desc_bring() -> Description {
    t!("bring.description").to_string().into()
}

/// Completes what `rola bring` can be given next.
///
/// Every word names where something is to land here, so the filesystem answers it. Which entry a
/// reference names is the Vault's to say, and a reference is read there rather than here, so there
/// is nothing of the Vault to list. The variant mode is the one thing only this run knows: its
/// first word is a variant the index holds, and the rest is a place like any other.
#[completion(EntryBring)]
pub fn complete_bring(ctx: ShellContext, index: &mut LazyRes<ResVCSIndex>) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_JOIN: t!("bring.complete.join"),
                ARG_BY_NAME: t!("bring.complete.by_name"),
                ARG_TREE_MERGE: t!("bring.complete.tree_merge"),
            },
        );
    }

    if filling_flag(&ctx, &ARG_JOIN) {
        return Suggest::file_comp();
    }

    if flag_value(&ctx, &ARG_JOIN).is_some() && positional(&ctx, "bring") == 0 {
        return offer(
            &ctx,
            index_hashes(index.get_ref().as_ref(), IndexObject::Variant),
        );
    }

    Suggest::file_comp()
}

/// Brings what the Vault holds but this Layout does not into it
///
/// The operands read the way `mv`'s do: every word but the last is a reference, a path in the
/// Vault's Layout, a glob over those paths, or a `Uuid`, and the last, when there is more than one
/// word, is where those references are to land here. A reference therefore always comes from the
/// Vault's side; nothing but the destination is read as a path of this run's.
///
/// A glob is matched against the paths the Vault's Layout names, so `Installers/*` names everything
/// directly under `Installers` there and `Installers/**` everything below it. An entry several
/// references name is brought in once.
///
/// With no destination each reference lands at the path the Vault's Layout names it by. A
/// destination that is an existing directory is a prefix: a reference that is a path keeps its whole
/// path under it, so `Assets/hero.psd` lands at `<TO>/Assets/hero.psd` and a folder brought in keeps
/// the structure it has in the Vault; a reference that is a glob is rebased on the directory it
/// selects from, so `Assets/*` lands its matches directly under `<TO>`. Any other destination names
/// the whole landing path, so it takes exactly one reference.
///
/// A reference that names a folder of the Vault's Layout, whether an entry stands at that path or
/// only things below it, brings everything that stands under it as well, at the paths they have
/// there: bringing a folder in is bringing in the tree it is.
///
/// What a reference names has to be something this Layout does not already hold, and every landing
/// has to be somewhere nothing is: a name this Layout already gives, a file already at the path, or
/// another reference of the same run landing there, is refused rather than written over. Every
/// landing is worked out before anything is written, so a run that is refused leaves nothing half
/// done.
///
/// The version the Vault holds is what comes with it, its content is taken out of the store and
/// put at the path, and what the Layout then says is that version and the holder the Vault names.
///
/// # Errors
///
/// Renders [`ErrorBringNoTrack`] when the Layout being worked in tracks no Vault,
/// [`ErrorBringArgument`] when the operands name nothing to bring in or a destination several
/// references cannot share, and [`ErrorBringUnknown`], [`ErrorBringKnown`],
/// [`ErrorBringUnnamed`] or [`ErrorBringTaken`] when one reference cannot be brought in.
#[command(node = "bring", entry = EntryBring)]
pub fn bring(args: EntryBring) -> Next {
    // Picking cannot fail: the positions are one list, and an absent one is the empty list.
    let picked = args
        .pick(arg![Vec<String>])
        .pick(ARG_JOIN)
        .pick(ARG_BY_NAME)
        .pick(ARG_TREE_MERGE)
        .pick(&UNMATCHED)
        .to_result();
    let (words, join, by_name, tree_merge, unmatched): (
        Vec<String>,
        Option<String>,
        Flag,
        Flag,
        _,
    ) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "bring") {
        return next;
    }

    let Some((refs, to)) = split(join.as_deref(), words) else {
        return ErrorBringArgument.into();
    };

    StateBring {
        refs,
        to,
        join,
        by_name: matches!(by_name, Flag::Active),
        tree_merge: matches!(tree_merge, Flag::Active),
    }
    .into()
}

/// The state a bring starts in.
#[derive(Grouped)]
pub struct StateBring {
    /// What to bring in: paths in the Vault's Layout and `Uuid`s, or the one variant a merge brings
    /// in.
    refs: Vec<String>,
    /// Where they are to land here, or nothing to land each at the path the Vault names it by, and
    /// for the variant mode, where its file is to lie.
    to: Option<String>,
    /// The file a variant is to be joined into, when this run brings one in.
    join: Option<String>,
    /// Whether each reference keeps only its own name under the destination.
    by_name: bool,
    /// Whether a tree variant is merged into its tree rather than put beside the work.
    tree_merge: bool,
}

#[chain(routeify)]
pub fn handle_bring(
    state: StateBring,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    progress: &ResProgressSetting,
    offline: &ResOffline,
    force: &ResForce,
    concurrency: &ResConcurrency,
) -> Next {
    let StateBring {
        refs,
        to,
        join,
        by_name,
        tree_merge,
    } = state;

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let layout = match held.layout_if_there() {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutNoCurrent.into(),
        Err(error) => return failed(&error),
    };

    // A variant is not a `Uuid` the Vault's Layout names, so the mode that brings one in reads its
    // own operands and asks for the Vault only when what it wants is not here: a merge of something
    // already synced is local work, and a Workspace that tracks no Vault can still do it.
    if let Some(target) = join {
        let variant = refs.first().map_or_default(String::as_str);

        return bring_variant(
            &target,
            variant,
            to.as_deref(),
            held,
            current.get_ref(),
            offline,
            **force,
            tree_merge,
            progress,
            concurrency,
        );
    }

    let track = match held.layout_track() {
        Ok(track) => track,
        Err(error) => return failed(&error),
    };
    let Some(track) = track else {
        return ErrorBringNoTrack.into();
    };

    let vault_name = remote.get_ref().name_or_default(track)?;
    let target = remote.get_ref().vault_or_default(vault_name.clone())?;

    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    // The content a bring carries is asked of whichever Vault holds it, and the one the Layout
    // tracks is asked first. What is read from that Vault is its Layout, a copy is still what the
    // references are read against, while the objects themselves may come from any Vault that has
    // them, since a key is the hash of what it names.
    let primary = fetch::primary(remote.get_ref(), Some(vault_name.as_str()));

    // What the Vault holds now is what the references are read against, so its copy is brought up
    // to date first, the same fetch `rola layout fetch` makes. Offline, the copy that is here is
    // the one read; a run with none is told so by the check below rather than by a fetch it may
    // not make.
    if !**offline {
        action_fetch_layout(held, &account, target.to_string(), vault_name.clone())?;
    }

    let remote_layout = cached_vault_copy(held, &vault_name)?;

    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let store = storage.get_ref().as_ref();

    let wanted = match resolve(
        &remote_layout,
        &layout,
        held.get_root(),
        &refs,
        to.as_deref(),
        by_name,
    ) {
        Ok(wanted) => wanted,
        Err(refusal) => return refused(refusal),
    };

    let runtime = match index_runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    // The index is what names the objects a bring reads, and it is the light half: it is moved
    // whole, while the content is asked for by what `bring_in` finds it needs. Both are the run's
    // progress: the index crosses first and every content after it, so a reader watching the
    // capsule sees the blocks arrive rather than a bar that stops at the index.
    let reporting = Reporting::start(*progress);

    if let Err(error) = pull(
        held,
        &account,
        &target.to_string(),
        &runtime,
        &reporting,
        **offline,
    ) {
        reporting.finish();

        return ErrorBringFailed {
            cause: error.to_string(),
        }
        .into();
    }

    // The candidates are the Vaults this run may ask, the one the Layout tracks first. The library
    // asks them for the content a bring needs; what it fetched is reported here, in this side's
    // words, because a count and a Vault name are things only the caller can say.
    let sources = match Sources::new(
        held,
        &account,
        fetch::candidates(remote.get_ref(), primary),
        **offline,
    ) {
        Ok(sources) => sources
            .reporting(reporting.progress())
            .with_concurrency(**concurrency),
        Err(cause) => {
            reporting.finish();

            return ErrorBringFailed { cause }.into();
        }
    };

    let messages = Messages {
        no_content: t!("bring.no_content").trim().to_owned(),
        tree_mark: t!("bring.err_tree_mark").trim().to_owned(),
    };
    let cancellation = crate::cancel::on_ctrl_c();

    let brought = match bring_in(
        held,
        &layout,
        &remote_layout,
        index,
        store,
        &runtime,
        wanted,
        &sources,
        &messages,
        cancellation,
    ) {
        Ok(brought) => brought,
        Err(error) => {
            reporting.finish();

            return ErrorBringFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    report_fetched(&sources);
    drop(sources);
    reporting.finish();

    ResultBroughtIn {
        layout: DEFAULT_LAYOUT_NAME.to_owned(),
        vault: vault_name,
        entries: brought
            .entries
            .into_iter()
            .map(|item| BringItem {
                uuid: item.uuid,
                remote_path: item.remote_path,
                local_path: item.local_path,
            })
            .collect(),
        failed: brought.failed,
    }
    .into()
}

/// Brings a variant in to be joined into a file this Layout names.
///
/// What comes in is not a path of the Vault's Layout but a variant the index holds, content that
/// already happened elsewhere and is to be joined into a file here without either side giving up.
/// The content is written to a **variant file** beside its target, under a name of the convention or
/// of the run's own, and what is waiting for what is written into the Workspace's merge records
/// rather than into the Layout: a merge is work in this tree alone, and the Layout is what the work
/// is.
///
/// The order the merge is made in is the library's, see [`BringVariantRequest::variant`]: what
/// stays here is reading the run's operands and its resources, and saying the answer in this
/// command's words.
fn bring_variant(
    target: &str,
    variant: &str,
    at: Option<&str>,
    held: &Workspace,
    current: &ResCurrentAccount,
    offline: &ResOffline,
    force: bool,
    tree_merge: bool,
    progress: &ResProgressSetting,
    concurrency: &ResConcurrency,
) -> Next {
    let request = BringVariantRequest::new(
        Common {
            workspace: held.get_root().to_path_buf(),
            account: current.name().unwrap_or_default().to_owned(),
            target: Target::Tracked,
            offline: **offline,
            force,
            concurrency: **concurrency,
            labels: Labels::default(),
        },
        variant.to_owned(),
        target.to_owned(),
        at.map(str::to_owned),
        tree_merge,
    );

    // The watcher is started before the merge, since what the merge may ask the Vaults for is
    // watched from the first connection rather than from the moment the asking is over.
    let reporting = Reporting::start(*progress);
    let brought = request.variant(&reporting.progress(), crate::cancel::on_ctrl_c());
    reporting.finish();

    match brought {
        Ok(report) => ResultBringJoin {
            target: report.target,
            variant: report.variant,
            path: report.path,
        }
        .into(),
        Err(refusal) => join_refused(target, refusal),
    }
}

/// The library's answer to a variant word that named no one variant, in the words this command has
/// for a hash.
///
/// The merge's own sentence is kept for a word that is no hash at all, and a head that names
/// nothing or several things is the same advice wherever it was typed, so it is said the shared
/// way.
impl From<VariantHashMiss> for HashMiss {
    fn from(miss: VariantHashMiss) -> Self {
        match miss {
            VariantHashMiss::Malformed => Self::Malformed,
            VariantHashMiss::Unknown => Self::Unknown,
            VariantHashMiss::Ambiguous(candidates) => Self::Ambiguous(candidates),
        }
    }
}

/// The failure a variant that could not be brought in is answered with.
///
/// Each refusal is said in the words this command has always used for it. An index the run does not
/// have keeps the failure and the exit code it has everywhere else in this command: the run stopped
/// before it read a variant at all, so there is no merge to word it as.
fn join_refused(target: &str, refusal: VariantRefusal) -> Next {
    let said = match refusal {
        VariantRefusal::NotAVariantHash { hash, miss } => {
            HashMiss::from(miss).reason(&hash, || t!("bring.err_join_hash").trim().to_string())
        }
        VariantRefusal::NotATarget => t!("bring.err_join_target").trim().to_string(),
        VariantRefusal::AlreadyMerging => t!("bring.err_join_merging").trim().to_string(),
        VariantRefusal::AlreadyBroughtIn => t!("bring.err_join_brought_in").trim().to_string(),
        VariantRefusal::UnknownVariant => t!("bring.err_join_unknown").trim().to_string(),
        VariantRefusal::OffChain => t!("bring.err_join_off_chain").trim().to_string(),
        VariantRefusal::NoStore => t!("bring.err_join_no_store").trim().to_string(),
        VariantRefusal::NoContent => t!("bring.err_join_no_content").trim().to_string(),
        VariantRefusal::PlaceOutside => t!("bring.err_join_place").trim().to_string(),
        VariantRefusal::PlaceTaken { path } => {
            t!("bring.err_join_taken", path = path).trim().to_string()
        }
        VariantRefusal::Chain => t!("bring.err_join_chain").trim().to_string(),
        VariantRefusal::NoTrack => t!("bring.err_join_no_track").trim().to_string(),
        VariantRefusal::NoAccount => t!("bring.err_join_no_account").trim().to_string(),
        VariantRefusal::NoVault => t!("bring.err_join_no_vault").trim().to_string(),
        VariantRefusal::Failed(WriteError::NoIndex) => return ErrorVcsIndexNoIndex.into(),
        VariantRefusal::Failed(error) => error.to_string(),
    };

    join_failed(target, said)
}

/// The failure a variant that could not be brought in is answered with.
fn join_failed(target: &str, cause: impl Into<String>) -> Next {
    ErrorBringJoin {
        target: target.to_owned(),
        cause: cause.into(),
    }
    .into()
}

/// Moves the index to what the Vault holds, which is what names the objects a bring reads.
///
/// Only the index: it is light, and it is what tells a version from its content. The content itself
/// is asked for one object at a time, by what [`bring_in`] finds it needs, see [`Sources`].
///
/// An offline run moves nothing: what the index already holds is what the versions are read from,
/// and one that is not here fails where it is read rather than being fetched.
fn pull(
    held: &librorolala::workspace::Workspace,
    account: &librorolala::auth::Account,
    target: &str,
    runtime: &tokio::runtime::Runtime,
    reporting: &Reporting,
    offline: bool,
) -> Result<(), ActionError> {
    if offline {
        return Ok(());
    }

    runtime.block_on(action_sync_index_all_async(
        held,
        account,
        target.to_owned(),
        String::new(),
        reporting.progress(),
    ))?;

    Ok(())
}

/// Says what the Vaults handed over, in one line, once the run is done with them.
///
/// The words are the command's: what a fetch came to is a count and the names of the Vaults it came
/// from, and the library keeps neither a language nor a place to print them.
fn report_fetched(sources: &Sources<'_>) {
    let took = sources.took();
    if took.is_empty() {
        return;
    }

    let count: usize = took.iter().map(|(_, count)| count).sum();
    let vaults: Vec<&str> = took.iter().map(|(name, _)| name.as_str()).collect();

    eprintln!(
        "{}",
        warn_line!(t!("fetch.brought", count = count, vaults = vaults.join(", ")).trim())
    );
}

/// Splits the operands the way `mv` reads its own: everything but the last word is what is to come
/// in, and the last, when there is more than one word, is where it is to go.
///
/// The variant mode reads the same words differently: the first is the variant to bring in and, when
/// a second is written, it is where that variant's file is to lie rather than another reference.
///
/// `None` is a run that named nothing at all, or one that named more than a mode can hold. It reads
/// the command's own words rather than a Layout, so it stays here: what a word means once it has
/// been split is the library's.
fn split(join: Option<&str>, mut words: Vec<String>) -> Option<(Vec<String>, Option<String>)> {
    if words.is_empty() {
        return None;
    }

    if join.is_some() {
        if words.len() > 2 {
            return None;
        }

        let at = (words.len() == 2).then(|| words.pop()).flatten();

        return Some((words, at));
    }

    if words.len() == 1 {
        return Some((words, None));
    }

    let to = words.pop();

    Some((words, to))
}

/// The failure a refused reference is answered with.
fn refused(refusal: Refusal) -> Next {
    match refusal {
        Refusal::Argument => ErrorBringArgument.into(),
        Refusal::Unknown(reference) => ErrorBringUnknown { reference }.into(),
        Refusal::Unnamed(uuid) => ErrorBringUnnamed {
            uuid: uuid.to_string(),
        }
        .into(),
        Refusal::Taken(path) => ErrorBringTaken { path }.into(),
        Refusal::Known(uuid) => ErrorBringKnown { uuid }.into(),
    }
}

/// Error: the Layout being worked in tracks no Vault.
#[derive(Grouped)]
pub struct ErrorBringNoTrack;

impl Failure for ErrorBringNoTrack {
    fn name(&self) -> &'static str {
        "error_bring_no_track"
    }

    fn reason(&self) -> String {
        t!("bring.err_no_track").trim().to_string()
    }
}

failure!(ErrorBringNoTrack);

#[renderer(buffer)]
pub fn render_error_bring_no_track(error: ErrorBringNoTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_no_track_help").trim()));
    ec.exit_code = EC_ERR_BRING;
}

/// Error: `rola bring` was given arguments it cannot use.
#[derive(Grouped)]
pub struct ErrorBringArgument;

impl Failure for ErrorBringArgument {
    fn name(&self) -> &'static str {
        "error_bring_argument"
    }

    fn reason(&self) -> String {
        t!("bring.err_argument").trim().to_string()
    }
}

failure!(ErrorBringArgument);

#[renderer(buffer)]
pub fn render_error_bring_argument(error: ErrorBringArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_argument_help").trim()));
    ec.exit_code = EC_ERR_BRING_ARGUMENT;
}

/// Error: the Vault's Layout names nothing by the reference given.
#[derive(Grouped)]
pub struct ErrorBringUnknown {
    /// The reference that names nothing.
    pub reference: String,
}

impl Failure for ErrorBringUnknown {
    fn name(&self) -> &'static str {
        "error_bring_unknown"
    }

    fn reason(&self) -> String {
        t!("bring.err_unknown", reference = self.reference)
            .trim()
            .to_string()
    }
}

failure!(ErrorBringUnknown);

#[renderer(buffer)]
pub fn render_error_bring_unknown(error: ErrorBringUnknown, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_unknown_help").trim()));
    ec.exit_code = EC_ERR_BRING;
}

/// Error: the Vault's Layout gives the `Uuid` no path, and nothing here names where it is to land.
#[derive(Grouped)]
pub struct ErrorBringUnnamed {
    /// The `Uuid` the Vault's Layout gives no path.
    pub uuid: String,
}

impl Failure for ErrorBringUnnamed {
    fn name(&self) -> &'static str {
        "error_bring_unnamed"
    }

    fn reason(&self) -> String {
        t!("bring.err_unnamed", uuid = self.uuid).trim().to_string()
    }
}

failure!(ErrorBringUnnamed);

#[renderer(buffer)]
pub fn render_error_bring_unnamed(error: ErrorBringUnnamed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_unnamed_help").trim()));
    ec.exit_code = EC_ERR_BRING;
}

/// Error: something is already at the path the reference was to be named by.
#[derive(Grouped)]
pub struct ErrorBringTaken {
    /// The path that is already something.
    pub path: String,
}

impl Failure for ErrorBringTaken {
    fn name(&self) -> &'static str {
        "error_bring_taken"
    }

    fn reason(&self) -> String {
        t!("bring.err_taken", path = self.path).trim().to_string()
    }
}

failure!(ErrorBringTaken);

#[renderer(buffer)]
pub fn render_error_bring_taken(error: ErrorBringTaken, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_taken_help").trim()));
    ec.exit_code = EC_ERR_BRING;
}

/// Error: this Layout already holds the `Uuid` the reference names.
#[derive(Grouped)]
pub struct ErrorBringKnown {
    /// The `Uuid` this Layout already holds.
    pub uuid: String,
}

impl Failure for ErrorBringKnown {
    fn name(&self) -> &'static str {
        "error_bring_known"
    }

    fn reason(&self) -> String {
        t!("bring.err_known", uuid = self.uuid).trim().to_string()
    }
}

failure!(ErrorBringKnown);

#[renderer(buffer)]
pub fn render_error_bring_known(error: ErrorBringKnown, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_known_help").trim()));
    ec.exit_code = EC_ERR_BRING;
}

/// Error: the exchange could not be made.
#[derive(Grouped)]
pub struct ErrorBringFailed {
    /// What the exchange failed with.
    pub cause: String,
}

impl Failure for ErrorBringFailed {
    fn name(&self) -> &'static str {
        "error_bring_failed"
    }

    fn reason(&self) -> String {
        t!("bring.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorBringFailed);

#[renderer(buffer)]
pub fn render_error_bring_failed(error: ErrorBringFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_failed_help").trim()));
    ec.exit_code = EC_ERR_BRING;
}

/// Error: a variant could not be brought in to be joined into a file.
#[derive(Grouped)]
pub struct ErrorBringJoin {
    /// The file it was to be joined into, as the run named it.
    pub target: String,
    /// Why it could not be brought in.
    pub cause: String,
}

impl Failure for ErrorBringJoin {
    fn name(&self) -> &'static str {
        "error_bring_join"
    }

    fn reason(&self) -> String {
        t!("bring.err_join", target = self.target, cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorBringJoin);

#[renderer(buffer)]
pub fn render_error_bring_join(error: ErrorBringJoin, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("bring.err_join_help").trim()));
    ec.exit_code = EC_ERR_BRING;
}

/// One entry that came in.
#[derive(Serialize)]
pub struct BringItem {
    /// The `Uuid` it is known by.
    uuid: String,
    /// The path the Vault's Layout names it by, when it names one.
    remote_path: Option<String>,
    /// The path this Layout now names it by.
    local_path: String,
}

/// Result: what the Vault held was brought in.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultBroughtIn {
    /// The Layout it came into.
    layout: String,
    /// The Vault it came from.
    vault: String,
    /// What came in.
    entries: Vec<BringItem>,
    /// What could not be brought in, one line each.
    failed: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_brought_in(result: ResultBroughtIn, ec: &mut ResExitCode) {
    for entry in &result.entries {
        r_println!(
            "{}  {}  ->  {}",
            entry.uuid,
            entry.remote_path.as_deref().unwrap_or("-"),
            entry.local_path
        );
    }

    for cause in &result.failed {
        r_eprintln!("{}", err_line!(cause));
    }

    r_println!(
        "{}",
        t!(
            "bring.result_summary",
            brought = result.entries.len(),
            failed = result.failed.len()
        )
        .trim()
    );

    if !result.failed.is_empty() {
        ec.exit_code = EC_ERR_BRING;
    }
}

/// Result: a variant was brought in to be joined into a file.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultBringJoin {
    /// The file it is to be joined into, as the run named it.
    target: String,
    /// The variant that was brought in.
    variant: String,
    /// The path its file was written to.
    path: String,
}

#[renderer(buffer)]
pub fn render_result_bring_join(result: ResultBringJoin) {
    r_println!(
        "{}",
        t!(
            "bring.joined",
            target = result.target,
            variant = result.variant,
            path = result.path
        )
        .trim()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words of a command line, as the picker hands them over.
    fn words(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| (*text).to_owned()).collect()
    }

    #[test]
    fn the_last_word_is_the_destination_unless_it_is_the_only_one() {
        assert_eq!(split(None, words(&["a"])), Some((words(&["a"]), None)));
        assert_eq!(
            split(None, words(&["a", "b"])),
            Some((words(&["a"]), Some("b".to_owned())))
        );
        assert_eq!(
            split(None, words(&["a", "b", "c"])),
            Some((words(&["a", "b"]), Some("c".to_owned())))
        );
        assert_eq!(split(None, words(&[])), None);
    }

    #[test]
    fn the_variant_mode_reads_the_second_word_as_where_its_file_lies() {
        assert_eq!(
            split(Some("target"), words(&["variant"])),
            Some((words(&["variant"]), None))
        );
        assert_eq!(
            split(Some("target"), words(&["variant", "here.psd"])),
            Some((words(&["variant"]), Some("here.psd".to_owned())))
        );
        assert_eq!(split(Some("target"), words(&["a", "b", "c"])), None);
        assert_eq!(split(Some("target"), words(&[])), None);
    }
}
