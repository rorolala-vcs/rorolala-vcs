//! The `rola entries` command: the paths a Layout names, said as a list to read and to pipe.
//!
//! It is [`rola layout entries`](crate::layout::cmd_layout_entries) in the one shape that is
//! wanted most of the time — a path a line, nothing else — which is what a `tree`, a script, or
//! anything else that reads one path a line is given. Everything else an entry holds is still one
//! command over, since that is what the other is for.
//!
//! What it reads is the Layout being worked in, or — with `--remote` — the Vault's own, as the
//! copy a `rola layout fetch` left in the Workspace. Nothing is fetched for it: a run that has not
//! fetched is told there is no copy rather than quietly reaching for one, so a query answers from
//! what is already at hand.

use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, renderer, routeify,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_LAYOUT, EC_HELP};
use crate::failure::failure;
use crate::format::ResFormat;
use crate::layout::cmd_layout_entries::StateLayoutEntries;
use crate::layout::{ErrorLayoutArgument, ErrorLayoutFailed};

/// What `rola entries` draws: the path alone, one a line.
const PATH_TEMPLATE: &str = "{{ entries.path }}";

/// Read the Vault's own Layout instead of the one being worked in.
const ARG_REMOTE: PickerArg<'static, Flag> = arg![remote: Flag];

#[help(buffer)]
pub fn help_entries(_: EntryEntries, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("entries.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryEntries)]
pub fn desc_entries() -> Description {
    t!("entries.description").to_string().into()
}

/// Completes what `rola entries` can be given next.
///
/// The command names no argument of its own — what it lists is whatever the run is in — so the
/// only thing to answer is a word that starts a flag.
#[completion(EntryEntries)]
pub fn complete_entries(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_REMOTE: t!("entries.complete.remote"),
            },
        );
    }

    suggest!()
}

/// Lists the paths a Layout names, one a line
///
/// The Layout being worked in is what is read. `--remote` reads the Vault's own Layout instead —
/// the one that Layout tracks — as the copy `rola layout fetch` left in the Workspace. Nothing is
/// fetched for it, so a run that has not fetched is told there is no copy rather than reaching for
/// one: what a query reads is what is at hand.
///
/// A path is listed whether or not the file is there, and one an entry is at no path is left out,
/// since a line is a path or it is nothing. What each entry is held by, at what version, and
/// whether the Vault has deprecated it are [`rola layout entries`](crate::layout::cmd_layout_entries)'s
/// to say.
///
/// # Errors
///
/// Renders the run-not-in-a-workspace failure when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when there is no Layout to read, [`ErrorEntriesNoTrack`] when the
/// Layout being worked in tracks no Vault, [`ErrorLayoutNotCached`] when the Vault's Layout has not
/// been fetched here, and [`ErrorLayoutFailed`] when what is there cannot be read.
///
/// [`ErrorLayoutNotCached`]: crate::layout::ErrorLayoutNotCached
#[command(node = "entries", entry = EntryEntries)]
pub fn entries(args: EntryEntries, format: &mut ResFormat) -> Next {
    // Picking flags cannot fail: a flag that is absent is `None`, not an error.
    let remote = args.pick(&ARG_REMOTE).unwrap();

    format.default_template(PATH_TEMPLATE);

    StateEntries {
        remote: matches!(remote, Flag::Active),
    }
    .into()
}

/// The state a listing of paths starts in.
///
/// It is `Copy` because it is one flag and nothing else: a state the framework hands on is worth
/// keeping cheap where it can be, and a marker for which Layout to read is not something to own.
#[derive(Grouped, Clone, Copy)]
pub struct StateEntries {
    /// Whether the Vault's own Layout is read rather than the one being worked in.
    remote: bool,
}

/// Names the Layout to list, which is what `--remote` is about.
///
/// The listing itself is [`rola layout entries`](crate::layout::cmd_layout_entries)'s, reached by
/// handing it the state it starts from: what is settled here is only which Layout that is, so the
/// two commands cannot drift apart in what they list or in how a template draws it.
#[chain(routeify)]
pub fn handle_entries(
    state: StateEntries,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Next {
    let StateEntries { remote: asked } = state;

    if !asked {
        return StateLayoutEntries::every(None).into();
    }

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let layouts = held.layouts();
    let name = match layouts.current() {
        Ok(Some(name)) => name,
        Ok(None) => return ErrorLayoutArgument.into(),
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };
    let Some(track) = (match layouts.track(&name) {
        Ok(track) => track,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    }) else {
        return ErrorEntriesNoTrack.into();
    };

    // The name is checked against what the Workspace has bound, so a Vault that was renamed out
    // from under the Layout is said rather than read as a copy that happens to be keyed by it.
    let vault = remote.get_ref().name_or_default(track)?;

    StateLayoutEntries::every(Some(format!("{VAULT_LAYOUT_NAME}@{vault}"))).into()
}

/// Error: the Layout being worked in tracks no Vault.
#[derive(Grouped)]
pub struct ErrorEntriesNoTrack;

impl Failure for ErrorEntriesNoTrack {
    fn name(&self) -> &'static str {
        "error_entries_no_track"
    }

    fn reason(&self) -> String {
        t!("entries.err_no_track").trim().to_string()
    }
}

failure!(ErrorEntriesNoTrack);

#[renderer(buffer)]
pub fn render_error_entries_no_track(error: ErrorEntriesNoTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("entries.err_no_track_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT;
}
