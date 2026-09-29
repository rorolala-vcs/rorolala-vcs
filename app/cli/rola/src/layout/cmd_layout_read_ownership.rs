//! The `rola layout read-ownership` command: who holds one entry, from the fetched copy.
//!
//! Nothing is reached for here: what is read is the copy a
//! [`fetch`](crate::layout::cmd_layout_fetch) wrote, so a run with no copy — or one whose copy does
//! not name the entry — is told to fetch rather than left to reach a Vault that may be elsewhere.

use librorolala::layout::Layout;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutFailed, ErrorLayoutNotCached, ErrorOwnershipUnknown,
    readonly_layout_dir, vault_and_uuid,
};

#[help(buffer)]
pub fn help_layout_read_ownership(_: EntryLayoutReadOwnership, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_read_ownership.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutReadOwnership)]
pub fn desc_layout_read_ownership() -> Description {
    t!("cmd_layout_read_ownership.description")
        .to_string()
        .into()
}

/// Reads who holds an entry, from the Vault's fetched copy
///
/// `VAULT` names the Vault whose copy is read, as
/// [`rola layout fetch`](crate::layout::cmd_layout_fetch) reads it. Naming none reads the copy of
/// the one the Workspace reaches for by default. `UUID` is the `Uuid` of the entry to read.
///
/// Nothing is reached for: what is answered comes from the copy under
/// `.rola/cache/readonly-layouts/`, so it says what the Vault held when it was last fetched.
///
/// # Errors
///
/// Renders [`ErrorShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorRemoteVault`] when no copy could be named, [`ErrorLayoutArgument`] when the arguments
/// name no entry, [`ErrorLayoutNotCached`] when the Vault's Layout was never fetched,
/// [`ErrorOwnershipUnknown`] when the copy names no such entry, and [`ErrorLayoutFailed`] when a
/// copy that is there cannot be read.
///
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
#[command(node = "layout.read-ownership", entry = EntryLayoutReadOwnership)]
pub fn layout_read_ownership(args: EntryLayoutReadOwnership) -> Next {
    let words: Vec<String> = args.pick(&arg![Vec<String>]).unwrap_or_default();

    let Some((vault, id)) = vault_and_uuid(&words) else {
        return ErrorLayoutArgument.into();
    };

    StateLayoutReadOwnership { vault, id }.into()
}

/// The state of reading who holds an entry.
#[derive(Grouped)]
pub struct StateLayoutReadOwnership {
    /// The Vault whose copy is read, or nothing when the Workspace's own choice is read.
    vault: Option<String>,
    /// The entry to read.
    id: Uuid,
}

#[chain(routeify)]
pub fn handle_layout_read_ownership(
    state: StateLayoutReadOwnership,
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
    let dir = readonly_layout_dir(held, &name);

    if !dir.is_dir() {
        return ErrorLayoutNotCached { vault: name }.into();
    }

    // Opening a copy makes one if it is not there, so what is not a directory is asked about
    // first: a copy that was never fetched is not a copy with nothing in it.
    let layout = match Layout::open(&dir) {
        Ok(layout) => layout,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    let uuid = state.id.to_string();

    match layout.entry(state.id) {
        None => ErrorOwnershipUnknown { uuid }.into(),
        Some(data) => ResultLayoutOwnership {
            owner: data.owner().map(str::to_owned),
            uuid,
        }
        .into(),
    }
}

/// Result: who holds one entry was read.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutOwnership {
    /// The entry that was read.
    uuid: String,
    /// The account that holds it, or nothing when nobody does.
    owner: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_layout_ownership(result: ResultLayoutOwnership) {
    let uuid = result.uuid;
    let said = result.owner.map_or_else(
        || {
            t!(
                "cmd_layout_read_ownership.result_unowned",
                uuid = uuid.as_str()
            )
        },
        |owner| {
            t!(
                "cmd_layout_read_ownership.result_owner",
                uuid = uuid.as_str(),
                owner = owner
            )
        },
    );

    r_println!("{}", said.trim());
}
