//! The `rola checkin` command: bringing a `Uuid` the Vault holds into the Layout being worked in.
//!
//! A `Uuid` enters a Layout by being tracked and enters the Vault's Layout by going up, which
//! leaves what only the Vault holds out of reach: a sync names it rather than bringing it, since a
//! `Uuid` is not something a sync may invent a local path for. This is how one is asked for by
//! name: the operands read the way `mv`'s do — what is to come in first, where it is to land last —
//! so a reference the Vault's Layout names is put at a path here, under a directory here, or at the
//! path the Vault itself names it by when no destination was written.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates,
// so a lint allowed on the handler below is reported as defined twice. The allow lives here,
// where it covers the one signature that needs it.
//
// The handler's parameters are the resources the framework injects, so how many of them there are
// is what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::trivially_copy_pass_by_ref, clippy::too_many_arguments)]

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr as _;

use librorolala::daemon::{action_fetch_layout, action_sync_index_all_async};
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::protocol::ActionError;
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
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
use crate::complete::typing_flag;
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

#[help(buffer)]
pub fn help_checkin(_: EntryCheckin, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("checkin.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryCheckin)]
pub fn desc_checkin() -> Description {
    t!("checkin.description").to_string().into()
}

/// Completes what `rola checkin` can be given next.
///
/// Every word names where something is to land here, so the filesystem answers it. Which entry a
/// reference names is the Vault's to say, and a reference is read there rather than here, so there
/// is nothing of the Vault to list. There are no flags of this command's to offer.
#[completion(EntryCheckin)]
pub fn complete_checkin(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    Suggest::file_comp()
}

/// Brings what the Vault holds but this Layout does not into it
///
/// The operands read the way `mv`'s do: every word but the last is a reference — a path in the
/// Vault's Layout or a `Uuid` — and the last, when there is more than one word, is where those
/// references are to land here. A reference therefore always comes from the Vault's side; nothing
/// but the destination is read as a path of this run's.
///
/// With no destination each reference lands at the path the Vault's Layout names it by. A
/// destination that is an existing directory takes each reference under it by the name the Vault
/// gives it; any other destination names the whole landing path, so it takes exactly one reference.
///
/// What a reference names has to be something this Layout does not already hold, and every landing
/// has to be somewhere nothing is: a name this Layout already gives, a file already at the path, or
/// another reference of the same run landing there, is refused rather than written over. Every
/// landing is worked out before anything is written, so a run that is refused leaves nothing half
/// done.
///
/// The version the Vault holds is what comes with it — its content is taken out of the store and
/// put at the path — and what the Layout then says is that version and the holder the Vault names.
///
/// # Errors
///
/// Renders [`ErrorCheckinNoTrack`] when the Layout being worked in tracks no Vault,
/// [`ErrorCheckinArgument`] when the operands name nothing to bring in or a destination several
/// references cannot share, and [`ErrorCheckinUnknown`], [`ErrorCheckinKnown`],
/// [`ErrorCheckinUnnamed`] or [`ErrorCheckinTaken`] when one reference cannot be brought in.
#[command(node = "checkin", entry = EntryCheckin)]
pub fn checkin(args: EntryCheckin) -> Next {
    // Picking cannot fail: the positions are one list, and an absent one is the empty list.
    let words: Vec<String> = args.pick(&arg![Vec<String>]).unwrap_or_default();

    let Some((refs, to)) = split(words) else {
        return ErrorCheckinArgument.into();
    };

    StateCheckin { refs, to }.into()
}

/// The state a checkin starts in.
#[derive(Grouped)]
pub struct StateCheckin {
    /// What to bring in, each a path in the Vault's Layout or a `Uuid`.
    refs: Vec<String>,
    /// Where they are to land here, or nothing to land each at the path the Vault names it by.
    to: Option<String>,
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
    let StateCheckin { refs, to } = state;

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

    let wanted = match resolve(
        &remote_layout,
        &layout,
        held.get_root(),
        &refs,
        to.as_deref(),
    ) {
        Ok(wanted) => wanted,
        Err(refusal) => return refused(refusal),
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
    /// The operands name nothing to bring in, or a destination several references cannot share.
    Argument,
    /// The Vault's Layout names nothing by the reference.
    Unknown(String),
    /// The Vault's Layout gives the `Uuid` no path, and a name is needed where it is to land.
    Unnamed(Uuid),
    /// Something is already at the path the reference was to land at.
    Taken(String),
    /// This Layout already holds the `Uuid` the reference names.
    Known(String),
}

/// The failure a refused reference is answered with.
fn refused(refusal: Refusal) -> Next {
    match refusal {
        Refusal::Argument => ErrorCheckinArgument.into(),
        Refusal::Unknown(reference) => ErrorCheckinUnknown { reference }.into(),
        Refusal::Unnamed(uuid) => ErrorCheckinUnnamed {
            uuid: uuid.to_string(),
        }
        .into(),
        Refusal::Taken(path) => ErrorCheckinTaken { path }.into(),
        Refusal::Known(uuid) => ErrorCheckinKnown { uuid }.into(),
    }
}

/// Where the references are to land here, read from the operands the way `mv` reads its target.
enum Destination {
    /// No destination was written: each reference lands at the path the Vault names it by.
    Named,
    /// One reference lands at exactly this path.
    Exact(LayoutPath),
    /// Every reference lands under this directory, joined with the name the Vault gives it; `""`
    /// names the Workspace root, the one directory with no name of its own.
    Under(String),
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
    to: Option<&str>,
) -> Result<Vec<Wanted>, Refusal> {
    let destination = destination(root, to)?;

    // A destination that is not a directory names one landing, so it can take only one reference.
    if matches!(destination, Destination::Exact(_)) && refs.len() > 1 {
        return Err(Refusal::Argument);
    }

    let mut wanted = Vec::new();
    let mut landings = BTreeSet::new();

    for reference in refs {
        let Some((id, data)) = find(remote, reference) else {
            return Err(Refusal::Unknown(reference.clone()));
        };

        // What is to come in is something this Layout does not hold.
        if layout.entry(id).is_some() {
            return Err(Refusal::Known(id.to_string()));
        }

        let Some(path) = landing(&destination, remote.path_of(id).as_ref()) else {
            return Err(Refusal::Unnamed(id));
        };

        // Where it lands is somewhere nothing is: not a name this Layout gives, not a file already
        // there, and not where an earlier reference of this run is to land.
        if !landings.insert(path.as_str().to_owned())
            || layout.id_of(&path).is_some()
            || root.join(path.to_path_buf()).exists()
        {
            return Err(Refusal::Taken(path.as_str().to_owned()));
        }

        wanted.push(Wanted { id, data, path });
    }

    Ok(wanted)
}

/// Splits the operands the way `mv` reads its own: everything but the last word is what is to come
/// in, and the last — when there is more than one word — is where it is to go.
///
/// `None` is a run that named nothing at all.
fn split(mut words: Vec<String>) -> Option<(Vec<String>, Option<String>)> {
    if words.is_empty() {
        return None;
    }

    if words.len() == 1 {
        return Some((words, None));
    }

    let to = words.pop();

    Some((words, to))
}

/// Where the operands say the references are to land.
///
/// A destination is read where the run was made, the way `rola track` and `rola align` read a name:
/// `.` is the directory the run is in and `..` climbs from there. It has to be somewhere the
/// Workspace holds, since a Layout names nothing outside its own root.
fn destination(root: &Path, to: Option<&str>) -> Result<Destination, Refusal> {
    let Some(to) = to else {
        return Ok(Destination::Named);
    };

    let Ok(cwd) = std::env::current_dir() else {
        return Err(Refusal::Argument);
    };

    let absolute = normalize(&resolve_against(&cwd, to));

    let Ok(relative) = absolute.strip_prefix(root) else {
        return Err(Refusal::Argument);
    };

    if absolute.is_dir() {
        return Ok(Destination::Under(prefix(relative)));
    }

    LayoutPath::from_relative(relative)
        .map(Destination::Exact)
        .map_err(|_| Refusal::Argument)
}

/// The path one reference lands at: the name the Vault gives it, joined into the destination when
/// that is a directory, or the destination itself when that is the whole name.
///
/// `None` is a reference the Vault gives no path while a name is needed, which the destination
/// being a directory — or absent — is.
fn landing(destination: &Destination, named: Option<&LayoutPath>) -> Option<LayoutPath> {
    match destination {
        Destination::Named => named.cloned(),
        Destination::Exact(path) => Some(path.clone()),
        Destination::Under(directory) => {
            let name = named?;

            // A directory takes something by its leaf name, the way `mv` names what it puts in one:
            // the directories above it are the Vault's own arrangement, kept there rather than
            // remade here.
            let leaf = name.as_str().rsplit('/').next()?;
            let joined = if directory.is_empty() {
                leaf.to_owned()
            } else {
                format!("{directory}/{leaf}")
            };

            // Both halves are names a `LayoutPath` already accepted, so joining them cannot name
            // something one would not.
            LayoutPath::new(&joined).ok()
        }
    }
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve_against(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// `path` with its `.` dropped and its `..` climbed, worked out lexically rather than on disk.
///
/// Working it out here, before the disk is asked anything, is what lets `.` name the Workspace root
/// rather than a path with no components in it.
fn normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if matches!(components.last(), Some(Component::Normal(_))) => {
                components.pop();
            }
            other => components.push(other),
        }
    }

    components.into_iter().collect()
}

/// The prefix the paths under `relative` share, or `""` for the Workspace root itself.
fn prefix(relative: &Path) -> String {
    LayoutPath::from_relative(relative)
        .map_or_else(|_| String::new(), |path| path.as_str().to_owned())
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

/// Error: the Vault's Layout gives the `Uuid` no path, and nothing here names where it is to land.
#[derive(Grouped)]
pub struct ErrorCheckinUnnamed {
    /// The `Uuid` the Vault's Layout gives no path.
    pub uuid: String,
}

impl Failure for ErrorCheckinUnnamed {
    fn name(&self) -> &'static str {
        "error_checkin_unnamed"
    }

    fn reason(&self) -> String {
        t!("checkin.err_unnamed", uuid = self.uuid)
            .trim()
            .to_string()
    }
}

failure!(ErrorCheckinUnnamed);

#[renderer(buffer)]
pub fn render_error_checkin_unnamed(error: ErrorCheckinUnnamed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_unnamed_help").trim()));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The words of a command line, as the picker hands them over.
    fn words(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| (*text).to_owned()).collect()
    }

    /// A path a Layout would name.
    fn path(text: &str) -> LayoutPath {
        LayoutPath::new(text).expect("a path a Layout could name")
    }

    #[test]
    fn the_last_word_is_the_destination_unless_it_is_the_only_one() {
        assert_eq!(split(words(&["a"])), Some((words(&["a"]), None)));
        assert_eq!(
            split(words(&["a", "b"])),
            Some((words(&["a"]), Some("b".to_owned())))
        );
        assert_eq!(
            split(words(&["a", "b", "c"])),
            Some((words(&["a", "b"]), Some("c".to_owned())))
        );
        assert_eq!(split(words(&[])), None);
    }

    #[test]
    fn a_missing_destination_lands_at_the_name_the_vault_gives() {
        assert_eq!(
            landing(&Destination::Named, Some(&path("a/b.psd"))),
            Some(path("a/b.psd"))
        );
        assert_eq!(landing(&Destination::Named, None), None);
    }

    #[test]
    fn a_whole_name_destination_is_the_path_itself() {
        assert_eq!(
            landing(
                &Destination::Exact(path("here.psd")),
                Some(&path("a/b.psd"))
            ),
            Some(path("here.psd"))
        );
        assert_eq!(
            landing(&Destination::Exact(path("here.psd")), None),
            Some(path("here.psd"))
        );
    }

    #[test]
    fn a_directory_destination_is_joined_with_the_name_the_vault_gives() {
        assert_eq!(
            landing(
                &Destination::Under("dir".to_owned()),
                Some(&path("a/b.psd"))
            ),
            Some(path("dir/b.psd"))
        );
        assert_eq!(
            landing(&Destination::Under(String::new()), Some(&path("a/b.psd"))),
            Some(path("b.psd"))
        );
        assert_eq!(landing(&Destination::Under("dir".to_owned()), None), None);
    }
}
