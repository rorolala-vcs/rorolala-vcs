//! The `rola vcs-index can-retrack-offline` command: whether a target's content is stored here.
//!
//! A variant names content, a version names the variant it fixes, and a Layout entry names the
//! version it records, so all three lead to one key: the store's key for the content. What this
//! answers is whether that key, and every object on the way to it, is here, which is what makes a
//! retrack to the target local work rather than a fetch.
//!
//! A tree is content too, and its structure names the content of every file it holds, so a tree is
//! checked down to those files: a structure whose files are gone puts nothing back.

use librorolala::status::local;
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{err_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::complete::{IndexObject, index_hashes, offer, typing_flag};
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::format::ResFormat;
use crate::layout::chosen;
use crate::vcs_index::{ErrorVcsIndexArgument, ErrorVcsIndexNoIndex, runtime, target_of};

#[help(buffer)]
pub fn help_vcs_index_can_retrack_offline(_: EntryVcsIndexCanRetrackOffline, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_can_retrack_offline.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexCanRetrackOffline)]
pub fn desc_vcs_index_can_retrack_offline() -> Description {
    t!("vcs_index_can_retrack_offline.description")
        .to_string()
        .into()
}

/// Completes what `rola vcs-index can-retrack-offline` can be given next.
///
/// A target is a variant or a version of this index, so what the index holds of those is offered.
/// A path the Layout records is not offered: the command reads one, and a listing of every path is
/// the Layout's business rather than this word's.
#[completion(EntryVcsIndexCanRetrackOffline)]
pub fn complete_vcs_index_can_retrack_offline(
    ctx: ShellContext,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    offer(
        &ctx,
        index_hashes(index.get_ref().as_ref(), IndexObject::Chain),
    )
}

/// Check whether a target's content is stored here, so it can be retracked offline
///
/// `TARGET` is a variant hash, a version hash, or a path the Layout being worked in records. A
/// variant, a version or an entry is answered `yes` when every object from it to its content is
/// held, the content's chunks included; a content that is a tree is checked down to the files its
/// structure names.
///
/// The answers keep the order the targets were written in, so a run may ask about a list and read
/// the answer beside it. Nothing is changed and no Vault is reached.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutShouldInWorkspace`](crate::layout::ErrorLayoutShouldInWorkspace) when it works in no
/// Layout, [`ErrorVcsIndexNoIndex`] when it holds no version control index,
/// [`ErrorVcsIndexArgument`] when no target is named, [`ErrorVcsIndexHash`] when a hash names no one
/// object, [`ErrorVcsIndexTarget`] when a word names no Layout entry, and [`ErrorVcsIndexRead`] when
/// the index cannot say what a key holds.
///
/// [`ErrorShouldInWorkspace`]: rorolala_cli_setups::ErrorShouldInWorkspace
/// [`ErrorVcsIndexHash`]: crate::vcs_index::ErrorVcsIndexHash
/// [`ErrorVcsIndexTarget`]: crate::vcs_index::ErrorVcsIndexTarget
/// [`ErrorVcsIndexRead`]: crate::vcs_index::ErrorVcsIndexRead
#[command(node = "vcs-index.can-retrack-offline", entry = EntryVcsIndexCanRetrackOffline)]
pub fn vcs_index_can_retrack_offline(args: EntryVcsIndexCanRetrackOffline) -> Next {
    let picked = args
        .pick_or_route(arg![Vec<String>], || {
            ErrorVcsIndexArgument {
                argument: "TARGET".to_owned(),
            }
            .to_chain()
        })
        .pick(&UNMATCHED)
        .to_result();

    let (words, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "can-retrack-offline") {
        return next;
    }

    StateVcsIndexCanRetrackOffline { words }.into()
}

/// The state a check starts in: the targets it names.
#[derive(Grouped)]
pub struct StateVcsIndexCanRetrackOffline {
    /// The targets, in the order they were written.
    words: Vec<String>,
}

#[chain(routeify)]
pub fn handle_vcs_index_can_retrack_offline(
    state: StateVcsIndexCanRetrackOffline,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    format: &mut ResFormat,
) -> Next {
    let StateVcsIndexCanRetrackOffline { words } = state;

    workspace.get_ref().check()?;

    let layout = chosen(workspace.get_ref(), vault.get_ref(), None)?;
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = runtime()?;

    let mut targets = Vec::with_capacity(words.len());
    for word in &words {
        targets.push(target_of(word, &layout, index, &runtime)?);
    }

    // UNWRAP: the store resource is an `Option` a run without a store holds as `None`, which the
    // reading answers as nothing being held rather than as a failure.
    let store = storage.get_ref().as_ref();
    let answers = runtime.block_on(local(&layout, index, store, &targets));

    let targets: Vec<Answered> = words
        .iter()
        .zip(&answers)
        .map(|(word, answer)| Answered {
            target: word.clone(),
            offline: answer.held,
        })
        .collect();

    format.set(
        "targets",
        targets
            .iter()
            .map(|answer| serde_json::json!(answer))
            .collect(),
    );

    ResultVcsIndexCanRetrackOffline { targets }.into()
}

/// One target and whether its content is here.
#[derive(Serialize)]
pub struct Answered {
    /// The target, as it was written.
    target: String,
    /// Whether every object it needs is stored here.
    offline: bool,
}

/// Result: every target was answered.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexCanRetrackOffline {
    /// One answer per target, in the order asked.
    targets: Vec<Answered>,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_can_retrack_offline(
    result: ResultVcsIndexCanRetrackOffline,
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
        let yes = t!("vcs_index_can_retrack_offline.yes").trim().to_string();
        let no = t!("vcs_index_can_retrack_offline.no").trim().to_string();

        for answer in &result.targets {
            r_println!(
                "{} {}",
                answer.target,
                if answer.offline { &yes } else { &no }
            );
        }
    }
}
