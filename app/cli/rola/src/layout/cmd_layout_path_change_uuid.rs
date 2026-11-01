//! The `rola layout path change-uuid` command: making a path name another entry.
//!
//! What changes is which `Uuid` the path names; the entry it named is dropped, since an entry is its
//! `Uuid`. Nothing else is touched: the index, the stores and the working tree know nothing of a
//! Layout's paths, so a command here changes the Layout as written and no more.
//!
//! The entry the path is to name is one no path names yet. Making it the entry of another path would
//! leave that path naming an entry that is no longer there, which is not a change this command makes.

use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, renderer, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::{
    chosen_layout, filling_flag, flag_value, layout_orphan_uuids, layout_paths, offer, positional,
    strip_written, typing_flag, workspace_layout_names,
};
use crate::exit_codes::{EC_ALREADY_EXIST, EC_ERR_LAYOUT_ARGUMENT, EC_NOT_EXIST};
use crate::failure::failure;
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutMissing, LayoutDid, ResultLayoutContent, chosen_writable,
    failed, path_of, switch_entry, uuid_of,
};

/// Error: the `Uuid` given is the one the path already names.
#[derive(Grouped)]
pub struct ErrorLayoutSameUuid {
    /// The `Uuid` the path names already.
    uuid: String,
}

impl Failure for ErrorLayoutSameUuid {
    fn name(&self) -> &'static str {
        "error_layout_same_uuid"
    }

    fn reason(&self) -> String {
        t!("cmd_layout_path.err_same_uuid", uuid = self.uuid)
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutSameUuid);

#[renderer(buffer)]
pub fn render_error_layout_same_uuid(error: ErrorLayoutSameUuid, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout_path.err_same_uuid_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: the `Uuid` given is already named by a path.
#[derive(Grouped)]
pub struct ErrorLayoutUuidNamed {
    /// The `Uuid` that is named already.
    uuid: String,
    /// The path that names it.
    path: String,
}

impl Failure for ErrorLayoutUuidNamed {
    fn name(&self) -> &'static str {
        "error_layout_uuid_named"
    }

    fn reason(&self) -> String {
        t!(
            "cmd_layout_path.err_uuid_named",
            uuid = self.uuid,
            path = self.path
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorLayoutUuidNamed);

#[renderer(buffer)]
pub fn render_error_layout_uuid_named(error: ErrorLayoutUuidNamed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout_path.err_uuid_named_help").trim())
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: the Layout holds nothing by the `Uuid` given.
#[derive(Grouped)]
pub struct ErrorLayoutUuidMissing {
    /// The `Uuid` the Layout holds nothing by.
    uuid: String,
}

impl Failure for ErrorLayoutUuidMissing {
    fn name(&self) -> &'static str {
        "error_layout_uuid_missing"
    }

    fn reason(&self) -> String {
        t!("cmd_layout_path.err_uuid_missing", uuid = self.uuid)
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutUuidMissing);

#[renderer(buffer)]
pub fn render_error_layout_uuid_missing(error: ErrorLayoutUuidMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout_path.err_uuid_missing_help").trim())
    );
    ec.exit_code = EC_NOT_EXIST;
}

#[help(buffer)]
pub fn help_layout_path_change_uuid(_: EntryLayoutPathChangeUuid, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_path.change_uuid_help")).trim());
    ec.exit_code = crate::exit_codes::EC_HELP;
}

#[metadata(EntryLayoutPathChangeUuid)]
pub fn desc_layout_path_change_uuid() -> Description {
    t!("cmd_layout_path.change_uuid_description")
        .to_string()
        .into()
}

/// Completes what `rola layout path change-uuid` can be given next.
///
/// The path is one the Layout names and the `Uuid` is one it holds at no path, since what the command
/// does is give the path to an entry no path names.
#[completion(EntryLayoutPathChangeUuid)]
pub fn complete_layout_path_change_uuid(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Suggest {
    if filling_flag(&ctx, &crate::layout::ARG_LAYOUT) {
        return offer(&ctx, workspace_layout_names(workspace.get_ref()));
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                crate::layout::ARG_LAYOUT: t!("cmd_layout_path.complete.layout"),
            },
        );
    }

    let named = flag_value(&ctx, &crate::layout::ARG_LAYOUT);
    let Some(layout) = chosen_layout(workspace.get_ref(), vault.get_ref(), named.as_deref()) else {
        return suggest!();
    };

    match positional(&ctx, "change-uuid") {
        0 => offer(&ctx, layout_paths(&layout)),
        1 => offer(&ctx, layout_orphan_uuids(&layout)),
        _ => suggest!(),
    }
}

/// Makes a path name another entry
///
/// The entry the path named is dropped and the entry named here takes its place, in one write. What
/// the new entry holds is what the Layout already holds for it, so this is a way of putting an entry
/// that has no path onto one.
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read,
/// [`ErrorLayoutMissing`] when the path names nothing, [`ErrorLayoutSameUuid`] when it already names
/// this entry, [`ErrorLayoutUuidNamed`] when another path does, [`ErrorLayoutUuidMissing`] when the
/// Layout holds nothing by it, and the read-only or failed failures otherwise.
#[command(node = "layout.path.change-uuid", entry = EntryLayoutPathChangeUuid)]
pub fn layout_path_change_uuid(args: EntryLayoutPathChangeUuid) -> Next {
    let picked = args
        .pick(crate::layout::ARG_LAYOUT)
        .pick_or_route(arg![String], || ErrorLayoutArgument.to_chain())
        .pick_or_route(arg![String], || ErrorLayoutArgument.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (layout, path, uuid, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "layout path change-uuid") {
        return next;
    }

    StateLayoutPathChangeUuid { path, uuid, layout }.into()
}

/// The state of changing what a path names.
#[derive(Grouped)]
pub struct StateLayoutPathChangeUuid {
    /// The path whose entry is to change.
    path: String,
    /// The entry it is to name.
    uuid: String,
    /// The Layout to work on, when one was named.
    layout: Option<String>,
}

#[chain]
pub fn handle_layout_path_change_uuid(
    state: StateLayoutPathChangeUuid,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Next {
    let StateLayoutPathChangeUuid { path, uuid, layout } = state;

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

    let Some(old) = layout.id_of(&path) else {
        return ErrorLayoutMissing.into();
    };
    if old == id {
        return ErrorLayoutSameUuid {
            uuid: id.to_string(),
        }
        .into();
    }
    if let Some(named) = layout.path_of(id) {
        return ErrorLayoutUuidNamed {
            uuid: id.to_string(),
            path: named.as_str().to_owned(),
        }
        .into();
    }
    let Some(data) = layout.entry(id) else {
        return ErrorLayoutUuidMissing {
            uuid: id.to_string(),
        }
        .into();
    };

    if let Err(error) = switch_entry(&layout, &path, id, data) {
        return failed(&error);
    }

    ResultLayoutContent {
        did: LayoutDid::UuidChanged,
        what: format!("{} -> {id}", path.as_str()),
    }
    .into()
}
