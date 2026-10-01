//! The `rola vcs-index ls-remote-variants` command: list the variants the other end's index holds.
//!
//! It is [`ls-variants`](crate::vcs_index::cmd_vcs_index_ls_variants) asked of the other end of an
//! exchange: where that lists the index the run is inside, this reaches the Vault and lists what
//! *its* index holds. What is printed is the variants' hashes, one per line, so a line read here is
//! one to hand to [`read-remote`](crate::vcs_index::cmd_vcs_index_read_remote).

use mingling::{
    Grouped, LazyRes, ShellContext, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResOffline, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{offer, positional, typing_flag, vault_names};
use crate::error::ErrorOffline;
use crate::exit_codes::EC_HELP;
use crate::format::ResFormat;
use crate::keys::account_named;
use crate::vcs_index::{DEFAULT_FORMAT_HASHES, list_remote_hashes};

#[help(buffer)]
pub fn help_vcs_index_ls_remote_variants(_: EntryVcsIndexLsRemoteVariants, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_ls_remote.variants_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexLsRemoteVariants)]
pub fn desc_vcs_index_ls_remote_variants() -> Description {
    t!("vcs_index_ls_remote.variants_description")
        .to_string()
        .into()
}

/// Lists the variants the index at the other end holds
///
/// `VAULT` names the Vault to reach, as [`vcs-index sync-all`](crate::vcs_index::cmd_vcs_index_sync_all)
/// reads it: a name the Workspace has bound, or an ip and a port, with none naming the one the
/// Workspace reaches for by default. What is printed is the variants' hashes, one per line, the way
/// [`rola vcs-index ls-variants`](crate::vcs_index::cmd_vcs_index_ls_variants) prints them of the
/// index the run is inside — each hash one to hand to `rola vcs-index read-remote`.
///
/// Nothing is changed: the Vault is asked and answers, and no object moves.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries it
/// out: the run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]), the Workspace
/// has no Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or as one no scope
/// holds ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), or the exchange itself failed
/// ([`ActionError`]).
///
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "vcs-index.ls-remote-variants", entry = EntryVcsIndexLsRemoteVariants)]
pub fn vcs_index_ls_remote_variants(
    args: EntryVcsIndexLsRemoteVariants,
    format: &mut ResFormat,
) -> StateVcsIndexLsRemoteVariants {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    format.default_template(DEFAULT_FORMAT_HASHES);
    StateVcsIndexLsRemoteVariants::from(named)
}

/// The state of listing the other end's variants.
#[derive(Grouped, Wrap)]
pub struct StateVcsIndexLsRemoteVariants {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_vcs_index_ls_remote_variants(
    state: StateVcsIndexLsRemoteVariants,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    format: &mut ResFormat,
    offline: &ResOffline,
) -> Next {
    // The other end's index is the whole of what is asked for here, and an offline run may not ask
    // it.
    if **offline {
        return ErrorOffline.into();
    }

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let target = remote
        .get_ref()
        .vault_or_default(state.vault.unwrap_or_default())?;

    let name = current.get_ref().must_bind()?;
    let account = account_named(&name, Some(held), None)?;

    let listing = list_remote_hashes(held, &account, &target.to_string(), "variants")?;
    format.set(
        "string_hashes",
        listing
            .string_hashes
            .iter()
            .map(|hash| serde_json::json!(hash))
            .collect(),
    );

    listing.into()
}

/// Completes what `rola vcs-index ls-remote-variants` can be given next.
///
/// The one word names the Vault to reach, so the names the Workspace has bound are what is offered;
/// naming none reaches for the one it reaches for.
#[completion(EntryVcsIndexLsRemoteVariants)]
pub fn complete_vcs_index_ls_remote_variants(
    ctx: ShellContext,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "ls-remote-variants") != 0 {
        return suggest!();
    }

    offer(&ctx, vault_names(remote.get_ref()))
}
