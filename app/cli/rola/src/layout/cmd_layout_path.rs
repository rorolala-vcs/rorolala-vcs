//! The `rola layout path` commands: which path names which entry.
//!
//! These write a Layout's content the way the library's own `Layout` does: a path names a `Uuid`,
//! and what is here is the three ways that naming changes. Nothing is checked beyond what the
//! Layout itself checks — binding a path to an entry that holds nothing is allowed — since this is
//! the layer for writing content by hand.

use librorolala::layout::LayoutPath;
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln},
    metadata::Description,
    picker::EntryPicker,
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

/// The `LayoutPath` `text` names, or the argument failure.
fn path_of(text: &str) -> Result<LayoutPath, Next> {
    LayoutPath::new(text).map_err(|error| failed(&error))
}

/// The `Uuid` `text` names, or the argument failure.
fn uuid_of(text: &str) -> Result<Uuid, Next> {
    use std::str::FromStr as _;

    Uuid::from_str(text).map_err(|_| ErrorLayoutArgument.into())
}

#[help(buffer)]
pub fn help_layout_path_create(_: EntryLayoutPathCreate, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_path.create_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutPathCreate)]
pub fn desc_layout_path_create() -> Description {
    t!("cmd_layout_path.create_description").to_string().into()
}

/// Makes a path name an entry
///
/// An entry is at one path, so naming it somewhere else moves it rather than putting it in two
/// places. The entry named need not hold anything yet: what a path names and what an entry holds
/// are written one at a time, and a half-made entry in between is a state a Layout allows.
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read, and the
/// path-not-allowed, path-already-taken or run-not-in-a-place failures otherwise.
#[command(node = "layout.path.create", entry = EntryLayoutPathCreate)]
pub fn layout_path_create(args: EntryLayoutPathCreate) -> Next {
    let picked = args
        .pick(&arg![LayoutOnlyFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, path, uuid) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutPathCreate {
        path,
        uuid,
        layout: flags.layout,
    }
    .into()
}

/// The state of binding a path.
#[derive(Grouped)]
pub struct StateLayoutPathCreate {
    /// The path to bind.
    path: String,
    /// The entry it is to name.
    uuid: String,
    /// The Layout to work on, when one was named.
    layout: Option<String>,
}

#[chain]
pub fn handle_layout_path_create(
    state: StateLayoutPathCreate,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Next {
    let StateLayoutPathCreate { path, uuid, layout } = state;

    let path = match path_of(&path) {
        Ok(path) => path,
        Err(next) => return next,
    };
    let id = match uuid_of(&uuid) {
        Ok(id) => id,
        Err(next) => return next,
    };
    let layout = match chosen_writable(workspace.get_ref(), vault.get_ref(), layout.as_deref()) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    if let Err(error) = layout.create_path(&path, id) {
        return failed(&error);
    }

    ResultLayoutContent {
        did: LayoutDid::Bound,
        what: format!("{} -> {id}", path.as_str()),
    }
    .into()
}

#[help(buffer)]
pub fn help_layout_path_remove(_: EntryLayoutPathRemove, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_path.remove_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutPathRemove)]
pub fn desc_layout_path_remove() -> Description {
    t!("cmd_layout_path.remove_description").to_string().into()
}

/// Makes a path name nothing
///
/// What is left is the entry the path named, at no path: a half-made entry, which the Layout
/// allows and `rola layout entries` reports.
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read, and the
/// path-not-there or run-not-in-a-place failures otherwise.
#[command(node = "layout.path.remove", entry = EntryLayoutPathRemove)]
pub fn layout_path_remove(args: EntryLayoutPathRemove) -> Next {
    let picked = args
        .pick(&arg![LayoutOnlyFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, path) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutPathRemove {
        path,
        layout: flags.layout,
    }
    .into()
}

/// The state of unbinding a path.
#[derive(Grouped)]
pub struct StateLayoutPathRemove {
    /// The path to unbind.
    path: String,
    /// The Layout to work on, when one was named.
    layout: Option<String>,
}

#[chain]
pub fn handle_layout_path_remove(
    state: StateLayoutPathRemove,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Next {
    let StateLayoutPathRemove { path, layout } = state;

    let path = match path_of(&path) {
        Ok(path) => path,
        Err(next) => return next,
    };
    let layout = match chosen_writable(workspace.get_ref(), vault.get_ref(), layout.as_deref()) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    if let Err(error) = layout.remove_path(&path) {
        return failed(&error);
    }

    ResultLayoutContent {
        did: LayoutDid::Unbound,
        what: path.as_str().to_owned(),
    }
    .into()
}

#[help(buffer)]
pub fn help_layout_path_move(_: EntryLayoutPathMove, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_path.move_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutPathMove)]
pub fn desc_layout_path_move() -> Description {
    t!("cmd_layout_path.move_description").to_string().into()
}

/// Moves the entry at one path to another
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read, and the
/// path-not-there, path-not-allowed or path-already-taken failures otherwise.
#[command(node = "layout.path.move", entry = EntryLayoutPathMove)]
pub fn layout_path_move(args: EntryLayoutPathMove) -> Next {
    let picked = args
        .pick(&arg![LayoutOnlyFlags])
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .pick_or_route(&arg![String], || ErrorLayoutArgument.into())
        .to_result();
    let (flags, from, to) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateLayoutPathMove {
        from,
        to,
        layout: flags.layout,
    }
    .into()
}

/// The state of moving an entry between paths.
#[derive(Grouped)]
pub struct StateLayoutPathMove {
    /// The path it is at.
    from: String,
    /// The path it is to be at.
    to: String,
    /// The Layout to work on, when one was named.
    layout: Option<String>,
}

#[chain]
pub fn handle_layout_path_move(
    state: StateLayoutPathMove,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Next {
    let StateLayoutPathMove { from, to, layout } = state;

    let from = match path_of(&from) {
        Ok(path) => path,
        Err(next) => return next,
    };
    let to = match path_of(&to) {
        Ok(path) => path,
        Err(next) => return next,
    };
    let layout = match chosen_writable(workspace.get_ref(), vault.get_ref(), layout.as_deref()) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    if let Err(error) = layout.move_path(&from, &to) {
        return failed(&error);
    }

    ResultLayoutContent {
        did: LayoutDid::Moved,
        what: format!("{} -> {}", from.as_str(), to.as_str()),
    }
    .into()
}
