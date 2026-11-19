//! The `rola storage create` command: make a directory a store.
//!
//! It is `rola create --storage` reached from the storage namespace: the same checks and the same
//! result, so a person who is already thinking in stores does not have to leave the namespace to
//! make one.

use std::path::PathBuf;

use mingling::{
    Routable, ShellContext, Suggest,
    macros::{arg, buffer, command, completion, help, metadata, r_eprintln, routeify, suggest},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::cmd_create::{ErrorCreatePathNotProvided, StateCreateStoragePath};
use crate::complete::positional;
use crate::exit_codes::EC_HELP;

#[help(buffer)]
pub fn help_storage_create(_: EntryStorageCreate, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage_create.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorageCreate)]
pub fn desc_storage_create() -> Description {
    t!("storage_create.description").to_string().into()
}

/// Completes what `rola storage create` can be given next.
#[completion(EntryStorageCreate)]
pub fn complete_storage_create(ctx: ShellContext) -> Suggest {
    if positional(&ctx, "storage create") == 0 {
        Suggest::file_comp()
    } else {
        suggest!()
    }
}

#[command(node = "storage.create", entry = EntryStorageCreate, routeify)]
pub fn storage_create(args: EntryStorageCreate) -> Next {
    let path = args
        .pick_or_route(arg![PathBuf], || ErrorCreatePathNotProvided.to_chain())
        .to_result()?;

    StateCreateStoragePath::from(path).into()
}
