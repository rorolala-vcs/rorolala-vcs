//! The `rola layout ls-ownership` command: who holds every entry, from the fetched copy.
//!
//! What [`read-ownership`](crate::layout::cmd_layout_read_ownership) answers about one entry, this
//! answers about all of them at once: every entry the copy names, with whoever holds it. Nothing is
//! reached for, so a run with no copy is told to fetch.

use librorolala::layout::Layout;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::complete::{offer, positional, typing_flag, vault_names};
use crate::exit_codes::EC_HELP;
use crate::layout::{ErrorLayoutFailed, ErrorLayoutNotCached, readonly_layout_dir};

#[help(buffer)]
pub fn help_layout_ls_ownership(_: EntryLayoutLsOwnership, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_ls_ownership.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutLsOwnership)]
pub fn desc_layout_ls_ownership() -> Description {
    t!("cmd_layout_ls_ownership.description").to_string().into()
}

/// Completes what `rola layout ls-ownership` can be given next.
///
/// The one word is the Vault whose copy is read, so the names the Workspace has bound are what is
/// offered; naming none reads the one it reaches for.
#[completion(EntryLayoutLsOwnership)]
pub fn complete_layout_ls_ownership(
    ctx: ShellContext,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "ls-ownership") != 0 {
        return suggest!();
    }

    offer(&ctx, vault_names(remote.get_ref()))
}

/// Lists who holds each entry, from the Vault's fetched copy
///
/// `VAULT` names the Vault whose copy is read, as
/// [`rola layout fetch`](crate::layout::cmd_layout_fetch) reads it. Naming none reads the copy of
/// the one the Workspace reaches for by default.
///
/// Nothing is reached for: what is listed comes from the copy under `.rola/cache/readonly-layouts/`,
/// so it says what the Vault held when it was last fetched. One entry a line: its `Uuid`, and the
/// account that holds it, or that nobody does.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorRemoteVault`] when no copy could be named, [`ErrorLayoutNotCached`] when the Vault's
/// Layout was never fetched, and [`ErrorLayoutFailed`] when a copy that is there cannot be read.
///
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
#[command(node = "layout.ls-ownership", entry = EntryLayoutLsOwnership)]
pub fn layout_ls_ownership(args: EntryLayoutLsOwnership) -> StateLayoutLsOwnership {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is read.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateLayoutLsOwnership::from(named)
}

/// The state of listing who holds each entry.
///
/// Which Vault's copy is read is the whole of what is said: naming none reads the one the
/// Workspace reaches for by default.
#[derive(Grouped, Wrap)]
pub struct StateLayoutLsOwnership {
    /// The Vault whose copy is read, or nothing when the Workspace's own choice is read.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_layout_ls_ownership(
    state: StateLayoutLsOwnership,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let name = remote
        .get_ref()
        .name_or_default(state.vault.unwrap_or_default())?;
    let dir = readonly_layout_dir(held, &name, VAULT_LAYOUT_NAME);

    if !dir.is_dir() {
        return ErrorLayoutNotCached {
            layout: VAULT_LAYOUT_NAME.to_owned(),
            vault: name,
        }
        .into();
    }

    // Opening a copy makes one if it is not there, so what is not a directory is asked about
    // first: a copy that was never fetched is not a copy with nothing in it.
    let layout = match Layout::open(&dir) {
        Ok(layout) => layout,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    // What a Layout answers with is in no order a reader can rely on, so the entries are held in
    // the order a listing is read in: by the `Uuid` each is known by.
    let mut owners: Vec<OwnershipItem> = layout
        .entries()
        .into_iter()
        .map(|(id, data)| OwnershipItem {
            uuid: id.to_string(),
            owner: data.owner().map(str::to_owned),
        })
        .collect();
    owners.sort_by(|left, right| left.uuid.cmp(&right.uuid));

    ResultLayoutOwnerships { owners }.into()
}

/// One entry, as `ls-ownership` shows it.
#[derive(Serialize)]
pub struct OwnershipItem {
    /// The `Uuid` the entry is known by.
    uuid: String,
    /// The account that holds it, or nothing when nobody does.
    owner: Option<String>,
}

/// Result: who holds each entry was listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutOwnerships {
    /// Each entry, in `Uuid` order.
    owners: Vec<OwnershipItem>,
}

#[renderer(buffer)]
pub fn render_result_layout_ownerships(result: ResultLayoutOwnerships) {
    for item in &result.owners {
        let uuid = item.uuid.as_str();
        let said = item.owner.as_deref().map_or_else(
            || t!("cmd_layout_ls_ownership.result_unowned", uuid = uuid),
            |owner| {
                t!(
                    "cmd_layout_ls_ownership.result_owner",
                    uuid = uuid,
                    owner = owner
                )
            },
        );

        r_println!("{}", said.trim());
    }
}
