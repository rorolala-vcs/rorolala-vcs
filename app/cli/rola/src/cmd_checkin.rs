//! The `rola checkin` command: bringing a `Uuid` the Vault holds into the Layout being worked in.
//!
//! A `Uuid` enters a Layout by being tracked and enters the Vault's Layout by going up, which
//! leaves what only the Vault holds out of reach: a sync names it rather than bringing it, since a
//! `Uuid` is not something a sync may invent a local path for. This is how one is asked for by
//! name: each reference — a remote logical path or a `Uuid` — is paired with the local path it is
//! to be named by, and the content its version names is put there.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates,
// so a lint allowed on the handler below is reported as defined twice. The allow lives here,
// where it covers the one signature that needs it.
//
// The handler's parameters are the resources the framework injects, so how many of them there are
// is what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::trivially_copy_pass_by_ref, clippy::too_many_arguments)]

use std::path::Path;
use std::str::FromStr as _;

use librorolala::daemon::{action_fetch_layout, action_sync_index_all_async};
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::protocol::ActionError;
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResCurrentRemoteVault, ResOffline, ResProgressSetting, ResRorolalaStorage, ResVCSIndex,
    ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::checkout::remember;
use crate::exit_codes::{EC_ERR_CHECKIN, EC_ERR_CHECKIN_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::fetch::{self, Sources};
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutNotCached, failed,
    readonly_layout_dir,
};
use crate::progress::Reporting;
use crate::sync;
use crate::vcs_index::{ErrorVcsIndexNoIndex, runtime as index_runtime};

/// The flags `rola checkin` takes.
#[derive(Pickable)]
struct CheckinFlags {
    /// Where each reference is to be named here, one for each reference, in the order they are
    /// named.
    #[arg(long)]
    to_local: Vec<String>,
}

#[help(buffer)]
pub fn help_checkin(_: EntryCheckin, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("checkin.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryCheckin)]
pub fn desc_checkin() -> Description {
    t!("checkin.description").to_string().into()
}

/// Brings what the Vault holds but this Layout does not into it
///
/// Each reference is a path in the Vault's Layout or a `Uuid`, and `--to-local` says where that one
/// is to be named here: the two lists are paired by position and have to be the same length. What
/// the reference names has to be something this Layout does not already hold, and where it is to
/// land has to be somewhere nothing is: a name this Layout already gives, or a file already at the
/// path, is refused rather than written over.
///
/// The version the Vault holds is what comes with it — its content is taken out of the store and
/// put at the path — and what the Layout then says is that version and the holder the Vault names.
///
/// # Errors
///
/// Renders [`ErrorCheckinNoTrack`] when the Layout being worked in tracks no Vault,
/// [`ErrorCheckinArgument`] when the references and `--to-local` do not line up, and
/// [`ErrorCheckinUnknown`], [`ErrorCheckinTaken`] or [`ErrorCheckinKnown`] when one reference does
/// not name something that can be brought in.
#[command(node = "checkin", entry = EntryCheckin)]
pub fn checkin(args: EntryCheckin) -> Next {
    let picked = args
        .pick(&arg![CheckinFlags])
        .pick_or_route(&arg![Vec<String>], || ErrorCheckinArgument.into())
        .to_result();
    let (flags, refs) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    // The framework hands a repeated flag its own name back as one of the values; it is not a path
    // a caller wrote.
    let locals: Vec<String> = flags
        .to_local
        .into_iter()
        .filter(|path| !path.starts_with("--to-local"))
        .collect();

    if refs.is_empty() || refs.len() != locals.len() {
        return ErrorCheckinArgument.into();
    }

    StateCheckin { refs, locals }.into()
}

/// The state a checkin starts in.
#[derive(Grouped)]
pub struct StateCheckin {
    /// What to bring in, each a path in the Vault's Layout or a `Uuid`.
    refs: Vec<String>,
    /// Where each is to be named here, paired with the references by position.
    locals: Vec<String>,
}

#[chain(routeify)]
pub fn handle_checkin(
    state: StateCheckin,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    progress: &ResProgressSetting,
    offline: &ResOffline,
) -> Next {
    let StateCheckin { refs, locals } = state;

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let layouts = held.layouts();
    let name = match layouts.current() {
        Ok(Some(name)) => name,
        Ok(None) => return ErrorLayoutArgument.into(),
        Err(error) => return failed(&error),
    };
    let layout = match layouts.get(&name) {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutMissing.into(),
        Err(error) => return failed(&error),
    };

    let Some(track) = (match layouts.track(&name) {
        Ok(track) => track,
        Err(error) => return failed(&error),
    }) else {
        return ErrorCheckinNoTrack.into();
    };

    let vault_name = remote.get_ref().name_or_default(track)?;
    let target = remote.get_ref().vault_or_default(vault_name.clone())?;

    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    // The content a checkin brings is asked of whichever Vault holds it, and the one the Layout
    // tracks is asked first. What is read from that Vault is its Layout — a copy is still what the
    // references are read against — while the objects themselves may come from any Vault that has
    // them, since a key is the hash of what it names.
    let primary = fetch::primary(remote.get_ref(), Some(vault_name.as_str()));
    let sources = Sources::new(held, &account, remote.get_ref(), primary, **offline)
        .map_err(|cause| ErrorCheckinFailed { cause })?;

    // What the Vault holds now is what the references are read against, so its copy is brought up
    // to date first — the same fetch `rola layout fetch` makes. Offline, the copy that is here is
    // the one read; a run with none is told so by the check below rather than by a fetch it may
    // not make.
    if !**offline {
        action_fetch_layout(held, &account, target.to_string(), vault_name.clone())?;
    }

    let dir = readonly_layout_dir(held, &vault_name, VAULT_LAYOUT_NAME);
    if !dir.is_dir() {
        return ErrorLayoutNotCached {
            layout: VAULT_LAYOUT_NAME.to_owned(),
            vault: vault_name,
        }
        .into();
    }
    let remote_layout = match Layout::open(&dir) {
        Ok(layout) => layout,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let store = storage.get_ref().as_ref();

    let wanted = match resolve(&remote_layout, &layout, held.get_root(), &refs, &locals) {
        Ok(wanted) => wanted,
        Err(Refusal::Argument) => return ErrorCheckinArgument.into(),
        Err(Refusal::Unknown(reference)) => {
            return ErrorCheckinUnknown { reference }.into();
        }
        Err(Refusal::Taken(path)) => return ErrorCheckinTaken { path }.into(),
        Err(Refusal::Known(uuid)) => return ErrorCheckinKnown { uuid }.into(),
    };

    let runtime = match index_runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    // The index is what names the objects a checkin reads, and it is the light half: it is moved
    // whole, while the content is asked for by what `bring_in` finds it needs.
    let reporting = Reporting::start(*progress);

    if let Err(error) = pull(
        held,
        &account,
        &target.to_string(),
        &runtime,
        &reporting,
        **offline,
    ) {
        reporting.finish();

        return ErrorCheckinFailed {
            cause: error.to_string(),
        }
        .into();
    }

    reporting.finish();

    let (entries, failed) = bring_in(
        held,
        &layout,
        &remote_layout,
        index,
        store,
        &runtime,
        wanted,
        &sources,
    );

    sources.report();

    ResultCheckedIn {
        layout: name,
        vault: vault_name,
        entries,
        failed,
    }
    .into()
}

/// Moves the index to what the Vault holds, which is what names the objects a checkin reads.
///
/// Only the index: it is light, and it is what tells a version from its content. The content itself
/// is asked for one object at a time, by what [`bring_in`] finds it needs — see [`Sources`].
///
/// An offline run moves nothing: what the index already holds is what the versions are read from,
/// and one that is not here fails where it is read rather than being fetched.
fn pull(
    held: &librorolala::workspace::Workspace,
    account: &librorolala::auth::Account,
    target: &str,
    runtime: &tokio::runtime::Runtime,
    reporting: &Reporting,
    offline: bool,
) -> Result<(), ActionError> {
    if offline {
        return Ok(());
    }

    runtime.block_on(action_sync_index_all_async(
        held,
        account,
        target.to_owned(),
        String::new(),
        reporting.progress(),
    ))?;

    Ok(())
}

/// What stopped a reference from being read.
enum Refusal {
    /// A path named here does not read as one.
    Argument,
    /// The Vault's Layout names nothing by the reference.
    Unknown(String),
    /// Something is already at the path the reference was to land at.
    Taken(String),
    /// This Layout already holds the `Uuid` the reference names.
    Known(String),
}

/// One reference read: the `Uuid`, what the Vault says about it, and where it lands here.
struct Wanted {
    /// The `Uuid` it is known by.
    id: Uuid,
    /// What the Vault's Layout says about it.
    data: MutableData,
    /// The path this Layout is to name it by.
    path: LayoutPath,
}

/// Reads every reference before anything is written, so one that names nothing leaves nothing half
/// done.
fn resolve(
    remote: &Layout,
    layout: &Layout,
    root: &Path,
    refs: &[String],
    locals: &[String],
) -> Result<Vec<Wanted>, Refusal> {
    let mut wanted = Vec::new();

    for (reference, local) in refs.iter().zip(locals) {
        let Some((id, data)) = find(remote, reference) else {
            return Err(Refusal::Unknown(reference.clone()));
        };
        let Ok(path) = LayoutPath::new(local) else {
            return Err(Refusal::Argument);
        };

        // What is to come in is something this Layout does not hold, and where it lands is
        // somewhere nothing is: neither a name this Layout gives nor a file already there.
        if layout.entry(id).is_some() {
            return Err(Refusal::Known(id.to_string()));
        }
        if layout.id_of(&path).is_some() || root.join(path.to_path_buf()).exists() {
            return Err(Refusal::Taken(local.clone()));
        }

        wanted.push(Wanted { id, data, path });
    }

    Ok(wanted)
}

/// Puts each version's content at its path, and makes the Layout name it.
///
/// What could not be done is named rather than stopping the run: a checkin is not one write but
/// several, and what went in is worth reporting beside what did not.
fn bring_in(
    held: &librorolala::workspace::Workspace,
    layout: &Layout,
    remote: &Layout,
    index: &VCSIndex,
    store: Option<&RorolalaStorage>,
    runtime: &tokio::runtime::Runtime,
    wanted: Vec<Wanted>,
    sources: &Sources<'_>,
) -> (Vec<CheckinItem>, Vec<String>) {
    let mut entries = Vec::new();
    let mut failed = Vec::new();

    // Every version's content is named before anything is written, so what is missing is asked of
    // the Vaults in one go rather than one file at a time.
    let mut resolved = Vec::new();
    for wanted in wanted {
        match runtime.block_on(sync::store_of(index, &wanted.data.version())) {
            Ok(stored) => resolved.push((wanted, stored)),
            Err(cause) => failed.push(format!("{}: {cause}", wanted.id)),
        }
    }

    if let Some(store) = store {
        let keys: Vec<Key> = resolved.iter().filter_map(|(_, stored)| *stored).collect();

        sources.bring(store, &keys);
    }

    for (wanted, stored) in resolved {
        let Wanted { id, data, path } = wanted;

        let (Some(stored), Some(store)) = (stored, store) else {
            failed.push(format!("{id}: {}", t!("checkin.no_content").trim()));

            continue;
        };

        let disk = held.get_root().join(path.to_path_buf());
        if let Some(parent) = disk.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            failed.push(format!("{id}: {error}"));

            continue;
        }
        if let Err(error) = runtime.block_on(store.extract_file(&stored, &disk)) {
            failed.push(format!("{id}: {error}"));

            continue;
        }
        if let Err(cause) = remember(layout, held.get_root(), path.as_str()) {
            failed.push(format!("{id}: {cause}"));

            continue;
        }

        // What the Layout says is the version the Vault holds and the holder it names; the path is
        // this side's own, which is what the run was asked for.
        let data = MutableData::new(
            data.owner().map(str::to_owned),
            data.version(),
            data.description().to_owned(),
        );
        if let Err(error) = layout
            .create_entry(id, data)
            .and_then(|()| layout.create_path(&path, id))
        {
            failed.push(format!("{id}: {error}"));

            continue;
        }

        entries.push(CheckinItem {
            uuid: id.to_string(),
            remote_path: remote.path_of(id).map(|path| path.as_str().to_owned()),
            local_path: path.as_str().to_owned(),
        });
    }

    (entries, failed)
}

/// What a reference names in the Vault's Layout: a `Uuid` read as one, and anything else as a path.
fn find(layout: &Layout, reference: &str) -> Option<(Uuid, MutableData)> {
    if let Ok(id) = Uuid::from_str(reference) {
        return layout.entry(id).map(|data| (id, data));
    }

    let path = LayoutPath::new(reference).ok()?;
    let id = layout.id_of(&path)?;

    layout.entry(id).map(|data| (id, data))
}

/// Error: the Layout being worked in tracks no Vault.
#[derive(Grouped)]
pub struct ErrorCheckinNoTrack;

impl Failure for ErrorCheckinNoTrack {
    fn name(&self) -> &'static str {
        "error_checkin_no_track"
    }

    fn reason(&self) -> String {
        t!("checkin.err_no_track").trim().to_string()
    }
}

failure!(ErrorCheckinNoTrack);

#[renderer(buffer)]
pub fn render_error_checkin_no_track(error: ErrorCheckinNoTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_no_track_help").trim()));
    ec.exit_code = EC_ERR_CHECKIN;
}

/// Error: `rola checkin` was given arguments it cannot use.
#[derive(Grouped)]
pub struct ErrorCheckinArgument;

impl Failure for ErrorCheckinArgument {
    fn name(&self) -> &'static str {
        "error_checkin_argument"
    }

    fn reason(&self) -> String {
        t!("checkin.err_argument").trim().to_string()
    }
}

failure!(ErrorCheckinArgument);

#[renderer(buffer)]
pub fn render_error_checkin_argument(error: ErrorCheckinArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_argument_help").trim()));
    ec.exit_code = EC_ERR_CHECKIN_ARGUMENT;
}

/// Error: the Vault's Layout names nothing by the reference given.
#[derive(Grouped)]
pub struct ErrorCheckinUnknown {
    /// The reference that names nothing.
    pub reference: String,
}

impl Failure for ErrorCheckinUnknown {
    fn name(&self) -> &'static str {
        "error_checkin_unknown"
    }

    fn reason(&self) -> String {
        t!("checkin.err_unknown", reference = self.reference)
            .trim()
            .to_string()
    }
}

failure!(ErrorCheckinUnknown);

#[renderer(buffer)]
pub fn render_error_checkin_unknown(error: ErrorCheckinUnknown, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_unknown_help").trim()));
    ec.exit_code = EC_ERR_CHECKIN;
}

/// Error: something is already at the path the reference was to be named by.
#[derive(Grouped)]
pub struct ErrorCheckinTaken {
    /// The path that is already something.
    pub path: String,
}

impl Failure for ErrorCheckinTaken {
    fn name(&self) -> &'static str {
        "error_checkin_taken"
    }

    fn reason(&self) -> String {
        t!("checkin.err_taken", path = self.path).trim().to_string()
    }
}

failure!(ErrorCheckinTaken);

#[renderer(buffer)]
pub fn render_error_checkin_taken(error: ErrorCheckinTaken, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_taken_help").trim()));
    ec.exit_code = EC_ERR_CHECKIN;
}

/// Error: this Layout already holds the `Uuid` the reference names.
#[derive(Grouped)]
pub struct ErrorCheckinKnown {
    /// The `Uuid` this Layout already holds.
    pub uuid: String,
}

impl Failure for ErrorCheckinKnown {
    fn name(&self) -> &'static str {
        "error_checkin_known"
    }

    fn reason(&self) -> String {
        t!("checkin.err_known", uuid = self.uuid).trim().to_string()
    }
}

failure!(ErrorCheckinKnown);

#[renderer(buffer)]
pub fn render_error_checkin_known(error: ErrorCheckinKnown, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_known_help").trim()));
    ec.exit_code = EC_ERR_CHECKIN;
}

/// Error: the exchange could not be made.
#[derive(Grouped)]
pub struct ErrorCheckinFailed {
    /// What the exchange failed with.
    pub cause: String,
}

impl Failure for ErrorCheckinFailed {
    fn name(&self) -> &'static str {
        "error_checkin_failed"
    }

    fn reason(&self) -> String {
        t!("checkin.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorCheckinFailed);

#[renderer(buffer)]
pub fn render_error_checkin_failed(error: ErrorCheckinFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_failed_help").trim()));
    ec.exit_code = EC_ERR_CHECKIN;
}

/// One entry that came in.
#[derive(Serialize)]
pub struct CheckinItem {
    /// The `Uuid` it is known by.
    uuid: String,
    /// The path the Vault's Layout names it by, when it names one.
    remote_path: Option<String>,
    /// The path this Layout now names it by.
    local_path: String,
}

/// Result: what the Vault held was brought in.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultCheckedIn {
    /// The Layout it came into.
    layout: String,
    /// The Vault it came from.
    vault: String,
    /// What came in.
    entries: Vec<CheckinItem>,
    /// What could not be brought in, one line each.
    failed: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_checked_in(result: ResultCheckedIn, ec: &mut ResExitCode) {
    for entry in &result.entries {
        r_println!(
            "{}  {}  ->  {}",
            entry.uuid,
            entry.remote_path.as_deref().unwrap_or("-"),
            entry.local_path
        );
    }

    for cause in &result.failed {
        r_eprintln!("{}", err_line!(cause));
    }

    r_println!(
        "{}",
        t!(
            "checkin.result_summary",
            checked = result.entries.len(),
            failed = result.failed.len()
        )
        .trim()
    );

    if !result.failed.is_empty() {
        ec.exit_code = EC_ERR_CHECKIN;
    }
}
