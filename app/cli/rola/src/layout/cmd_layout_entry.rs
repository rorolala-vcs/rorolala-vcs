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
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{arg, buffer, chain, command, completion, help, metadata, r_eprintln, suggest},
    metadata::Description,
    picker::{EntryPicker, PickerArg},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVCSIndex, ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::complete::{
    IndexObject, chosen_layout, filling_flag, flag_value, index_hashes, layout_uuids, offer,
    positional, strip_written, typing_flag, workspace_layout_names,
};
use crate::exit_codes::EC_HELP;
use crate::keys::account_names;
use crate::layout::{ErrorLayoutArgument, LayoutDid, ResultLayoutContent, chosen_writable, failed};

/// The account the entry is held by; none when it is left out.
const ARG_OWNER: PickerArg<'static, Option<String>> = arg![owner: Option<String>];

/// What the entry says about itself; nothing when it is left out.
const ARG_DESCRIPTION: PickerArg<'static, Option<String>> = arg![description: Option<String>];

/// The Layout to work on; the one being worked in when none is named.
const ARG_LAYOUT: PickerArg<'static, Option<String>> = arg![layout: Option<String>];

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

/// Completes what `rola layout entry create` can be given next.
#[completion(EntryLayoutEntryCreate)]
pub fn complete_layout_entry_create(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    complete_entry_written(
        &ctx,
        false,
        workspace.get_ref(),
        vault.get_ref(),
        index.get_ref(),
    )
}

/// Completes what `rola layout entry update` can be given next.
#[completion(EntryLayoutEntryUpdate)]
pub fn complete_layout_entry_update(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    complete_entry_written(
        &ctx,
        true,
        workspace.get_ref(),
        vault.get_ref(),
        index.get_ref(),
    )
}

/// Completes what `rola layout entry remove` can be given next.
///
/// What is named is an entry the Layout holds, and the only flag is which Layout to read it from.
#[completion(EntryLayoutEntryRemove)]
pub fn complete_layout_entry_remove(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Suggest {
    if filling_flag(&ctx, &ARG_LAYOUT) {
        return offer(&ctx, workspace_layout_names(workspace.get_ref()));
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_LAYOUT: t!("cmd_layout_entry.complete.layout"),
            },
        );
    }

    if positional(&ctx, "remove") == 0 {
        entry_uuids(&ctx, workspace.get_ref(), vault.get_ref())
    } else {
        suggest!()
    }
}

/// The completion `layout entry create` and `update` share, told apart by whether the entry named
/// has to be one that is already there.
///
/// The `Uuid` is what the entry is known by upstream: `update` changes one that exists, so those are
/// offered, while `create` makes a new one and has nothing to offer. The version is an index object
/// either way. `--owner` names an account, `--description` is the caller's own words, and `--layout`
/// picks which Layout to read.
fn complete_entry_written(
    ctx: &ShellContext,
    update: bool,
    workspace: &ResWorkspace,
    vault: &ResVault,
    index: &ResVCSIndex,
) -> Suggest {
    if filling_flag(ctx, &ARG_LAYOUT) {
        return offer(ctx, workspace_layout_names(workspace));
    }

    if filling_flag(ctx, &ARG_OWNER) {
        return offer(ctx, account_names(workspace.as_ref(), vault.as_ref()));
    }

    if filling_flag(ctx, &ARG_DESCRIPTION) {
        return suggest!();
    }

    if typing_flag(ctx) {
        return strip_written(
            ctx,
            suggest! {
                ARG_OWNER: t!("cmd_layout_entry.complete.owner"),
                ARG_DESCRIPTION: t!("cmd_layout_entry.complete.description"),
                ARG_LAYOUT: t!("cmd_layout_entry.complete.layout"),
            },
        );
    }

    match positional(ctx, if update { "update" } else { "create" }) {
        0 if update => entry_uuids(ctx, workspace, vault),
        1 => offer(ctx, index_hashes(index.as_ref(), IndexObject::Version)),
        _ => suggest!(),
    }
}

/// The entries the Layout a run would work on holds, when there is one to read.
fn entry_uuids(ctx: &ShellContext, workspace: &ResWorkspace, vault: &ResVault) -> Suggest {
    let named = flag_value(ctx, &ARG_LAYOUT);
    let Some(layout) = chosen_layout(workspace, vault, named.as_deref()) else {
        return suggest!();
    };

    offer(ctx, layout_uuids(&layout))
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
        .pick(&ARG_OWNER)
        .pick(&ARG_DESCRIPTION)
        .pick(&ARG_LAYOUT)
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (owner, description, layout, uuid, version) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutEntry {
        uuid,
        version,
        owner,
        description,
        layout,
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
        .pick(&ARG_OWNER)
        .pick(&ARG_DESCRIPTION)
        .pick(&ARG_LAYOUT)
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (owner, description, layout, uuid, version) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutEntry {
        uuid,
        version,
        owner,
        description,
        layout,
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
        .pick(&crate::layout::ARG_LAYOUT)
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (layout, uuid) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutEntry {
        uuid,
        version: String::new(),
        owner: None,
        description: None,
        layout,
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
