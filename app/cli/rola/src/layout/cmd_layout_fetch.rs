//! The `rola layout fetch` command: bring a Vault's Layout here to read.
//!
//! A Vault holds one Layout, and it is the truth about what the work is made of: which path names
//! which `Uuid`, and who holds it. A Workspace cannot work in it, it is elsewhere, so what it
//! fetches is a copy, kept under the name the Vault was bound by and read rather than worked in.
//!
//! What the copy names is versions, and a version is only a meaning once its content is here. So
//! the same command takes `--with`, which reads the words it is given as entries of the Layout and
//! brings the content of the versions those entries are at. The work itself is the library's, see
//! [`FetchWithRequest::fetch_with`]: what stays here is reading the flags, saying the result in this
//! command's words, and the exit code a caller branches on.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates, so a
// lint allowed on the handler below is reported as defined twice. The allow lives here, where it
// covers the one signature that needs it: the resources are the framework's to inject, and how they
// are handed over is not this program's to choose.
#![allow(clippy::trivially_copy_pass_by_ref)]

use librorolala::daemon::{
    FetchIgnoreRulesRequest, FetchLayoutRequest, action_fetch_ignore_rules, action_fetch_layout,
};
use librorolala::write::fetch_with::{
    FetchWithMissing, FetchWithRefusal, FetchWithRequest, FetchWithSource,
    HashMiss as FetchHashMiss,
};
use librorolala::write::request::{Common, Target};
use librorolala::write::{Labels, WriteError};
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, StructuralData, Suggest,
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
    ResConcurrency, ResCurrentRemoteVault, ResOffline, ResProgressSetting, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd, warn_line};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::{ErrorNoAccount, ResCurrentAccount};
use crate::complete::{filling_flag, layout_names, offer, positional, typing_flag, vault_names};
use crate::error::ErrorOffline;
use crate::exit_codes::{
    EC_CANCELLED, EC_ERR_FETCH_WITH_CHAIN, EC_ERR_FETCH_WITH_FAILED, EC_ERR_FETCH_WITH_HASH,
    EC_ERR_FETCH_WITH_INCOMPLETE, EC_ERR_LAYOUT_ARGUMENT, EC_HELP,
};
use crate::failure::failure;
use crate::hash::HashMiss;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutNoCurrent, ErrorLayoutNotCached, remote_spec,
};
use crate::progress::Reporting;
use crate::vcs_index::ErrorVcsIndexNoIndex;

#[help(buffer)]
pub fn help_layout_fetch(_: EntryLayoutFetch, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_fetch.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutFetch)]
pub fn desc_layout_fetch() -> Description {
    t!("cmd_layout_fetch.description").to_string().into()
}

/// Completes what `rola layout fetch` can be given next.
///
/// What is named is a Vault the Workspace has bound, and never an address: the copy is kept under
/// the name, so a name is the whole of what the command can take, beside the flags it takes.
#[completion(EntryLayoutFetch)]
pub fn complete_layout_fetch(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) {
        return crate::complete::strip_written(
            &ctx,
            suggest! {
                ARG_WITH: t!("fetch.complete.with"),
                crate::layout::ARG_LAYOUT: t!("fetch.complete.layout"),
                ARG_RECURSIVE: t!("fetch.complete.recursive"),
                ARG_SHALLOW: t!("fetch.complete.shallow"),
                ARG_NO_IGNORE_RULES: t!("fetch.complete.no_ignore_rules"),
            },
        );
    }

    // A word of `--with` is where something sits, so the filesystem answers it. What the word
    // comes to is read later, by the command itself, and a shell cannot know it.
    if filling_flag(&ctx, &ARG_WITH) {
        return Suggest::file_comp();
    }

    if filling_flag(&ctx, &crate::layout::ARG_LAYOUT) {
        return offer(&ctx, layout_names(workspace.get_ref()));
    }

    if positional(&ctx, "fetch") != 0 {
        return suggest!();
    }

    offer(&ctx, vault_names(remote.get_ref()))
}

/// Fetch only the Layout, leaving the Vault's ignore rules unfetched.
pub const ARG_NO_IGNORE_RULES: PickerArg<'static, Flag> = arg![no_ignore_rules: Flag];

/// The entries whose content comes with the Layout, by path, directory or `Uuid`.
pub const ARG_WITH: PickerArg<'static, Vec<String>> = arg![with: Vec<String>];

/// Follow each version back to its root, bringing the whole chain.
pub const ARG_RECURSIVE: PickerArg<'static, Flag> = arg![recursive: Flag];

/// Bring only the version each entry names, which is the default.
pub const ARG_SHALLOW: PickerArg<'static, Flag> = arg![shallow: Flag];

/// Brings a Vault's Layout here as a read-only copy
///
/// `VAULT` names the Vault to reach: a name the Workspace has
/// [bound](crate::vault::cmd_vault_bind), and not an address, since what is fetched is kept under
/// the Vault's name. Naming none reaches for the one the Workspace
/// [reaches for by default](crate::vault::cmd_vault_set_default).
///
/// The copy is written under `.rola/cache/readonly-layouts/<VAULT>/` and replaces whatever was
/// there. Nothing of the Workspace's own Layouts is touched, and nothing moves the other way.
///
/// `--with` brings the content of the entries, variants and versions its words name, `--shallow`
/// (the default) taking the version each entry is at or the content each variant points at, and
/// `--recursive` the whole chain behind either. A word that names no entry and no directory holding
/// one is passed over, and a word written as a hash is refused when it names no variant or version.
/// `--layout` names the Layout the path words are read in, and a run that names `truth@VAULT` there
/// need not name the Vault again.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries
/// it out: the run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]), the
/// Workspace has no Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or as one
/// no scope holds ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), or the exchange itself failed
/// ([`ActionError`]). A fetch of content adds [`ErrorFetchWithMode`] when `--recursive` and
/// `--shallow` are both given, [`ErrorFetchWithVault`] when the Layout's Vault and the one named are
/// two answers, [`ErrorFetchWithHash`] when a hash names no variant or version,
/// [`ErrorFetchWithChain`] when the index holds no chain for a target,
/// [`ErrorFetchWithIncomplete`] when the content did not all arrive, and [`ErrorFetchWithFailed`]
/// for the rest.
///
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "layout.fetch", entry = EntryLayoutFetch)]
pub fn layout_fetch(args: EntryLayoutFetch) -> Next {
    // UNWRAP: Picking cannot fail: a flag is a flag and a positional that is absent is `None`, and
    // naming none is what lets the Workspace's own choice be the one that is reached for.
    let (no_ignore_rules, layout, with, recursive, shallow, named, unmatched): (
        Flag,
        Option<String>,
        Vec<String>,
        Flag,
        Flag,
        Option<String>,
        _,
    ) = args
        .pick(ARG_NO_IGNORE_RULES)
        .pick(crate::layout::ARG_LAYOUT)
        .pick(ARG_WITH)
        .pick(ARG_RECURSIVE)
        .pick(ARG_SHALLOW)
        .pick(arg![Option<String>])
        .pick(&UNMATCHED)
        .unwrap();

    if let Some(next) = crate::unmatched::refused(&unmatched, "layout fetch") {
        return next;
    }

    match state(named, no_ignore_rules, layout, with, recursive, shallow) {
        Ok(state) => state.into(),
        Err(next) => next,
    }
}

/// The state a fetch starts in, from the words and flags both names of the command read.
///
/// It is the one way the state is made: `rola fetch` and `rola layout fetch` are the same command,
/// so what a fetch is cannot be two answers. What can be refused on the spot is refused here.
pub fn state(
    vault: Option<String>,
    no_ignore_rules: Flag,
    layout: Option<String>,
    with: Vec<String>,
    recursive: Flag,
    shallow: Flag,
) -> Result<StateLayoutFetch, Next> {
    if matches!(recursive, Flag::Active) && matches!(shallow, Flag::Active) {
        return Err(ErrorFetchWithMode.into());
    }

    // A Layout named `NAME@VAULT` already says which Vault the words and the versions are of, so
    // naming another beside it is two answers to one question.
    let from_layout = layout
        .as_deref()
        .and_then(remote_spec)
        .map(|(_, vault)| vault.to_owned());

    if let (Some(named), Some(from_layout)) = (&vault, &from_layout)
        && named != from_layout
    {
        return Err(ErrorFetchWithVault {
            layout: layout.unwrap_or_default(),
            layout_vault: from_layout.clone(),
            named: named.clone(),
        }
        .into());
    }

    let vault = vault.or(from_layout);

    Ok(StateLayoutFetch::new(
        vault,
        matches!(no_ignore_rules, Flag::Active),
        layout,
        with,
        matches!(recursive, Flag::Active),
    ))
}

/// The state of fetching a Vault's Layout.
///
/// Which Vault is reached, whether its ignore rules come with it, which Layout the words are read
/// in, and what content comes beside it is the whole of what is said: naming none reaches for the
/// one the Workspace reaches for by default, and the rules are fetched unless the run said not to.
#[derive(Grouped)]
pub struct StateLayoutFetch {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
    /// Whether the Vault's ignore rules are left unfetched.
    no_ignore_rules: bool,
    /// The Layout the words are read in, or nothing for the one being worked in.
    layout: Option<String>,
    /// The entries whose content comes with the copy.
    with: Vec<String>,
    /// Whether each version comes with its whole chain.
    recursive: bool,
}

impl StateLayoutFetch {
    /// The state a fetch of `vault` starts in, with its rules left unfetched when the run said so.
    ///
    /// It is the one way the state is made: the two names share the command, so a field-by-field
    /// construction written once per name would be two places for what a fetch is to disagree.
    pub const fn new(
        vault: Option<String>,
        no_ignore_rules: bool,
        layout: Option<String>,
        with: Vec<String>,
        recursive: bool,
    ) -> Self {
        Self {
            vault,
            no_ignore_rules,
            layout,
            with,
            recursive,
        }
    }
}

#[chain(routeify)]
pub fn handle_layout_fetch(
    state: StateLayoutFetch,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    progress: &ResProgressSetting,
    concurrency: &ResConcurrency,
    offline: &ResOffline,
) -> Next {
    // This command is the fetch, so an offline run has nothing to do and nowhere to look.
    if **offline {
        return ErrorOffline.into();
    }

    // The copy is kept beside the Workspace and named after the Vault, so the two are needed
    // before anything is spoken: `check` is `routeify`'s, and a run outside a Workspace leaves
    // through it.
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    // What is fetched is kept under the Vault's name, so the name is what is resolved first and
    // the address only to dial it: `?` is `routeify`'s in both.
    let name = remote
        .get_ref()
        .name_or_default(state.vault.unwrap_or_default())?;
    let target = remote.get_ref().vault_or_default(name.clone())?;

    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    action_fetch_layout(
        held,
        &account,
        target.to_string(),
        &FetchLayoutRequest { name: name.clone() },
    )?;

    // The rules the Vault manages come with the Layout: they are what the fetched copy means for
    // the readings the Workspace makes, and a rule the Vault gave up is gone from here by the same
    // fetch. A run that named `--no-ignore-rules` leaves whatever is here as it is.
    if !state.no_ignore_rules {
        action_fetch_ignore_rules(
            held,
            &account,
            target.to_string(),
            &FetchIgnoreRulesRequest { name: name.clone() },
        )?;
    }

    // What is named is the place the copy is kept rather than the generation this run wrote: a
    // generation is a name of the copy's own, and one a reader has no use for.
    let path = held
        .readonly_layouts_dir(&name, VAULT_LAYOUT_NAME)
        .display()
        .to_string();

    // The content is the same command's second half, and it reads the copy this run has just
    // written: the words are read against a Layout the Vault answered with, not one left here by
    // an older run.
    if state.with.is_empty() {
        return ResultLayoutFetched { path, vault: name }.into();
    }

    let request = FetchWithRequest {
        common: Common {
            workspace: held.get_root().to_path_buf(),
            account: account_name,
            target: Target::Named { name: name.clone() },
            offline: false,
            force: false,
            concurrency: **concurrency,
            labels: Labels::default(),
        },
        layout: state.layout.unwrap_or_default(),
        words: state.with,
        recursive: state.recursive,
    };

    let reporting = Reporting::start(*progress);
    let outcome = request.fetch_with(&reporting.progress(), crate::cancel::on_ctrl_c());
    reporting.finish();

    match outcome {
        Ok(report) if report.entries.is_empty() => ResultLayoutFetched { path, vault: name }.into(),
        Ok(report) => {
            report_fetched(&report.brought);

            ResultFetchWith {
                vault: name,
                path,
                entries: report.entries.len(),
            }
            .into()
        }
        Err(refusal) => refused(refusal),
    }
}

/// Says what the Vaults handed over, in one line, once the run is done with them.
///
/// The words are the command's: what a fetch came to is a count and the names of the Vaults it came
/// from, and the library keeps neither a language nor a place to print them.
fn report_fetched(brought: &[FetchWithSource]) {
    if brought.is_empty() {
        return;
    }

    let count: usize = brought.iter().map(|source| source.count).sum();
    let vaults: Vec<&str> = brought.iter().map(|source| source.vault.as_str()).collect();

    eprintln!(
        "{}",
        warn_line!(t!("fetch.brought", count = count, vaults = vaults.join(", ")).trim())
    );
}

/// The failure a refused fetch of content is answered with, in this command's words.
fn refused(refusal: FetchWithRefusal) -> Next {
    match refusal {
        FetchWithRefusal::Cancelled => ResultFetchCancelled.into(),
        FetchWithRefusal::NoLayout => ErrorLayoutNoCurrent.into(),
        FetchWithRefusal::NotCached { layout, vault } => {
            ErrorLayoutNotCached { layout, vault }.into()
        }
        FetchWithRefusal::LayoutName => ErrorLayoutMissing.into(),
        FetchWithRefusal::LayoutFailed { reason } => ErrorLayoutFailed::new(reason).into(),
        FetchWithRefusal::Hash { hash, miss } => ErrorFetchWithHash {
            hash,
            miss: match miss {
                FetchHashMiss::Unknown => HashMiss::Unknown,
                FetchHashMiss::Ambiguous(candidates) => HashMiss::Ambiguous(candidates),
            },
        }
        .into(),
        FetchWithRefusal::Chain { target, reason } => ErrorFetchWithChain { target, reason }.into(),
        FetchWithRefusal::Incomplete { entries } => ErrorFetchWithIncomplete { entries }.into(),
        FetchWithRefusal::Action(error) => error.to_chain(),
        FetchWithRefusal::Failed(WriteError::NoIndex) => ErrorVcsIndexNoIndex.into(),
        FetchWithRefusal::Failed(WriteError::NoAccount) => ErrorNoAccount.into(),
        FetchWithRefusal::Failed(error) => ErrorFetchWithFailed {
            cause: error.to_string(),
        }
        .into(),
    }
}

/// Result: a Vault's Layout was fetched here.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutFetched {
    /// The Vault it was fetched from.
    vault: String,
    /// Where the copy was written.
    path: String,
}

#[renderer(buffer)]
pub fn render_result_layout_fetched(result: ResultLayoutFetched) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_fetch.result_fetched",
            vault = result.vault,
            path = result.path
        )
        .trim()
    );
}

/// Result: a Vault's Layout was fetched here, and with it the content of some entries.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultFetchWith {
    /// The Vault it was fetched from.
    vault: String,
    /// Where the copy was written.
    path: String,
    /// How many entries came with it.
    entries: usize,
}

#[renderer(buffer)]
pub fn render_result_fetch_with(result: ResultFetchWith) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_fetch.result_fetched",
            vault = result.vault,
            path = result.path
        )
        .trim()
    );
    r_println!("{}", t!("fetch.with_result", count = result.entries).trim());
}

/// Result: the fetch was stopped before the content was all here.
#[derive(Grouped)]
pub struct ResultFetchCancelled;

#[renderer(buffer)]
pub fn render_result_fetch_cancelled(_: ResultFetchCancelled, ec: &mut ResExitCode) {
    r_eprintln!("{}", help_line!(t!("fetch.cancelled").trim()));
    ec.exit_code = EC_CANCELLED;
}

/// Error: `--recursive` and `--shallow` say opposite things.
#[derive(Grouped)]
pub struct ErrorFetchWithMode;

impl Failure for ErrorFetchWithMode {
    fn name(&self) -> &'static str {
        "error_fetch_with_mode"
    }

    fn reason(&self) -> String {
        t!("fetch.with_mode").trim().to_owned()
    }
}

failure!(ErrorFetchWithMode);

#[renderer(buffer)]
pub fn render_error_fetch_with_mode(error: ErrorFetchWithMode, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: the Layout's Vault and the Vault named are two answers.
#[derive(Grouped)]
pub struct ErrorFetchWithVault {
    /// The Layout named, as it was written.
    layout: String,
    /// The Vault it belongs to.
    layout_vault: String,
    /// The Vault the run named beside it.
    named: String,
}

impl Failure for ErrorFetchWithVault {
    fn name(&self) -> &'static str {
        "error_fetch_with_vault"
    }

    fn reason(&self) -> String {
        t!(
            "fetch.with_vault",
            layout = self.layout,
            layout_vault = self.layout_vault,
            named = self.named
        )
        .trim()
        .to_owned()
    }
}

failure!(ErrorFetchWithVault);

#[renderer(buffer)]
pub fn render_error_fetch_with_vault(error: ErrorFetchWithVault, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: a word written as a hash names no one variant or version.
#[derive(Grouped)]
pub struct ErrorFetchWithHash {
    /// The word, as it was written.
    hash: String,
    /// Why it named no one object.
    miss: HashMiss,
}

impl Failure for ErrorFetchWithHash {
    fn name(&self) -> &'static str {
        "error_fetch_with_hash"
    }

    fn reason(&self) -> String {
        self.miss.reason(&self.hash, || {
            t!("fetch.with_hash", hash = self.hash).trim().to_owned()
        })
    }
}

failure!(ErrorFetchWithHash);

#[renderer(buffer)]
pub fn render_error_fetch_with_hash(error: ErrorFetchWithHash, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    let malformed = t!("fetch.with_hash_help").trim().to_owned();
    r_eprintln!("{}", help_line!(error.miss.help(&malformed)));
    ec.exit_code = EC_ERR_FETCH_WITH_HASH;
}

/// Error: the index holds no chain for a target the words name.
#[derive(Grouped)]
pub struct ErrorFetchWithChain {
    /// The target the chain was wanted for.
    target: String,
    /// Why no chain could be read.
    reason: String,
}

impl Failure for ErrorFetchWithChain {
    fn name(&self) -> &'static str {
        "error_fetch_with_chain"
    }

    fn reason(&self) -> String {
        t!(
            "fetch.with_chain",
            target = self.target,
            reason = self.reason
        )
        .trim()
        .to_owned()
    }
}

failure!(ErrorFetchWithChain);

#[renderer(buffer)]
pub fn render_error_fetch_with_chain(error: ErrorFetchWithChain, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_FETCH_WITH_CHAIN;
}

/// Error: the content of some targets did not arrive.
#[derive(Grouped)]
pub struct ErrorFetchWithIncomplete {
    /// The targets still missing content.
    entries: Vec<FetchWithMissing>,
}

impl Failure for ErrorFetchWithIncomplete {
    fn name(&self) -> &'static str {
        "error_fetch_with_incomplete"
    }

    fn reason(&self) -> String {
        let entries: Vec<String> = self
            .entries
            .iter()
            .map(|entry| format!("{} ({})", entry.target, entry.path))
            .collect();

        t!("fetch.with_incomplete", entries = entries.join(", "))
            .trim()
            .to_owned()
    }
}

failure!(ErrorFetchWithIncomplete);

#[renderer(buffer)]
pub fn render_error_fetch_with_incomplete(error: ErrorFetchWithIncomplete, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("fetch.with_incomplete_help").trim()));
    ec.exit_code = EC_ERR_FETCH_WITH_INCOMPLETE;
}

/// Error: the fetch of content failed for a reason of its own.
#[derive(Grouped)]
pub struct ErrorFetchWithFailed {
    /// What failed, in the words of the part that knows.
    cause: String,
}

impl Failure for ErrorFetchWithFailed {
    fn name(&self) -> &'static str {
        "error_fetch_with_failed"
    }

    fn reason(&self) -> String {
        self.cause.clone()
    }
}

failure!(ErrorFetchWithFailed);

#[renderer(buffer)]
pub fn render_error_fetch_with_failed(error: ErrorFetchWithFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_FETCH_WITH_FAILED;
}
