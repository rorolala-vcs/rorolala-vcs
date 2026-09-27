//! The `rola vcs-index ls-remote-versions` command: list the versions the other end's index holds.
//!
//! It is [`ls-versions`](crate::vcs_index::cmd_vcs_index_ls_versions) asked of the other end of an
//! exchange: where that lists the index the run is inside, this reaches the Vault and lists what
//! *its* index holds. What is printed is the versions' hashes, one per line — not their numbers,
//! which would cost a trace of the chain for every one — so a line read here is one to hand to
//! [`read-remote`](crate::vcs_index::cmd_vcs_index_read_remote).

use mingling::{
    Grouped, LazyRes, Wrap,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::EC_HELP;
use crate::keys::account_named;
use crate::vcs_index::list_remote_hashes;

#[help(buffer)]
pub fn help_vcs_index_ls_remote_versions(_: EntryVcsIndexLsRemoteVersions, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_ls_remote.versions_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexLsRemoteVersions)]
pub fn desc_vcs_index_ls_remote_versions() -> Description {
    t!("vcs_index_ls_remote.versions_description")
        .to_string()
        .into()
}

/// Lists the versions the index at the other end holds
///
/// `VAULT` names the Vault to reach, as [`vcs-index sync-all`](crate::vcs_index::cmd_vcs_index_sync_all)
/// reads it: a name the Workspace has bound, or an ip and a port, with none naming the one the
/// Workspace reaches for by default. What is printed is the versions' hashes, one per line, the way
/// [`rola vcs-index ls-versions`](crate::vcs_index::cmd_vcs_index_ls_versions) prints them of the
/// index the run is inside — each hash one to hand to `rola vcs-index read-remote`, which works out
/// the number.
///
/// Nothing is changed: the Vault is asked and answers, and no object moves.
///
/// # Errors
///
/// As [`vcs-index ls-remote-variants`](crate::vcs_index::cmd_vcs_index_ls_remote_variants).
#[command(node = "vcs-index.ls-remote-versions", entry = EntryVcsIndexLsRemoteVersions)]
pub fn vcs_index_ls_remote_versions(
    args: EntryVcsIndexLsRemoteVersions,
) -> StateVcsIndexLsRemoteVersions {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateVcsIndexLsRemoteVersions::from(named)
}

/// The state of listing the other end's versions.
#[derive(Grouped, Wrap)]
pub struct StateVcsIndexLsRemoteVersions {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_vcs_index_ls_remote_versions(
    state: StateVcsIndexLsRemoteVersions,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let target = remote
        .get_ref()
        .vault_or_default(state.vault.unwrap_or_default())?;

    let name = current.get_ref().must_bind()?;
    let account = account_named(&name, Some(held), None)?;

    list_remote_hashes(held, &account, &target.to_string(), "versions")?.into()
}
