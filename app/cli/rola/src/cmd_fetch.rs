//! The `rola fetch` command: the top-level name of `rola layout fetch`.
//!
//! Bringing a Vault's Layout here is the first thing a run does with a Vault, so it is reached by a
//! word of its own as well as under `layout`. Both are the same command: the same state is built
//! here and the same handler does the work, so what the two names do cannot drift apart.

use mingling::{
    LazyRes, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{arg, buffer, command, completion, help, metadata, r_eprintln, suggest},
    metadata::Description,
    picker::{EntryPicker, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{filling_flag, layout_names, offer, positional, typing_flag, vault_names};
use crate::exit_codes::EC_HELP;
use crate::layout::cmd_layout_fetch::{
    ARG_NO_IGNORE_RULES, ARG_RECURSIVE, ARG_SHALLOW, ARG_WITH, state,
};

#[help(buffer)]
pub fn help_fetch(_: EntryFetch, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_fetch.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFetch)]
pub fn desc_fetch() -> Description {
    t!("cmd_fetch.description").to_string().into()
}

/// Completes what `rola fetch` can be given next.
///
/// What is named is a Vault the Workspace has bound, and never an address: the copy is kept under
/// the name, so a name is the whole of what the command can take, beside the flags it takes.
#[completion(EntryFetch)]
pub fn complete_fetch(
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

/// Brings a Vault's Layout here as a read-only copy
///
/// The same command as [`rola layout fetch`](crate::layout::cmd_layout_fetch), reached by a shorter
/// name: `VAULT` names the Vault to reach, and naming none reaches for the one the Workspace
/// reaches for by default. The copy is written under `.rola/cache/readonly-layouts/<VAULT>/` and
/// replaces whatever was there.
///
/// `--with` brings the content of the entries its words name, `--shallow` (the default) taking the
/// version each entry is at and `--recursive` the whole chain behind it. A word that names no entry
/// and no directory holding one is passed over. `--layout` names the Layout the words are read in,
/// and a run that names `truth@VAULT` there need not name the Vault again.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries it
/// out: the run is not where the exchange is spoken from
/// ([`ErrorShouldInWorkspace`](crate::error::ErrorShouldInWorkspace)), the Workspace has no Vault to
/// hand it ([`ErrorRemoteVault`](crate::error::ErrorRemoteVault)), the run acts as no account or as
/// one no scope holds ([`ErrorNoAccount`](crate::account::ErrorNoAccount),
/// [`ErrorAccountUnknown`](crate::keys::ErrorAccountUnknown)), or the exchange itself failed
/// ([`ActionError`](librorolala::protocol::ActionError)). A fetch of content adds
/// [`ErrorFetchWithMode`](crate::layout::cmd_layout_fetch::ErrorFetchWithMode) when `--recursive`
/// and `--shallow` are both given,
/// [`ErrorFetchWithVault`](crate::layout::cmd_layout_fetch::ErrorFetchWithVault) when the Layout's
/// Vault and the one named are two answers,
/// [`ErrorFetchWithChain`](crate::layout::cmd_layout_fetch::ErrorFetchWithChain) when the index
/// holds no chain for an entry,
/// [`ErrorFetchWithIncomplete`](crate::layout::cmd_layout_fetch::ErrorFetchWithIncomplete) when the
/// content did not all arrive, and
/// [`ErrorFetchWithFailed`](crate::layout::cmd_layout_fetch::ErrorFetchWithFailed) for the rest.
#[command(node = "fetch", entry = EntryFetch)]
pub fn fetch(args: EntryFetch) -> Next {
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

    if let Some(next) = crate::unmatched::refused(&unmatched, "fetch") {
        return next;
    }

    match state(named, no_ignore_rules, layout, with, recursive, shallow) {
        Ok(state) => state.into(),
        Err(next) => next,
    }
}
