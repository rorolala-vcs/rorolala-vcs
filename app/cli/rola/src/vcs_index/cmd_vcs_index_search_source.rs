//! The `rola vcs-index search-source` command: which Vaults hold a target's content.
//!
//! A key is the hash of its content, so any Vault holding it hands back the same content. This is
//! the asking: every Vault the Workspace has bound is asked what its store holds, and a target is
//! answered with the Vaults holding every object it needs. A target this side cannot resolve is
//! named by the Vault's own index, so a version this side never fetched can still be found.
//!
//! It is the one command of the pair that reaches the network, so `--offline` refuses it outright.

// The handler's parameters are the resources the framework injects, so how many of them there are
// is what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments)]

use std::collections::BTreeSet;

use librorolala::existence::search;
use librorolala::status::tracked_vault;
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
use rorolala_cli_setups::{ResCurrentRemoteVault, ResOffline, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_utils_cli_theme::{err_line, trd, warn_line};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{IndexObject, index_hashes, offer, typing_flag};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::fetch;
use crate::format::ResFormat;
use crate::keys::account_named;
use crate::layout::chosen;
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexNoIndex, ErrorVcsIndexNoVault, runtime, target_of,
};

#[help(buffer)]
pub fn help_vcs_index_search_source(_: EntryVcsIndexSearchSource, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_search_source.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexSearchSource)]
pub fn desc_vcs_index_search_source() -> Description {
    t!("vcs_index_search_source.description").to_string().into()
}

/// Completes what `rola vcs-index search-source` can be given next.
///
/// A target is a variant or a version of this index, so what the index holds of those is offered.
/// A path the Layout records is read as well, and is not offered for the same reason the other
/// command does not offer one.
#[completion(EntryVcsIndexSearchSource)]
pub fn complete_vcs_index_search_source(
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

/// Ask which Vaults hold the content a target names
///
/// `TARGET` is a variant hash, a version hash, or a path the Layout being worked in records. Every
/// Vault the Workspace has bound is asked what its store holds, and a target is answered with the
/// Vaults holding every object it needs, the content's chunks included. A target whose version or
/// variant is not here is named by each Vault's own index, so a version this side never fetched can
/// still be found.
///
/// The answers keep the order the targets were written in. A Vault that cannot be reached is
/// reported rather than ending the search: one Vault being down says nothing about the rest.
///
/// # Errors
///
/// Renders [`ErrorOffline`] when the run is offline, [`ErrorShouldInWorkspace`] when it is not inside
/// a Workspace, [`ErrorLayoutShouldInWorkspace`](crate::layout::ErrorLayoutShouldInWorkspace) when it
/// works in no Layout, [`ErrorVcsIndexNoIndex`] when it holds no version control index,
/// [`ErrorVcsIndexNoVault`] when it has bound no Vault to ask, [`ErrorVcsIndexArgument`] when no
/// target is named, [`ErrorVcsIndexHash`] when a hash names no one object, [`ErrorVcsIndexTarget`]
/// when a word names no Layout entry, [`ErrorVcsIndexRead`] when the index cannot say what a key
/// holds, and the account and Vault failures the exchange reports for itself.
///
/// [`ErrorShouldInWorkspace`]: rorolala_cli_setups::ErrorShouldInWorkspace
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ErrorRemoteVault`]: rorolala_cli_setups::ErrorRemoteVault
/// [`ErrorVcsIndexHash`]: crate::vcs_index::ErrorVcsIndexHash
/// [`ErrorVcsIndexTarget`]: crate::vcs_index::ErrorVcsIndexTarget
/// [`ErrorVcsIndexRead`]: crate::vcs_index::ErrorVcsIndexRead
#[command(node = "vcs-index.search-source", entry = EntryVcsIndexSearchSource)]
pub fn vcs_index_search_source(args: EntryVcsIndexSearchSource) -> Next {
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

    if let Some(next) = crate::unmatched::refused(&unmatched, "search-source") {
        return next;
    }

    StateVcsIndexSearchSource { words }.into()
}

/// The state a search starts in: the targets it names.
#[derive(Grouped)]
pub struct StateVcsIndexSearchSource {
    /// The targets, in the order they were written.
    words: Vec<String>,
}

#[chain(routeify)]
pub fn handle_vcs_index_search_source(
    state: StateVcsIndexSearchSource,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: &ResOffline,
    format: &mut ResFormat,
) -> Next {
    // The Vaults are the whole of what is asked for here, and an offline run may not ask them.
    if **offline {
        return ErrorOffline.into();
    }

    let StateVcsIndexSearchSource { words } = state;

    // The Layout is read before the Workspace is held: both come out of the same resource, and the
    // reading of one is what the second borrow would have to wait for.
    let layout = chosen(workspace.get_ref(), vault.get_ref(), None)?;
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = runtime()?;

    let mut targets = Vec::with_capacity(words.len());
    for word in &words {
        targets.push(target_of(word, &layout, index, &runtime)?);
    }

    let name = current.get_ref().must_bind()?;
    let account = account_named(&name, Some(held), None)?;
    let tracked = tracked_vault(held);
    let primary = fetch::primary(remote.get_ref(), tracked.as_deref());
    let candidates = fetch::candidates(remote.get_ref(), primary);

    if candidates.is_empty() {
        return ErrorVcsIndexNoVault.into();
    }

    let sourced = search(
        &runtime,
        held,
        &layout,
        index,
        &account,
        &candidates,
        &targets,
    );

    let mut unreachable: BTreeSet<&str> = BTreeSet::new();
    let targets: Vec<Answered> = words
        .iter()
        .zip(&sourced)
        .map(|(word, sourced)| {
            unreachable.extend(sourced.unreachable.iter().map(String::as_str));

            Answered {
                target: word.clone(),
                sources: sourced.sources.clone(),
                unreachable: sourced.unreachable.clone(),
            }
        })
        .collect();

    if !unreachable.is_empty() {
        let names: Vec<&str> = unreachable.into_iter().collect();
        eprintln!(
            "{}",
            warn_line!(
                t!(
                    "vcs_index_search_source.warn_unreachable",
                    vaults = names.join(", ")
                )
                .trim()
            )
        );
    }

    format.set(
        "targets",
        targets
            .iter()
            .map(|answer| serde_json::json!(answer))
            .collect(),
    );

    ResultVcsIndexSearchSource { targets }.into()
}

/// One target and the Vaults that hold what it names.
#[derive(Serialize)]
pub struct Answered {
    /// The target, as it was written.
    target: String,
    /// The Vaults whose store holds every object it needs.
    sources: Vec<String>,
    /// The Vaults that could not be asked.
    unreachable: Vec<String>,
}

/// Result: every target was searched for.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexSearchSource {
    /// One answer per target, in the order asked.
    targets: Vec<Answered>,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_search_source(
    result: ResultVcsIndexSearchSource,
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
        let none = t!("vcs_index_search_source.no_source").trim().to_string();

        for answer in &result.targets {
            let said = if answer.sources.is_empty() {
                none.clone()
            } else {
                answer.sources.join(",")
            };

            r_println!("{} {}", answer.target, said);
        }
    }
}
