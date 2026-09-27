//! The `rola vcs-index read-remote` command: read one index object from the other end.
//!
//! It is [`read`](crate::vcs_index::cmd_vcs_index_read) asked of the other end of an exchange:
//! where that reads the index the run is inside, this reaches the Vault and reads what *its* index
//! holds. A Creator or a Message is printed as its text. A Variant is printed the way `ls-variants`
//! prints it, and a Version the way `ls-versions` prints it — its number traced on the Vault, since
//! the chain is there and not here.

use librorolala::protocol::ActionError;
use librorolala::storage::Key;
use librorolala::vcs::{UNKNOWN_VERSION, VCSIndexObject};
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use std::str::FromStr as _;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::EC_HELP;
use crate::keys::account_named;
use crate::vcs_index::cmd_vcs_index_read::{ResultVcsIndexRead, view_of};
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexHash, ErrorVcsIndexNotFound, parse_hash, read_remote,
};

#[help(buffer)]
pub fn help_vcs_index_read_remote(_: EntryVcsIndexReadRemote, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_read_remote.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexReadRemote)]
pub fn desc_vcs_index_read_remote() -> Description {
    t!("vcs_index_read_remote.description").to_string().into()
}

/// Reads one index object from the index at the other end
///
/// `HASH` names the object, as [`read`](crate::vcs_index::cmd_vcs_index_read) reads it. `VAULT`
/// names the Vault to reach, as `rola vcs-index sync-all` reads it: a name the Workspace has bound,
/// or an ip and a port, with none naming the one the Workspace reaches for by default. A Creator or
/// a Message is printed as its text; a Variant the way `ls-variants` prints it, and a Version the
/// way `ls-versions` prints it.
///
/// Nothing is changed: the Vault is asked and answers, and no object moves.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexHash`] when the hash does not read, [`ErrorVcsIndexNotFound`] when no
/// object is held under it, and — as
/// [`vcs-index ls-remote-variants`](crate::vcs_index::cmd_vcs_index_ls_remote_variants) — whatever
/// the exchange reports.
#[command(node = "vcs-index.read-remote", entry = EntryVcsIndexReadRemote)]
pub fn vcs_index_read_remote(args: EntryVcsIndexReadRemote) -> Next {
    // Picking cannot fail: no positions are `None`-able but the list itself, which is empty.
    let words: Vec<String> = args.pick(&arg![Vec<String>]).unwrap_or_default();
    let (vault, words) = with_vault(words);

    let Some(hash) = words.into_iter().next() else {
        return ErrorVcsIndexArgument {
            argument: "HASH".to_owned(),
        }
        .into();
    };

    StateVcsIndexReadRemote { hash, vault }.into()
}

/// The hash and the Vault to reach, from the words a run gave.
///
/// A hash reads as one and a Vault does not, so the last word is the Vault when it is not a hash —
/// the same reading [`storage sync-hashes`](crate::storage::cmd_storage_sync_hashes) gives its
/// trailing Vault, so the two are named the same way.
fn with_vault(mut words: Vec<String>) -> (Option<String>, Vec<String>) {
    let vault = words
        .last()
        .filter(|word| Key::from_str(word).is_err())
        .cloned();

    if vault.is_some() {
        words.pop();
    }

    (vault, words)
}

/// The state a remote read starts in: the hash it names, and the Vault to reach.
#[derive(Grouped)]
pub struct StateVcsIndexReadRemote {
    /// The hash of the object to read.
    hash: String,
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_vcs_index_read_remote(
    state: StateVcsIndexReadRemote,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    // The hash is checked here so that a word that is not one is refused in this run's words rather
    // than carried to the Vault; what crosses is the hash as it was given.
    if parse_hash(&state.hash).is_none() {
        return ErrorVcsIndexHash { hash: state.hash }.into();
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

    let remote_read = read_remote(held, &account, &target.to_string(), &state.hash);

    // The same answer the local read gives when nothing is stored under a hash: the read was
    // finished and had nothing to hand back.
    if matches!(&remote_read, Err(ActionError::MissingObject)) {
        return ErrorVcsIndexNotFound { hash: state.hash }.into();
    }

    let object = remote_read?;

    // The number was traced on the Vault and carried in the Version; a Version that came back
    // without one never had a chain to trace.
    let number = match &object {
        VCSIndexObject::Version(version) if version.version_num() != UNKNOWN_VERSION => {
            Some(version.version_num())
        }
        _ => None,
    };

    ResultVcsIndexRead {
        object: view_of(object, number),
    }
    .into()
}
