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
// is what the command needs of the run rather than a signature this program shaped. It is long
// because it reads every reference, the Variant mode included, before anything is written.
#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use globset::GlobBuilder;
use librorolala::daemon::{action_fetch_layout, action_sync_index_all_async};
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::protocol::ActionError;
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::{VCSIndex, VCSIndexObject, Variant};
use librorolala::workspace::{Merging, Pending};
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResCurrentRemoteVault, ResForce, ResOffline, ResProgressSetting, ResRorolalaStorage,
    ResVCSIndex, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rorolala_utils_location::normalize;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::checkout::remember;
use crate::complete::{
    IndexObject, filling_flag, flag_value, index_hashes, index_keys, offer, positional,
    strip_written, typing_flag,
};
use crate::exit_codes::{EC_ERR_CHECKIN, EC_ERR_CHECKIN_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::fetch::{self, Sources};
use crate::hash::resolve as resolve_hash;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutNoCurrent, ErrorLayoutNotCached, failed,
    readonly_layout_dir,
};
use crate::progress::Reporting;
use crate::sync;
use crate::vcs_index::{ErrorVcsIndexNoIndex, runtime as index_runtime};

/// The file a variant is to be joined into, for the mode that checks a variant in rather than
/// bringing a `Uuid` the Vault holds into this Layout.
const ARG_JOIN: PickerArg<'static, Option<String>> = arg![join: Option<String>];

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
/// is nothing of the Vault to list. The variant mode is the one thing only this run knows: its
/// first word is a variant the index holds, and the rest is a place like any other.
#[completion(EntryCheckin)]
pub fn complete_checkin(ctx: ShellContext, index: &mut LazyRes<ResVCSIndex>) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_JOIN: t!("checkin.complete.join"),
            },
        );
    }

    if filling_flag(&ctx, &ARG_JOIN) {
        return Suggest::file_comp();
    }

    if flag_value(&ctx, &ARG_JOIN).is_some() && positional(&ctx, "checkin") == 0 {
        return offer(
            &ctx,
            index_hashes(index.get_ref().as_ref(), IndexObject::Variant),
        );
    }

    Suggest::file_comp()
}

/// Brings what the Vault holds but this Layout does not into it
///
/// The operands read the way `mv`'s do: every word but the last is a reference — a path in the
/// Vault's Layout, a glob over those paths, or a `Uuid` — and the last, when there is more than one
/// word, is where those references are to land here. A reference therefore always comes from the
/// Vault's side; nothing but the destination is read as a path of this run's.
///
/// A glob is matched against the paths the Vault's Layout names, so `Installers/*` names everything
/// directly under `Installers` there and `Installers/**` everything below it. An entry several
/// references name is brought in once.
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
    let picked = args.pick(&arg![Vec<String>]).pick(&ARG_JOIN).to_result();
    let (words, join): (Vec<String>, Option<String>) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    let Some((refs, to)) = split(join.as_deref(), words) else {
        return ErrorCheckinArgument.into();
    };

    StateCheckin { refs, to, join }.into()
}

/// The state a checkin starts in.
#[derive(Grouped)]
pub struct StateCheckin {
    /// What to bring in: paths in the Vault's Layout and `Uuid`s, or the one variant a merge checks
    /// in.
    refs: Vec<String>,
    /// Where they are to land here, or nothing to land each at the path the Vault names it by — and
    /// for the variant mode, where its file is to lie.
    to: Option<String>,
    /// The file a variant is to be joined into, when this run checks one in.
    join: Option<String>,
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
    force: &ResForce,
) -> Next {
    let StateCheckin { refs, to, join } = state;

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let layouts = held.layouts();
    let name = match layouts.current() {
        Ok(Some(name)) => name,
        Ok(None) => return ErrorLayoutNoCurrent.into(),
        Err(error) => return failed(&error),
    };
    let layout = match layouts.get(&name) {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutMissing.into(),
        Err(error) => return failed(&error),
    };

    // A variant is not a `Uuid` the Vault's Layout names, so the mode that checks one in reads its
    // own operands and asks for the Vault only when what it wants is not here: a merge of something
    // already synced is local work, and a Workspace that tracks no Vault can still do it.
    if let Some(target) = join {
        // UNWRAP: `split` keeps one reference for this mode, and refuses when there is none.
        let variant = refs.first().map(String::as_str).unwrap_or_default();

        return checkin_variant(
            &target,
            variant,
            to.as_deref(),
            held,
            &layout,
            index,
            storage,
            remote,
            current,
            progress,
            offline,
            **force,
        );
    }

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

/// Checks a variant in to be joined into a file this Layout names.
///
/// What comes in is not a path of the Vault's Layout but a variant the index holds — content that
/// already happened elsewhere and is to be joined into a file here without either side giving up.
/// The content is written to a **variant file** beside its target, under a name of the convention or
/// of the run's own, and what is waiting for what is written into [`Merging`] rather than into the
/// Layout: a merge is work in this tree alone, and the Layout is what the work is.
///
/// The variant and its content are asked of the Vault the Layout tracks only when they are not here;
/// a merge of something already synced is local work, so a run that has both does not need a Vault,
/// an account, or a reachable one.
fn checkin_variant(
    target: &str,
    variant: &str,
    at: Option<&str>,
    held: &librorolala::workspace::Workspace,
    layout: &Layout,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    progress: &ResProgressSetting,
    offline: &ResOffline,
    force: bool,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let variant_key = match resolve_hash(variant, || index_keys(Some(index), IndexObject::Variant))
    {
        Ok(key) => key,
        Err(miss) => {
            let said = miss.reason(variant, || t!("checkin.err_join_hash").trim().to_string());

            return join_failed(target, said);
        }
    };

    let root = held.get_root();
    let cwd = std::env::current_dir().ok();
    let Some(id) = target_id(layout, root, cwd.as_deref(), target) else {
        return join_failed(target, t!("checkin.err_join_target").trim());
    };
    let Some(data) = layout.entry(id) else {
        return join_failed(target, t!("checkin.err_join_target").trim());
    };

    let mut merging = Merging::read(root);

    // One variant joins one file once: a second waiting on the same file would leave the first with
    // nowhere to go, and the same variant checked in twice would be two names for one thing.
    if merging.at(id).is_some() {
        return join_failed(target, t!("checkin.err_join_merging").trim());
    }
    if merging.holds_variant(variant_key.digest()) {
        return join_failed(target, t!("checkin.err_join_checked_in").trim());
    }

    let runtime = match index_runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let mut found = read_variant(index, &runtime, &variant_key);

    // What is not here is asked of the Vault, unless the run was made offline: an offline run reads
    // what is here and says so where the read wants what is not.
    if found.is_none() && !**offline {
        match index_from_vault(held, layout, remote, current, progress, &runtime, **offline) {
            Ok(()) => found = read_variant(index, &runtime, &variant_key),
            Err(cause) => return join_failed(target, cause),
        }
    }

    let Some(variant_object) = found else {
        return join_failed(target, t!("checkin.err_join_unknown").trim());
    };

    // A variant is joined into a file whose history it shares: its base has to be a version the
    // file's own chain holds, or the two are histories that never met and the drawing has no place
    // to put it. `--force` is how a run says it knows that and means to join it anyway.
    if !force {
        match on_chain(
            index,
            &runtime,
            data.version(),
            variant_object.base_version(),
        ) {
            Ok(true) => {}
            Ok(false) => return join_failed(target, t!("checkin.err_join_off_chain").trim()),
            Err(cause) => return join_failed(target, cause),
        }
    }

    let Some(store) = storage.get_ref().as_ref() else {
        return join_failed(target, t!("checkin.err_join_no_store").trim());
    };
    let content = Key::new(*variant_object.storage_hash());

    // The content itself is brought the way any other checkin brings it: asked of whichever Vault
    // holds it, when there is an account to ask as and a run that may reach one at all.
    let account = current
        .get_ref()
        .must_bind()
        .ok()
        .and_then(|name| account_named(&name, Some(held), None).ok());
    if let Some(account) = &account {
        let tracked = crate::ownership::tracked_vault(held, layout);
        let primary = fetch::primary(remote.get_ref(), tracked.as_deref());

        if let Ok(sources) = Sources::new(held, account, remote.get_ref(), primary, **offline) {
            sources.bring(store, &[content]);
            sources.report();
        }
    }

    // A place the run names is checked as it is given: one that climbs out of the Workspace is
    // refused rather than held at the root, since there is no target that has moved yet to excuse
    // it. A place that comes to climb out later, by the target moving shallower, is held instead.
    if let Some(at) = at
        && let Some(target_path) = layout.path_of(id)
        && !Merging::place_holds(&target_path, Path::new(at))
    {
        return join_failed(target, t!("checkin.err_join_place").trim());
    }

    let pending = Pending::new(id, *variant_key.digest(), at.map(PathBuf::from));
    let Some(path) = merging.variant_path(layout, &pending) else {
        return join_failed(target, t!("checkin.err_join_place").trim());
    };
    let disk = root.join(path.to_path_buf());

    // Nothing is written over: a file already at the place is something to be told about rather than
    // replaced, which is how `checkin` reads a landing everywhere else.
    if disk.exists() {
        return join_failed(
            target,
            t!("checkin.err_join_taken", path = path.as_str()).trim(),
        );
    }

    if let Some(parent) = disk.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return join_failed(target, error.to_string());
    }
    if let Err(error) = runtime.block_on(store.extract_file(&content, &disk)) {
        return join_failed(target, error.to_string());
    }

    merging.push(pending);
    if let Err(error) = merging.write(root) {
        return join_failed(target, error.to_string());
    }

    ResultCheckinJoin {
        target: target.to_owned(),
        variant: variant.to_owned(),
        path: path.as_str().to_owned(),
    }
    .into()
}

/// Brings the index over from the Vault the Layout tracks, so what the run names can be read here.
///
/// What cannot be done is a cause rather than a failure of its own: the run that wanted the variant
/// is the one that reports it, and it is that run's name the reader needs beside it.
fn index_from_vault(
    held: &librorolala::workspace::Workspace,
    layout: &Layout,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    progress: &ResProgressSetting,
    runtime: &tokio::runtime::Runtime,
    offline: bool,
) -> Result<(), String> {
    let Some(vault_name) = crate::ownership::tracked_vault(held, layout) else {
        return Err(t!("checkin.err_join_no_track").trim().to_owned());
    };
    let Some(account_name) = current.get_ref().must_bind().ok() else {
        return Err(t!("checkin.err_join_no_account").trim().to_owned());
    };
    let Some(account) = account_named(&account_name, Some(held), None).ok() else {
        return Err(t!("checkin.err_join_no_account").trim().to_owned());
    };
    let Ok(target) = remote.get_ref().vault_or_default(vault_name) else {
        return Err(t!("checkin.err_join_no_vault").trim().to_owned());
    };

    let reporting = Reporting::start(*progress);
    let pulled = pull(
        held,
        &account,
        &target.to_string(),
        runtime,
        &reporting,
        offline,
    );
    reporting.finish();

    pulled.map_err(|error| error.to_string())
}

/// The `Uuid` of the file `target` names, read from where the run was made and then as it stands.
///
/// It is the same reading `rola track` and `rola align` give a name, since a person writes one the
/// same way wherever it is written.
fn target_id(layout: &Layout, root: &Path, cwd: Option<&Path>, target: &str) -> Option<Uuid> {
    if let Some(cwd) = cwd {
        let rooted = normalize(root);
        let absolute = normalize(&resolve_against(cwd, target));

        if let Ok(relative) = absolute.strip_prefix(&rooted)
            && let Ok(path) = LayoutPath::from_relative(relative)
            && let Some(id) = layout.id_of(&path)
        {
            return Some(id);
        }
    }

    LayoutPath::new(target)
        .ok()
        .and_then(|path| layout.id_of(&path))
}

/// The variant the index holds under `key`, when it holds one.
fn read_variant(index: &VCSIndex, runtime: &tokio::runtime::Runtime, key: &Key) -> Option<Variant> {
    match runtime.block_on(index.read(*key)) {
        Ok(VCSIndexObject::Variant(variant)) => Some(variant),
        _ => None,
    }
}

/// Whether `base` is a version on the chain the version `head` heads.
///
/// The chain is read by following what each version points at and what that variant was based on,
/// back to the root. A version met twice is the end of the reading rather than a loop to follow,
/// since only a corrupted index has one.
fn on_chain(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    head: [u8; 32],
    base: &[u8; 32],
) -> Result<bool, String> {
    let mut seen = BTreeSet::new();
    let mut current = head;

    loop {
        if current == *base {
            return Ok(true);
        }
        if !seen.insert(current) {
            return Ok(false);
        }

        let version = match runtime.block_on(index.read(Key::new(current))) {
            Ok(VCSIndexObject::Version(version)) => version,
            Ok(_) => return Err(t!("checkin.err_join_chain").trim().to_owned()),
            Err(error) => return Err(error.reason()),
        };

        if version.is_root() {
            return Ok(false);
        }

        let variant = match runtime.block_on(index.read(Key::new(*version.variant()))) {
            Ok(VCSIndexObject::Variant(variant)) => variant,
            Ok(_) => return Err(t!("checkin.err_join_chain").trim().to_owned()),
            Err(error) => return Err(error.reason()),
        };

        current = *variant.base_version();
    }
}

/// The failure a variant that could not be checked in is answered with.
fn join_failed(target: &str, cause: impl Into<String>) -> Next {
    ErrorCheckinJoin {
        target: target.to_owned(),
        cause: cause.into(),
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
#[derive(Debug)]
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

    // A destination that is not a directory names one landing. More than one operand is already
    // more than it can take, the way `mv` reads them; one operand naming several — a glob — is
    // found out once the references are read below.
    if matches!(destination, Destination::Exact(_)) && refs.len() > 1 {
        return Err(Refusal::Argument);
    }

    let matched = expand(remote, refs)?;

    // A destination that is not a directory names one landing, so it can take only one entry.
    if matches!(destination, Destination::Exact(_)) && matched.len() > 1 {
        return Err(Refusal::Argument);
    }

    let mut wanted = Vec::new();
    let mut landings = BTreeSet::new();

    for (id, data) in matched {
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

/// Everything the references name, each entry once and in the order the references name them.
///
/// An entry several references name — a glob and a path that reaches into what it matched — is one
/// request rather than a clash, so it is brought in once.
fn expand(layout: &Layout, refs: &[String]) -> Result<Vec<(Uuid, MutableData)>, Refusal> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();

    for reference in refs {
        for (id, data) in named(layout, reference)? {
            if seen.insert(id) {
                entries.push((id, data));
            }
        }
    }

    Ok(entries)
}

/// What one reference names in the Vault's Layout: a `Uuid` read as one, a glob over the paths
/// there, and anything else one path there.
///
/// # Errors
///
/// Returns [`Refusal::Argument`] when a reference reads as a pattern but not as a usable one, and
/// [`Refusal::Unknown`] when it names nothing — a glob that matches nothing included, since a run
/// that quietly did part of what was asked is worse than one that stops.
fn named(layout: &Layout, reference: &str) -> Result<Vec<(Uuid, MutableData)>, Refusal> {
    if let Ok(id) = Uuid::from_str(reference) {
        return layout
            .entry(id)
            .map(|data| vec![(id, data)])
            .ok_or_else(|| Refusal::Unknown(reference.to_owned()));
    }

    if is_pattern(reference) {
        // The Layout lists its paths in no particular order, so they are ordered here: what a glob
        // matched is reported the same way however the Layout happens to hold it.
        let mut paths: Vec<LayoutPath> = layout.paths().into_iter().map(|(path, _)| path).collect();
        paths.sort_unstable();

        let found: Vec<(Uuid, MutableData)> = matching(reference, &paths)?
            .into_iter()
            .filter_map(|path| {
                let id = layout.id_of(&path)?;

                layout.entry(id).map(|data| (id, data))
            })
            .collect();

        if found.is_empty() {
            return Err(Refusal::Unknown(reference.to_owned()));
        }

        return Ok(found);
    }

    let Ok(path) = LayoutPath::new(reference) else {
        return Err(Refusal::Unknown(reference.to_owned()));
    };
    let Some(id) = layout.id_of(&path) else {
        return Err(Refusal::Unknown(reference.to_owned()));
    };

    layout
        .entry(id)
        .map(|data| vec![(id, data)])
        .ok_or_else(|| Refusal::Unknown(reference.to_owned()))
}

/// Whether a reference is to be read as a glob over the Vault's Layout paths.
///
/// A reference with none of the pattern characters names one path exactly, so a path holding a `[`
/// or `{` is still nameable; one with them is a pattern, and a literal occurrence has to be written
/// as a class the way [`globset`] spells one.
fn is_pattern(reference: &str) -> bool {
    reference
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | '{'))
}

/// The paths among `paths` that the glob `reference` names, keeping the order they came in.
///
/// Separators are literal, the way a shell reads a pattern: `*` names what is directly under a
/// directory and `**` what is anywhere below it, rather than `*` swallowing a whole tree.
///
/// # Errors
///
/// Returns [`Refusal::Argument`] when the reference does not read as a pattern.
fn matching(reference: &str, paths: &[LayoutPath]) -> Result<Vec<LayoutPath>, Refusal> {
    let glob = GlobBuilder::new(reference)
        .literal_separator(true)
        .build()
        .map_err(|_| Refusal::Argument)?;
    let matcher = glob.compile_matcher();

    Ok(paths
        .iter()
        .filter(|path| matcher.is_match(path.as_str()))
        .cloned()
        .collect())
}

/// Splits the operands the way `mv` reads its own: everything but the last word is what is to come
/// in, and the last — when there is more than one word — is where it is to go.
///
/// The variant mode reads the same words differently: the first is the variant to check in and, when
/// a second is written, it is where that variant's file is to lie rather than another reference.
///
/// `None` is a run that named nothing at all, or one that named more than a mode can hold.
fn split(join: Option<&str>, mut words: Vec<String>) -> Option<(Vec<String>, Option<String>)> {
    if words.is_empty() {
        return None;
    }

    if join.is_some() {
        if words.len() > 2 {
            return None;
        }

        let at = (words.len() == 2).then(|| words.pop()).flatten();

        return Some((words, at));
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
/// The path `given` names, made absolute against the directory the run was made in.
fn resolve_against(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
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

/// Error: a variant could not be checked in to be joined into a file.
#[derive(Grouped)]
pub struct ErrorCheckinJoin {
    /// The file it was to be joined into, as the run named it.
    pub target: String,
    /// Why it could not be checked in.
    pub cause: String,
}

impl Failure for ErrorCheckinJoin {
    fn name(&self) -> &'static str {
        "error_checkin_join"
    }

    fn reason(&self) -> String {
        t!("checkin.err_join", target = self.target, cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorCheckinJoin);

#[renderer(buffer)]
pub fn render_error_checkin_join(error: ErrorCheckinJoin, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("checkin.err_join_help").trim()));
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

/// Result: a variant was checked in to be joined into a file.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultCheckinJoin {
    /// The file it is to be joined into, as the run named it.
    target: String,
    /// The variant that was checked in.
    variant: String,
    /// The path its file was written to.
    path: String,
}

#[renderer(buffer)]
pub fn render_result_checkin_join(result: ResultCheckinJoin) {
    r_println!(
        "{}",
        t!(
            "checkin.joined",
            target = result.target,
            variant = result.variant,
            path = result.path
        )
        .trim()
    );
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
        assert_eq!(split(None, words(&["a"])), Some((words(&["a"]), None)));
        assert_eq!(
            split(None, words(&["a", "b"])),
            Some((words(&["a"]), Some("b".to_owned())))
        );
        assert_eq!(
            split(None, words(&["a", "b", "c"])),
            Some((words(&["a", "b"]), Some("c".to_owned())))
        );
        assert_eq!(split(None, words(&[])), None);
    }

    #[test]
    fn the_variant_mode_reads_the_second_word_as_where_its_file_lies() {
        assert_eq!(
            split(Some("target"), words(&["variant"])),
            Some((words(&["variant"]), None))
        );
        assert_eq!(
            split(Some("target"), words(&["variant", "here.psd"])),
            Some((words(&["variant"]), Some("here.psd".to_owned())))
        );
        assert_eq!(split(Some("target"), words(&["a", "b", "c"])), None);
        assert_eq!(split(Some("target"), words(&[])), None);
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

    #[test]
    fn a_path_without_a_pattern_character_is_named_exactly() {
        assert!(!is_pattern("Installers/0dCloud.pkg.tar.zst"));
        assert!(is_pattern("Installers/*"));
        assert!(is_pattern("a?b"));
        assert!(is_pattern("a[0-9]"));
        assert!(is_pattern("a{b,c}"));
    }

    #[test]
    fn a_glob_names_the_paths_it_matches_and_nothing_else() {
        let paths = [
            path("Installers/0dCloud.pkg.tar.zst"),
            path("Installers/sub/extra.pkg"),
            path("README.md"),
        ];

        assert_eq!(
            matching("Installers/*", &paths).unwrap(),
            vec![path("Installers/0dCloud.pkg.tar.zst")]
        );
        assert_eq!(
            matching("Installers/**", &paths).unwrap(),
            vec![
                path("Installers/0dCloud.pkg.tar.zst"),
                path("Installers/sub/extra.pkg")
            ]
        );
        assert!(matching("Missing/*", &paths).unwrap().is_empty());
        assert!(matches!(
            matching("Installers/[", &paths),
            Err(Refusal::Argument)
        ));
    }
}
