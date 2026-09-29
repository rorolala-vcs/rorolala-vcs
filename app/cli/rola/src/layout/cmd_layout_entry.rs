//! The `rola layout entry` commands: what an entry holds.
//!
//! These write a Layout's content the way the library's own `Layout` does, one operation at a time:
//! an entry is a `Uuid` and the data it holds, and what is here is the three ways that data
//! changes. Nothing is checked beyond what the Layout itself checks — an entry that is already
//! there is refused by `create` and an entry that is not is refused by `update` — since this is the
//! layer for writing content by hand.

use std::str::FromStr as _;

use librorolala::layout::MutableData;
use librorolala::storage::Key;
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln},
    metadata::Description,
    picker::{EntryPicker, Pickable},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::layout::{
    ErrorLayoutArgument, LayoutDid, LayoutOnlyFlags, ResultLayoutContent, chosen_writable, failed,
};

/// The flags `rola layout entry create` and `update` take.
#[derive(Pickable)]
struct EntryFlags {
    /// The account the entry is held by; none when it is left out.
    #[arg(long)]
    owner: Option<String>,
    /// What the entry says about itself; nothing when it is left out.
    #[arg(long)]
    description: Option<String>,
    /// The Layout to work on; the one being worked in when none is named.
    #[arg(long)]
    layout: Option<String>,
}

/// The `Uuid` `text` names, or the argument failure.
fn uuid_of(text: &str) -> Result<Uuid, Next> {
    Uuid::from_str(text).map_err(|_| ErrorLayoutArgument.into())
}

/// The version `text` names, or the argument failure.
fn version_of(text: &str) -> Result<Key, Next> {
    Key::from_str(text).map_err(|_| ErrorLayoutArgument.into())
}

#[help(buffer)]
pub fn help_layout_entry_create(_: EntryLayoutEntryCreate, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_entry.create_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutEntryCreate)]
pub fn desc_layout_entry_create() -> Description {
    t!("cmd_layout_entry.create_description").to_string().into()
}

/// Makes an entry hold data
///
/// `UUID` is how the file is known upstream, `VERSION` the version it is at — written as
/// `blake3:<hex>` or as the digest alone. `--owner` names the account that holds it, `--description`
/// what it says about itself; leaving either out leaves it holding none and saying nothing.
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read,
/// [`ErrorLayoutExists`](crate::layout::ErrorLayoutExists) when the entry is already there, and the
/// run-not-in-a-place failure when the run is in neither a Workspace nor a Vault.
#[command(node = "layout.entry.create", entry = EntryLayoutEntryCreate)]
pub fn layout_entry_create(args: EntryLayoutEntryCreate) -> Next {
    let picked = args
        .pick(&arg![EntryFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, uuid, version) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutEntry {
        uuid,
        version,
        owner: flags.owner,
        description: flags.description,
        layout: flags.layout,
        update: false,
        remove: false,
    }
    .into()
}

#[help(buffer)]
pub fn help_layout_entry_update(_: EntryLayoutEntryUpdate, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_entry.update_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutEntryUpdate)]
pub fn desc_layout_entry_update() -> Description {
    t!("cmd_layout_entry.update_description").to_string().into()
}

/// Changes what an entry holds
///
/// The data is replaced whole, as the Layout keeps it: `--owner` and `--description` left out mean
/// the entry holds no account and says nothing, rather than that either is left as it was.
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read,
/// [`ErrorLayoutMissing`](crate::layout::ErrorLayoutMissing) when the entry is not there, and the
/// run-not-in-a-place failure when the run is in neither a Workspace nor a Vault.
#[command(node = "layout.entry.update", entry = EntryLayoutEntryUpdate)]
pub fn layout_entry_update(args: EntryLayoutEntryUpdate) -> Next {
    let picked = args
        .pick(&arg![EntryFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, uuid, version) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutEntry {
        uuid,
        version,
        owner: flags.owner,
        description: flags.description,
        layout: flags.layout,
        update: true,
        remove: false,
    }
    .into()
}

#[help(buffer)]
pub fn help_layout_entry_remove(_: EntryLayoutEntryRemove, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_entry.remove_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutEntryRemove)]
pub fn desc_layout_entry_remove() -> Description {
    t!("cmd_layout_entry.remove_description").to_string().into()
}

/// Drops an entry, and the path that named it
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read,
/// [`ErrorLayoutMissing`](crate::layout::ErrorLayoutMissing) when the entry is not there, and the
/// run-not-in-a-place failure when the run is in neither a Workspace nor a Vault.
#[command(node = "layout.entry.remove", entry = EntryLayoutEntryRemove)]
pub fn layout_entry_remove(args: EntryLayoutEntryRemove) -> Next {
    let picked = args
        .pick(&arg![LayoutOnlyFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, uuid) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutEntry {
        uuid,
        version: String::new(),
        owner: None,
        description: None,
        layout: flags.layout,
        remove: true,
        update: false,
    }
    .into()
}

/// The state of writing to an entry.
#[derive(Grouped)]
pub struct StateLayoutEntry {
    /// The entry's `Uuid`.
    uuid: String,
    /// The version it is to hold.
    version: String,
    /// The account it is to be held by.
    owner: Option<String>,
    /// What it is to say about itself.
    description: Option<String>,
    /// The Layout to work on, when one was named.
    layout: Option<String>,
    /// Whether the data is replaced rather than made.
    update: bool,
    /// Whether the entry is dropped.
    remove: bool,
}

#[chain]
pub fn handle_layout_entry(
    state: StateLayoutEntry,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Next {
    let StateLayoutEntry {
        uuid,
        version,
        owner,
        description,
        layout: named,
        update,
        remove,
    } = state;

    let id = match uuid_of(&uuid) {
        Ok(id) => id,
        Err(next) => return next,
    };

    let layout = match chosen_writable(workspace.get_ref(), vault.get_ref(), named.as_deref()) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let what = id.to_string();

    if remove {
        if let Err(error) = layout.remove_entry(id) {
            return failed(&error);
        }

        return ResultLayoutContent {
            did: LayoutDid::Removed,
            what,
        }
        .into();
    }

    let version = match version_of(&version) {
        Ok(version) => version,
        Err(next) => return next,
    };
    let data = MutableData::new(owner, *version.digest(), description.unwrap_or_default());

    let written = if update {
        layout.update_entry(id, data)
    } else {
        layout.create_entry(id, data)
    };
    if let Err(error) = written {
        return failed(&error);
    }

    ResultLayoutContent {
        did: if update {
            LayoutDid::Updated
        } else {
            LayoutDid::Created
        },
        what,
    }
    .into()
}
