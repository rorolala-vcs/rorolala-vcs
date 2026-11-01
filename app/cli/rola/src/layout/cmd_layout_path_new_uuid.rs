//! The `rola layout path new-uuid` command: giving a path's entry a new `Uuid`.
//!
//! What the path names changes, and the entry it named is dropped: an entry is its `Uuid`, so there
//! is nothing else for the old one to be. What the new entry holds is what the old one held, which
//! makes this the way to start a fresh entry from what one already is.
//!
//! Nothing beyond the Layout is touched. The working tree and the stores are keyed by the `Uuid` a
//! run reads from the Layout, so what they make of the new one is the next command's business.

use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{arg, buffer, chain, command, completion, help, metadata, r_eprintln, suggest},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::complete::{
    chosen_layout, filling_flag, flag_value, layout_paths, offer, positional, strip_written,
    typing_flag, workspace_layout_names,
};
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutMissing, LayoutDid, ResultLayoutContent, chosen_writable,
    failed, path_of, switch_entry,
};

#[help(buffer)]
pub fn help_layout_path_new_uuid(_: EntryLayoutPathNewUuid, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_path.new_uuid_help")).trim());
    ec.exit_code = crate::exit_codes::EC_HELP;
}

#[metadata(EntryLayoutPathNewUuid)]
pub fn desc_layout_path_new_uuid() -> Description {
    t!("cmd_layout_path.new_uuid_description")
        .to_string()
        .into()
}

/// Completes what `rola layout path new-uuid` can be given next.
///
/// The path is one the Layout names, read from the Layout the run would work on.
#[completion(EntryLayoutPathNewUuid)]
pub fn complete_layout_path_new_uuid(
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

    if positional(&ctx, "new-uuid") == 0 {
        offer(&ctx, layout_paths(&layout))
    } else {
        suggest!()
    }
}

/// Gives a path's entry a new `Uuid`
///
/// The entry the path named is dropped and one made in its place under a `Uuid` nobody has used, with
/// the same data. What was recorded for the old `Uuid` is not written again anywhere: the Vault's
/// Layout still has it if it was ever submitted, and the next `rola sync` reports it as one the Vault
/// holds and this Workspace does not.
///
/// # Errors
///
/// Renders [`ErrorLayoutArgument`] when an argument is missing or does not read,
/// [`ErrorLayoutMissing`] when the path names nothing or names an entry that holds nothing, and the
/// read-only or failed failures otherwise.
#[command(node = "layout.path.new-uuid", entry = EntryLayoutPathNewUuid)]
pub fn layout_path_new_uuid(args: EntryLayoutPathNewUuid) -> Next {
    let picked = args
        .pick(crate::layout::ARG_LAYOUT)
        .pick_or_route(arg![String], || ErrorLayoutArgument.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (layout, path, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "layout path new-uuid") {
        return next;
    }

    StateLayoutPathNewUuid { path, layout }.into()
}

/// The state of renewing the `Uuid` a path names.
#[derive(Grouped)]
pub struct StateLayoutPathNewUuid {
    /// The path whose entry is to be renewed.
    path: String,
    /// The Layout to work on, when one was named.
    layout: Option<String>,
}

#[chain]
pub fn handle_layout_path_new_uuid(
    state: StateLayoutPathNewUuid,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
) -> Next {
    let StateLayoutPathNewUuid { path, layout } = state;

    let path = match path_of(&path) {
        Ok(path) => path,
        Err(next) => return next,
    };
    let layout = match chosen_writable(workspace.get_ref(), vault.get_ref(), layout.as_deref()) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(old) = layout.id_of(&path) else {
        return ErrorLayoutMissing.into();
    };
    let Some(data) = layout.entry(old) else {
        return ErrorLayoutMissing.into();
    };

    let id = Uuid::new_v4();
    if let Err(error) = switch_entry(&layout, &path, id, data) {
        return failed(&error);
    }

    ResultLayoutContent {
        did: LayoutDid::UuidNew,
        what: format!("{} -> {id}", path.as_str()),
    }
    .into()
}
