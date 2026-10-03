//! The `rola layout` namespace: the Layouts a Workspace works in and a Vault keeps.
//!
//! A Workspace holds many Layouts and works in one at a time; a Vault holds one. What is here is
//! what the two have in common — where a run's Layouts are, and how a failure to work on them is
//! said — beside the subcommands, each in a file of its own: listing them, and, under those, making,
//! copying, tracking, exporting, importing and removing them.

pub mod cmd_layout;
pub mod cmd_layout_cp;
pub mod cmd_layout_entries;
pub mod cmd_layout_entry;
pub mod cmd_layout_fetch;
pub mod cmd_layout_force_switch;
pub mod cmd_layout_giveup_ownership;
pub mod cmd_layout_import;
pub mod cmd_layout_ls;
pub mod cmd_layout_ls_ownership;
pub mod cmd_layout_new;
pub mod cmd_layout_path;
pub mod cmd_layout_read_ownership;
pub mod cmd_layout_req_ownership;
pub mod cmd_layout_rm;
pub mod cmd_layout_set_track;
pub mod cmd_layout_shot;
pub mod cmd_layout_tree_diff;
pub mod cmd_layout_un_track;

use mingling::{
    Grouped,
    macros::{arg, buffer, r_eprintln, r_println, renderer},
    picker::PickerArg,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rorolala_utils_constants::{VAULT_LAYOUT_NAME, WORKSPACE_READONLY_LAYOUTS_DIR};
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;
use rust_i18n::t;
use uuid::Uuid;

use librorolala::layout::{Layout, LayoutError, LayoutPath, Layouts, MutableData};
use librorolala::storage::Key;

use crate::Next;
use crate::exit_codes::{
    EC_ALREADY_EXIST, EC_ERR_LAYOUT, EC_ERR_LAYOUT_ARGUMENT, EC_ERR_LAYOUT_NO_CURRENT,
    EC_ERR_LAYOUT_NOT_CACHED, EC_ERR_LAYOUT_OWNERSHIP, EC_ERR_SHOULD_IN_WORKSPACE, EC_NOT_EXIST,
};
use crate::failure::failure;
use crate::hash::{HashMiss, resolve};

/// What a run works its Layouts through.
pub enum Place {
    /// A Workspace, with its set of named Layouts.
    Workspace(Layouts),

    /// A Vault, with its one Layout.
    Vault(Layout),
}

/// The place a run works its Layouts in, from what the run found.
///
/// A Workspace comes before a Vault: a run inside one works in its Layouts, whatever else the
/// directories above it hold. A Vault keeps its one Layout directly, so it is opened here.
///
/// # Errors
///
/// Answers with [`ErrorNoLayouts`] when the run is in neither, and [`ErrorLayoutFailed`] when a
/// Vault's Layout could not be opened.
pub fn place(workspace: &ResWorkspace, vault: &ResVault) -> Result<Place, Next> {
    if let Some(workspace) = workspace.as_ref() {
        return Ok(Place::Workspace(workspace.layouts()));
    }

    let Some(vault) = vault.as_ref() else {
        return Err(ErrorNoLayouts.into());
    };

    vault.layout().map(Place::Vault).map_err(|error| {
        ErrorLayoutFailed {
            cause: error.to_string(),
        }
        .into()
    })
}

/// Error: this run is in neither a Workspace nor a Vault, so it has no Layouts to work on.
#[derive(Grouped)]
pub struct ErrorNoLayouts;

impl Failure for ErrorNoLayouts {
    fn name(&self) -> &'static str {
        "error_no_layouts"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_no_layouts").trim().to_string()
    }
}

failure!(ErrorNoLayouts);

#[renderer(buffer)]
pub fn render_error_no_layouts(_: ErrorNoLayouts, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_no_layouts").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_no_layouts_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT;
}

/// Error: a Layout could not be worked on.
///
/// The reason is the library's own words, since the Layout says what went wrong about it — a path
/// already taken, a name that is not one, or bytes that would not be read.
#[derive(Grouped)]
pub struct ErrorLayoutFailed {
    /// Why the Layout would not be worked on.
    cause: String,
}

impl ErrorLayoutFailed {
    /// The failure of a Layout that could not be worked on, saying why in `cause`.
    #[must_use]
    pub fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorLayoutFailed {
    fn name(&self) -> &'static str {
        "error_layout_failed"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutFailed);

#[renderer(buffer)]
pub fn render_error_layout_failed(error: ErrorLayoutFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_failed_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT;
}

/// The Layout a run works on: the one named, or the one being worked in.
///
/// What is named is a Workspace's to choose; a Vault keeps one Layout and has nothing to name. A
/// name written `NAME@VAULT` names a Vault's Layout rather than one of the Workspace's own: the
/// copy a [`fetch`](crate::layout::cmd_layout_fetch) brought here is read, and a run that names one
/// outside a Workspace — or one that was never fetched — is refused rather than answered from
/// somewhere else.
///
/// # Errors
///
/// Answers with the rendered failures of [`place`], with [`ErrorLayoutNoCurrent`] when nothing is
/// named and the Workspace has no Layout being worked in, with [`ErrorLayoutArgument`] when a Vault
/// run named one, with [`ErrorLayoutShouldInWorkspace`] when a Vault's Layout is named outside a
/// Workspace, with [`ErrorLayoutNotCached`] when that Layout was never fetched, and with
/// [`ErrorLayoutMissing`] when the one named is not there.
pub fn chosen(
    workspace: &ResWorkspace,
    vault: &ResVault,
    named: Option<&str>,
) -> Result<Layout, Next> {
    if let Some((layout, vault_name)) = named.and_then(remote_spec) {
        return cached_layout(workspace, layout, vault_name);
    }

    match place(workspace, vault)? {
        Place::Workspace(layouts) => {
            let name = match named {
                Some(name) => name.to_owned(),
                None => match layouts.current() {
                    Ok(Some(name)) => name,
                    // A Workspace with no Layout being worked in has nowhere to work, which is a
                    // different thing from a name that does not resolve: what is missing is the
                    // Layout itself, and adding one is what makes the Workspace usable.
                    Ok(None) => return Err(ErrorLayoutNoCurrent.into()),
                    Err(error) => return Err(failed(&error)),
                },
            };

            match layouts.get(&name) {
                Ok(Some(layout)) => Ok(layout),
                Ok(None) => Err(ErrorLayoutMissing.into()),
                Err(error) => Err(failed(&error)),
            }
        }
        Place::Vault(layout) => {
            if named.is_some() {
                return Err(ErrorLayoutArgument.into());
            }

            Ok(layout)
        }
    }
}

/// The Layout a run works on, when the run is to change it.
///
/// It is [`chosen`] for a command that writes: a fetched copy of a Vault's Layout is read, never
/// worked in, so naming one is refused before anything is opened — see [`ErrorLayoutReadOnly`].
///
/// # Errors
///
/// Returns what [`chosen`] does, and [`ErrorLayoutReadOnly`] when the name is a Vault's Layout.
pub fn chosen_writable(
    workspace: &ResWorkspace,
    vault: &ResVault,
    named: Option<&str>,
) -> Result<Layout, Next> {
    if let Some((layout, _)) = named.and_then(remote_spec) {
        return Err(ErrorLayoutReadOnly {
            layout: layout.to_owned(),
        }
        .into());
    }

    chosen(workspace, vault, named)
}

/// The Vault and the Layout a name written `NAME@VAULT` names, when it is written that way.
///
/// `NAME` is the Layout as the Vault knows it — `truth`, for the one a Vault keeps — and `VAULT` is
/// the name the Workspace fetched it under, which is what the copy on disk is keyed by. A name
/// that is not written this way, or one that names nothing on either side of the `@`, is no such
/// name: `@` is not a character a Layout's own name can hold, so there is no local name to confuse
/// it with.
#[must_use]
pub fn remote_spec(name: &str) -> Option<(&str, &str)> {
    let (layout, vault) = name.split_once('@')?;

    (!layout.is_empty() && !vault.is_empty()).then_some((layout, vault))
}

/// The fetched copy of the Vault `vault`'s Layout `layout`, opened for reading.
///
/// It is what a name written `layout@vault` resolves to, so it is where a query reads a Vault's
/// own Layout from: the copy lives under the Workspace — see [`readonly_layout_dir`] — and a run
/// that has not fetched it has no copy to read.
///
/// # Errors
///
/// Returns [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorLayoutArgument`] when either name is not one a directory may be keyed by,
/// [`ErrorLayoutNotCached`] when nothing was fetched under them, and [`ErrorLayoutFailed`] when a
/// copy that is there cannot be read.
pub fn cached_layout(workspace: &ResWorkspace, layout: &str, vault: &str) -> Result<Layout, Next> {
    let Some(workspace) = workspace.as_ref() else {
        return Err(ErrorLayoutShouldInWorkspace.into());
    };

    if !is_plain_name(layout) || !is_plain_name(vault) {
        return Err(ErrorLayoutArgument.into());
    }

    let dir = readonly_layout_dir(workspace, vault, layout);
    if !dir.is_dir() {
        return Err(ErrorLayoutNotCached {
            layout: layout.to_owned(),
            vault: vault.to_owned(),
        }
        .into());
    }

    Layout::open(&dir).map_err(|error| failed(&error))
}

/// Whether `name` is one path component and nothing else.
///
/// A fetched copy is keyed by the names it was fetched under, so a name that names a path — or
/// climbs, or is a drive — would read outside the cache rather than inside it.
fn is_plain_name(name: &str) -> bool {
    let mut components = std::path::Path::new(name).components();

    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

/// Error: a Layout command works on a Workspace, and this run is not inside one.
///
/// It is the placement failure already said elsewhere, under a name this namespace can return: what
/// a person is shown is the same sentence, since the two mean the same thing.
#[derive(Grouped)]
pub struct ErrorLayoutShouldInWorkspace;

impl Failure for ErrorLayoutShouldInWorkspace {
    fn name(&self) -> &'static str {
        "error_layout_should_in_workspace"
    }

    fn reason(&self) -> String {
        t!("error.placement.err_should_in_workspace")
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutShouldInWorkspace);

#[renderer(buffer)]
pub fn render_error_layout_should_in_workspace(
    _: ErrorLayoutShouldInWorkspace,
    ec: &mut ResExitCode,
) {
    r_eprintln!(
        "{}",
        err_line!(t!("error.placement.err_should_in_workspace").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("error.placement.err_should_in_workspace_help").trim())
    );
    ec.exit_code = EC_ERR_SHOULD_IN_WORKSPACE;
}

/// Error: a `rola layout` command was given arguments it cannot use.
#[derive(Grouped)]
pub struct ErrorLayoutArgument;

impl Failure for ErrorLayoutArgument {
    fn name(&self) -> &'static str {
        "error_layout_argument"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_argument").trim().to_string()
    }
}

failure!(ErrorLayoutArgument);

#[renderer(buffer)]
pub fn render_error_layout_argument(_: ErrorLayoutArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_layout_argument").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_argument_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: a hash argument does not read as a hash, or names no one object.
#[derive(Grouped)]
pub struct ErrorLayoutHash {
    /// What was given instead of a hash.
    hash: String,
    /// Why it named no one hash.
    miss: HashMiss,
}

impl Failure for ErrorLayoutHash {
    fn name(&self) -> &'static str {
        "error_layout_hash"
    }

    fn reason(&self) -> String {
        self.miss.reason(&self.hash, || {
            t!("cmd_layout.err_layout_hash", hash = self.hash)
                .trim()
                .to_string()
        })
    }
}

failure!(ErrorLayoutHash);

#[renderer(buffer)]
pub fn render_error_layout_hash(error: ErrorLayoutHash, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    let malformed = t!("cmd_layout.err_layout_hash_help").trim().to_string();
    r_eprintln!("{}", help_line!(error.miss.help(&malformed)));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// The hash `text` names, or the failure for a word that names no one.
///
/// A whole hash is taken as it is; only the head of one is resolved, and then among `candidates`.
pub fn hash_of(text: &str, candidates: impl FnOnce() -> Vec<Key>) -> Result<Key, ErrorLayoutHash> {
    resolve(text, candidates).map_err(|miss| ErrorLayoutHash {
        hash: text.to_owned(),
        miss,
    })
}

/// Error: the Workspace has no Layout to work in.
///
/// A Workspace that has just been made has none, and one whose Layouts were all taken away is back
/// to that: nothing is wrong with the arguments, and nothing is missing by name — what is missing
/// is the Layout itself, so it gets an answer of its own rather than the one for a name that does
/// not resolve.
#[derive(Grouped)]
pub struct ErrorLayoutNoCurrent;

impl Failure for ErrorLayoutNoCurrent {
    fn name(&self) -> &'static str {
        "error_layout_no_current"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_no_current").trim().to_string()
    }
}

failure!(ErrorLayoutNoCurrent);

#[renderer(buffer)]
pub fn render_error_layout_no_current(_: ErrorLayoutNoCurrent, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("cmd_layout.err_layout_no_current").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_no_current_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_NO_CURRENT;
}

/// Error: there is already a Layout by that name.
#[derive(Grouped)]
pub struct ErrorLayoutExists;

impl Failure for ErrorLayoutExists {
    fn name(&self) -> &'static str {
        "error_layout_exists"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_exists").trim().to_string()
    }
}

failure!(ErrorLayoutExists);

#[renderer(buffer)]
pub fn render_error_layout_exists(_: ErrorLayoutExists, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_layout_exists").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_exists_help").trim())
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: there is no Layout by that name.
#[derive(Grouped)]
pub struct ErrorLayoutMissing;

impl Failure for ErrorLayoutMissing {
    fn name(&self) -> &'static str {
        "error_layout_missing"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_missing").trim().to_string()
    }
}

failure!(ErrorLayoutMissing);

#[renderer(buffer)]
pub fn render_error_layout_missing(_: ErrorLayoutMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_layout_missing").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_missing_help").trim())
    );
    ec.exit_code = EC_NOT_EXIST;
}

/// Error: the Vault a Layout was to track is not one the Workspace has bound.
#[derive(Grouped)]
pub struct ErrorLayoutTrackNotBound {
    /// The name that is not bound.
    pub track: String,
}

impl Failure for ErrorLayoutTrackNotBound {
    fn name(&self) -> &'static str {
        "error_layout_track_not_bound"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_track_not_bound", track = self.track)
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutTrackNotBound);

#[renderer(buffer)]
pub fn render_error_layout_track_not_bound(error: ErrorLayoutTrackNotBound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_track_not_bound_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// What a Layout that could not be worked on is answered with.
///
/// A name already taken and a name with nothing behind it are told apart, since what is to be done
/// about them differs; everything else is the library's own words, under the one exit code.
pub fn failed(error: &LayoutError) -> Next {
    match error {
        LayoutError::AlreadyExists => ErrorLayoutExists.into(),
        LayoutError::NotFound => ErrorLayoutMissing.into(),
        other => ErrorLayoutFailed {
            cause: other.to_string(),
        }
        .into(),
    }
}

/// The `--layout` of a command that works on one Layout.
///
/// The name is written once, here: the parse reads it and every completion that answers for a
/// Layout offers it, so the two cannot come to disagree about what the flag is called.
pub const ARG_LAYOUT: PickerArg<'static, Option<String>> = arg![layout: Option<String>];

/// What a Layout-content command did, for the one sentence that says so.
#[derive(Grouped, Clone, Copy)]
pub enum LayoutDid {
    /// An entry was made.
    Created,
    /// An entry's data was changed.
    Updated,
    /// An entry was dropped.
    Removed,
    /// A path was bound.
    Bound,
    /// A path was unbound.
    Unbound,
    /// A path was moved.
    Moved,
}

/// Result: a Layout's content was changed.
#[derive(Grouped)]
pub struct ResultLayoutContent {
    /// What was done.
    pub did: LayoutDid,
    /// What it was done to.
    pub what: String,
}

#[renderer(buffer)]
pub fn render_result_layout_content(result: ResultLayoutContent) {
    let said = match result.did {
        LayoutDid::Created => t!("cmd_layout_entry.result_created", what = result.what),
        LayoutDid::Updated => t!("cmd_layout_entry.result_updated", what = result.what),
        LayoutDid::Removed => t!("cmd_layout_entry.result_removed", what = result.what),
        LayoutDid::Bound => t!("cmd_layout_path.result_bound", what = result.what),
        LayoutDid::Unbound => t!("cmd_layout_path.result_unbound", what = result.what),
        LayoutDid::Moved => t!("cmd_layout_path.result_moved", what = result.what),
    };

    r_println!("{}", said.trim());
}

/// Where a Workspace keeps the read-only copy of a Vault's Layout.
///
/// The cache is a directory per Vault and a directory per Layout under it, both named by what the
/// copy was fetched under, so a command that reads one reaches the same place a
/// [`rola layout fetch`](crate::layout::cmd_layout_fetch) wrote — see
/// [`WORKSPACE_READONLY_LAYOUTS_DIR`]. A Vault keeps one Layout, and what it is known by is
/// [`VAULT_LAYOUT_NAME`].
#[must_use]
pub fn readonly_layout_dir(workspace: &Workspace, vault: &str, layout: &str) -> std::path::PathBuf {
    workspace
        .get_root()
        .join(WORKSPACE_READONLY_LAYOUTS_DIR)
        .join(vault)
        .join(layout)
}

/// The Vault and the `Uuid` an ownership command was given, from the words it was given.
///
/// `VAULT` comes first and may be left out, so one word is the `Uuid` and two are the Vault and
/// then the `Uuid`. Anything else — none, or more than two — is not a way to name one entry, and
/// a word where the `Uuid` belongs that does not read as one is not either.
#[must_use]
pub fn vault_and_uuid(words: &[String]) -> Option<(Option<String>, Uuid)> {
    let (vault, uuid) = match words {
        [uuid] => (None, uuid),
        [vault, uuid] => (Some(vault.clone()), uuid),
        _ => return None,
    };

    Uuid::parse_str(uuid).ok().map(|id| (vault, id))
}

/// The Vault and the `Uuid`s an ownership command was given, from the words it was given.
///
/// `VAULT` comes first and may be left out. Nothing is both a Vault name and a `Uuid`, so the first
/// word is read as the Vault exactly when it does not read as a `Uuid`; every word after it has to,
/// and so does the first when there is no Vault. A run that names no `Uuid` at all names nothing.
#[must_use]
pub fn vault_and_uuids(words: &[String]) -> Option<(Option<String>, Vec<Uuid>)> {
    let (vault, uuids) = match words.split_first() {
        Some((first, rest)) if Uuid::parse_str(first).is_err() => (Some(first.clone()), rest),
        _ => (None, words),
    };

    if uuids.is_empty() {
        return None;
    }

    uuids
        .iter()
        .map(|uuid| Uuid::parse_str(uuid).ok())
        .collect::<Option<Vec<Uuid>>>()
        .map(|ids| (vault, ids))
}

/// Names the entry `id` in the fetched copy as held by `owner`, when the copy holds it.
///
/// What is changed is the Vault's own Layout — the one known as [`VAULT_LAYOUT_NAME`] — since that
/// is the copy an ownership exchange is about. Nothing is a failure here but a copy that cannot be
/// read or written: a run may ask for ownership without ever having fetched, and one that has
/// fetched may hold a copy that does not name the entry yet — the Vault is the truth, and the copy
/// is brought up to date by `rola layout fetch`, not by this.
///
/// # Errors
///
/// Returns [`LayoutError`] if a copy that is there cannot be read back or written.
pub fn set_cached_owner(
    workspace: &Workspace,
    vault: &str,
    id: Uuid,
    owner: Option<String>,
) -> Result<(), LayoutError> {
    let dir = readonly_layout_dir(workspace, vault, VAULT_LAYOUT_NAME);
    if !dir.is_dir() {
        return Ok(());
    }

    let layout = Layout::open(&dir)?;
    let Some(data) = layout.entry(id) else {
        return Ok(());
    };

    layout.update_entry(
        id,
        MutableData::new(owner, data.version(), data.description().to_owned()),
    )
}

/// Moves the path `from` to `to` in the fetched copy, when the copy is there and names `from`.
///
/// It follows a move the Vault accepted, so that a copy read afterwards says what the Vault says —
/// the same reason [`set_cached_owner`] follows an ownership change. A copy that is not there, or
/// one that does not name the path, is left as it is: the Vault is the truth, and the copy is
/// brought up to date by `rola layout fetch`.
///
/// # Errors
///
/// Returns [`LayoutError`] if a copy that is there cannot be read back or written.
pub fn set_cached_path(
    workspace: &Workspace,
    vault: &str,
    layout: &str,
    from: &LayoutPath,
    to: &LayoutPath,
) -> Result<(), LayoutError> {
    let dir = readonly_layout_dir(workspace, vault, layout);
    if !dir.is_dir() {
        return Ok(());
    }

    let copy = Layout::open(&dir)?;
    if copy.id_of(from).is_none() {
        return Ok(());
    }

    copy.move_path(from, to)
}

/// Adds the entry `id` to the fetched copy, at `path` and `version`, held by `owner`.
///
/// It is what a run that has just made an entry in the Vault writes down here, so that a reading
/// command sees it without a `rola layout fetch` of its own. A copy that is not there, or one that
/// already holds the `Uuid` or names the path, is left as it is: the Vault is the truth, and a
/// fetch is what settles a copy that has drifted, not this.
///
/// # Errors
///
/// Returns [`LayoutError`] if a copy that is there cannot be read back or written.
pub fn add_cached_entry(
    workspace: &Workspace,
    vault: &str,
    id: Uuid,
    path: &LayoutPath,
    version: [u8; 32],
    owner: Option<String>,
) -> Result<(), LayoutError> {
    let dir = readonly_layout_dir(workspace, vault, VAULT_LAYOUT_NAME);
    if !dir.is_dir() {
        return Ok(());
    }

    let copy = Layout::open(&dir)?;
    if copy.entry(id).is_some() || copy.id_of(path).is_some() {
        return Ok(());
    }

    copy.create_path(path, id)?;
    copy.create_entry(id, MutableData::new(owner, version, String::new()))
}

/// Moves the entry `id` of the fetched copy to `version`, leaving its path and holder alone.
///
/// It is what a run that has just set a version in the Vault writes down here, for the same reason
/// [`add_cached_entry`] does.
///
/// # Errors
///
/// Returns [`LayoutError`] if a copy that is there cannot be read back or written.
pub fn set_cached_version(
    workspace: &Workspace,
    vault: &str,
    id: Uuid,
    version: [u8; 32],
) -> Result<(), LayoutError> {
    let dir = readonly_layout_dir(workspace, vault, VAULT_LAYOUT_NAME);
    if !dir.is_dir() {
        return Ok(());
    }

    let copy = Layout::open(&dir)?;
    let Some(data) = copy.entry(id) else {
        return Ok(());
    };

    copy.update_entry(
        id,
        MutableData::new(
            data.owner().map(str::to_owned),
            version,
            data.description().to_owned(),
        ),
    )
}

/// Error: the Vault's Layout has not been fetched, so there is no copy to read.
#[derive(Grouped)]
pub struct ErrorLayoutNotCached {
    /// The Layout that was not fetched.
    pub layout: String,
    /// The Vault whose Layout was not fetched.
    pub vault: String,
}

impl Failure for ErrorLayoutNotCached {
    fn name(&self) -> &'static str {
        "error_layout_not_cached"
    }

    fn reason(&self) -> String {
        t!(
            "cmd_layout.err_not_cached",
            layout = self.layout,
            vault = self.vault
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorLayoutNotCached);

#[renderer(buffer)]
pub fn render_error_layout_not_cached(error: ErrorLayoutNotCached, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_not_cached_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_NOT_CACHED;
}

/// Error: a fetched copy of a Vault's Layout is named for a command that changes a Layout.
///
/// A copy is a reading of what the Vault held when it was fetched, not a place to work: changing it
/// would change what a query answers with, and nothing of it would reach the Vault.
#[derive(Grouped)]
pub struct ErrorLayoutReadOnly {
    /// The Layout that is a copy.
    pub layout: String,
}

impl Failure for ErrorLayoutReadOnly {
    fn name(&self) -> &'static str {
        "error_layout_read_only"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_read_only", layout = self.layout)
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutReadOnly);

#[renderer(buffer)]
pub fn render_error_layout_read_only(error: ErrorLayoutReadOnly, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("cmd_layout.err_read_only_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT;
}

/// Error: a path is already the name of another entry of the Vault's Layout.
#[derive(Grouped)]
pub struct ErrorLayoutPathTaken;

impl Failure for ErrorLayoutPathTaken {
    fn name(&self) -> &'static str {
        "error_layout_path_taken"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_path_taken").trim().to_string()
    }
}

failure!(ErrorLayoutPathTaken);

#[renderer(buffer)]
pub fn render_error_layout_path_taken(error: ErrorLayoutPathTaken, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_path_taken_help").trim())
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: the Vault's Layout holds the entry, but not for this account to move.
#[derive(Grouped)]
pub struct ErrorLayoutMoveRefused;

impl Failure for ErrorLayoutMoveRefused {
    fn name(&self) -> &'static str {
        "error_layout_move_refused"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_move_refused").trim().to_string()
    }
}

failure!(ErrorLayoutMoveRefused);

#[renderer(buffer)]
pub fn render_error_layout_move_refused(error: ErrorLayoutMoveRefused, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_move_refused_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the fetched copy names no entry by that `Uuid`.
#[derive(Grouped)]
pub struct ErrorOwnershipUnknown {
    /// The `Uuid` the copy names nothing by.
    pub uuid: String,
}
impl Failure for ErrorOwnershipUnknown {
    fn name(&self) -> &'static str {
        "error_ownership_unknown"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_ownership_unknown", uuid = self.uuid)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipUnknown);

#[renderer(buffer)]
pub fn render_error_ownership_unknown(error: ErrorOwnershipUnknown, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_ownership_unknown_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_NOT_CACHED;
}

/// Error: the Vault holds no entry by that `Uuid`.
#[derive(Grouped)]
pub struct ErrorOwnershipMissing {
    /// The `Uuid` the Vault holds nothing by.
    pub uuid: String,
}

impl Failure for ErrorOwnershipMissing {
    fn name(&self) -> &'static str {
        "error_ownership_missing"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_ownership_missing", uuid = self.uuid)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipMissing);

#[renderer(buffer)]
pub fn render_error_ownership_missing(error: ErrorOwnershipMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_ownership_missing_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_NOT_CACHED;
}

/// Error: another account holds the entry, so its ownership was left alone.
#[derive(Grouped)]
pub struct ErrorOwnershipHeld {
    /// The `Uuid` that is held.
    pub uuid: String,
    /// The account that holds it.
    pub owner: String,
}

impl Failure for ErrorOwnershipHeld {
    fn name(&self) -> &'static str {
        "error_ownership_held"
    }

    fn reason(&self) -> String {
        t!(
            "cmd_layout.err_ownership_held",
            uuid = self.uuid,
            owner = self.owner
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorOwnershipHeld);

#[renderer(buffer)]
pub fn render_error_ownership_held(error: ErrorOwnershipHeld, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_ownership_held_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{remote_spec, vault_and_uuid};

    /// A name written `NAME@VAULT` names a Vault's Layout; one that is not written that way is a
    /// name of the Workspace's own, and `@` is what tells them apart.
    #[test]
    fn a_vault_layout_is_named_by_an_at_sign() {
        assert_eq!(remote_spec("truth@origin"), Some(("truth", "origin")));
        // The first `@` is the one that separates: a name after it may hold one of its own.
        assert_eq!(remote_spec("truth@my@vault"), Some(("truth", "my@vault")));
        // A name that holds no `@` is local, and one that names nothing on a side names nothing.
        assert_eq!(remote_spec("main"), None);
        assert_eq!(remote_spec("@origin"), None);
        assert_eq!(remote_spec("truth@"), None);
    }

    /// A `Uuid` on its own is the entry; a word before it is the Vault. Anything else names no
    /// entry at all, which is what a run that can do nothing is told.
    #[test]
    fn an_entry_is_named_by_a_uuid_and_maybe_a_vault_before_it() {
        let id = Uuid::from_u128(7);
        let text = id.to_string();

        assert_eq!(
            vault_and_uuid(std::slice::from_ref(&text)),
            Some((None, id))
        );
        assert_eq!(
            vault_and_uuid(&["origin".to_owned(), text]),
            Some((Some("origin".to_owned()), id))
        );

        assert_eq!(vault_and_uuid(&[]), None);
        assert_eq!(vault_and_uuid(&["origin".to_owned()]), None);
        assert_eq!(
            vault_and_uuid(&["a".to_owned(), "b".to_owned(), "c".to_owned()]),
            None
        );
        // A word where the `Uuid` belongs that is not one names no entry, whatever else it is.
        assert_eq!(
            vault_and_uuid(&["origin".to_owned(), "not-a-uuid".to_owned()]),
            None
        );
    }
}
