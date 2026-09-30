//! The `rola retrack` command: move the version a file's Layout entry is at.
//!
//! A Layout names one version for each path, and the content behind a version stays in the index
//! whether or not anything points at it. So putting a file's record back where it was is one
//! pointer moving: the `Uuid`, the path and the description are left as they are, and nothing is
//! removed from the index.
//!
//! The file on disk is not touched unless `--restore-content` says so. That is deliberate — a run
//! that only wants the record to say something else leaves the work alone, and what the tree then
//! holds is newer than the version named, which is what `rola status` reports as a change that was
//! never recorded. With `--restore-content` the version's content is written back over the file
//! and the reading is told the two agree again, the way [`crate::checkout`] tells it for a pull.

// A retrack reads a level, a Layout, a chain and a version before it moves one pointer, and every
// step of that is a refusal of its own to say; keeping them in one function is what lets the order
// they are read in be read off the function rather than assembled from helpers.
#![allow(clippy::too_many_lines)]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr as _;

use librorolala::daemon::action_fetch_layout;
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::storage::{Blake3Hash, Key, StorageBackend as _};
use librorolala::tree_analyze::{Cache, cache_path, entry_of};
use librorolala::vcs::{VCSIndex, VCSWrite as _, Variant, Version};
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResCurrentRemoteVault, ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace,
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
use crate::exit_codes::{EC_ERR_RETRACK, EC_ERR_RETRACK_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutShouldInWorkspace, chosen, failed as layout_failed, readonly_layout_dir, remote_spec,
};

/// The flags `rola retrack` takes.
#[derive(Pickable)]
struct RetrackFlags {
    /// Where the pointer goes, one version back unless said.
    #[arg(long)]
    until: Option<String>,
    /// Write the version's content back over the file as well.
    #[arg(long)]
    restore_content: Flag,
    /// Move the pointer to a version that is not on the way back to where it is now.
    #[arg(long)]
    allow_jump: Flag,
}

#[help(buffer)]
pub fn help_retrack(_: EntryRetrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("retrack.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryRetrack)]
pub fn desc_retrack() -> Description {
    t!("retrack.description").to_string().into()
}

/// Moves a file's recorded version back, in the Layout being worked in
///
/// Only the entry's version pointer moves: its `Uuid`, its path and its description stay as they
/// are, and nothing is removed from the index. `FILE` is one local path in this Workspace — a
/// `Uuid`, a hash and a remote name are refused — and it has to be a path the Layout records.
///
/// `--until` is where the pointer goes, and is one version back when it is not given: `-1` one
/// version back, `~N` N versions back, `N` the version numbered N, a hash a version on the way
/// back from where the entry is now, and a Vault's name the version that Vault's own Layout
/// records for the file. A hash that is not on the way back is refused unless `--allow-jump` is
/// given; `~N` and `N` past either end of the chain are always refused.
///
/// Without `--restore-content` the file on disk is left alone, so it is then newer than the
/// version the Layout names and `rola status` reports it as changed. With it, the version's
/// content is written back over the file.
///
/// # Errors
///
/// Renders [`ErrorRetrackNoFile`] when no file or several are named, [`ErrorRetrackFile`] when what
/// was named is not a local path the Layout records, [`ErrorRetrackLevel`] when `--until` names no
/// version the entry can reach, [`ErrorRetrackNothing`] when the entry was never recorded,
/// [`ErrorRetrackUpstream`] when the Vault's Layout records no version for it,
/// [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorRetrackFailed`] when the store, the index or the Layout refuses.
#[command(node = "retrack", entry = EntryRetrack)]
pub fn retrack(args: EntryRetrack) -> Next {
    let picked = args
        .pick(&arg![RetrackFlags])
        .pick_or_route(&arg![Vec<String>], || ErrorRetrackNoFile.into())
        .to_result();
    let (flags, mut files) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    // One file is the whole of what this moves, so naming more is not a run this reads as one of
    // its own: it is refused rather than acted on for the first and quietly ignoring the rest.
    if files.len() > 1 {
        return ErrorRetrackManyFiles { count: files.len() }.into();
    }

    let Some(file) = files.pop() else {
        return ErrorRetrackNoFile.into();
    };

    StateRetrack {
        file,
        until: flags.until,
        restore_content: matches!(flags.restore_content, Flag::Active),
        allow_jump: matches!(flags.allow_jump, Flag::Active),
    }
    .into()
}

/// The state a retracking starts in.
#[derive(Grouped)]
pub struct StateRetrack {
    /// The one file named, as it was written.
    file: String,
    /// Where the pointer goes, as it was written; one version back when it was not said.
    until: Option<String>,
    /// Whether the version's content is written back over the file.
    restore_content: bool,
    /// Whether a version off the way back may be jumped to.
    allow_jump: bool,
}

#[chain(routeify)]
pub fn handle_retrack(
    state: StateRetrack,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let StateRetrack {
        file,
        until,
        restore_content,
        allow_jump,
    } = state;

    // The level is read before anything is looked at, so a run that misspelled it is told that
    // rather than a complaint about a file it would have refused for another reason.
    let level = until_of(until.as_deref())?;
    let text = until.unwrap_or_else(|| "-1".to_owned());

    let layout = chosen(workspace.get_ref(), vault.get_ref(), None)?;

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };

    let root = held.get_root().to_path_buf();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return failed(error.to_string()),
    };
    let path = locate(&root, &cwd, &file)?;

    let Some(id) = layout.id_of(&path) else {
        return ErrorRetrackFile::new(&file, RetrackFileError::NotRecorded).into();
    };
    let Some(data) = layout.entry(id) else {
        return ErrorRetrackFile::new(&file, RetrackFileError::NotRecorded).into();
    };

    // The root version is what an entry is at before anything was recorded for it, so it is no
    // version to go back to: a file the Layout names this way has no record yet.
    if data.version() == [0; 32] {
        return ErrorRetrackNothing {
            path: path.as_str().to_owned(),
        }
        .into();
    }

    let Some(vcs) = index.get_ref().as_ref() else {
        return failed(t!("retrack.err_no_index").trim().to_owned());
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return failed(error.to_string()),
    };

    let here = read_version(vcs, &runtime, data.version())?;
    let chain = ancestors(vcs, &runtime, &here)?;

    // The one level that leaves the local index for what the Vault's own Layout says. It is read
    // before the rest only for this level, since reading it may mean reaching for a Vault.
    let upstream = match &level {
        Until::Vault(alias) => Some(upstream_version(held, remote, current, alias, &path, id)?),
        _ => None,
    };

    let asked = Asked {
        text: &text,
        level: &level,
        path: path.as_str(),
        allow_jump,
    };
    let (target, number) = resolve_target(vcs, &runtime, &chain, &asked, upstream)?;

    let current_hash = data.version();
    let moved = target != current_hash;

    if moved {
        let owner = data.owner().map(str::to_owned);
        let description = data.description().to_owned();

        if let Err(error) = layout.update_entry(id, MutableData::new(owner, target, description)) {
            return layout_failed(&error);
        }
    }

    if restore_content {
        restore(storage, vcs, &runtime, &layout, &root, &path, target)?;
    } else if moved {
        // What the file is to agree with now, which is the content the version moved to holds. One
        // the index does not hold is nothing to read, and the agreement is then taken back.
        let agreed = read_version(vcs, &runtime, target)
            .ok()
            .and_then(|version| stored_hash(vcs, &runtime, &version).ok());

        disagree(&layout, &root, &path, agreed)?;
    }

    ResultRetrack {
        path: path.as_str().to_owned(),
        version: hex(target),
        number,
        moved,
        restored: restore_content,
    }
    .into()
}

/// What a run asked the pointer to be moved to, and what a refusal about it says.
struct Asked<'a> {
    /// The level as it was written, for the words a refusal uses.
    text: &'a str,
    /// Where the pointer is to go.
    level: &'a Until,
    /// The path the entry is at, for the words a refusal uses.
    path: &'a str,
    /// Whether a version off the way back may be jumped to.
    allow_jump: bool,
}

/// The version the pointer is to be moved to, and its number when it is known.
///
/// `upstream` is the Vault's version for the [`Until::Vault`] level, read before this because only
/// that level leaves the local index; every other level is worked out from the chain the entry is
/// the top of.
fn resolve_target(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    chain: &[Version],
    asked: &Asked<'_>,
    upstream: Option<Blake3Hash>,
) -> Result<(Blake3Hash, Option<u64>), Next> {
    let top = chain.len() - 1;
    let out_of_chain = || {
        ErrorRetrackLevel {
            until: asked.text.to_owned(),
            kind: RetrackLevelError::OutOfChain {
                path: asked.path.to_owned(),
            },
        }
        .into()
    };

    match asked.level {
        Until::Steps(back) => {
            let Ok(back) = usize::try_from(*back) else {
                return Err(out_of_chain());
            };
            let Some(version) = chain.get(back) else {
                return Err(out_of_chain());
            };

            Ok((*version.hash().digest(), Some(number_at(top, back))))
        }
        Until::Number(number) => {
            let Ok(number) = usize::try_from(*number) else {
                return Err(out_of_chain());
            };
            if number > top {
                return Err(out_of_chain());
            }

            let at = top - number;
            Ok((
                *chain[at].hash().digest(),
                Some(u64::try_from(number).unwrap_or(u64::MAX)),
            ))
        }
        Until::Hash(key) => {
            if let Some((at, version)) = chain
                .iter()
                .enumerate()
                .find(|(_, version)| version.hash().digest() == key.digest())
            {
                return Ok((*version.hash().digest(), Some(number_at(top, at))));
            }

            let Ok(version) = read_version(index, runtime, *key.digest()) else {
                return Err(ErrorRetrackLevel {
                    until: asked.text.to_owned(),
                    kind: RetrackLevelError::NotAVersion,
                }
                .into());
            };

            if !asked.allow_jump {
                return Err(ErrorRetrackLevel {
                    until: asked.text.to_owned(),
                    kind: RetrackLevelError::NotAncestor {
                        path: asked.path.to_owned(),
                    },
                }
                .into());
            }

            let number = runtime.block_on(index.version_num(&version)).ok();
            Ok((*key.digest(), number))
        }
        Until::Vault(_) => {
            // UNWRAP: the caller reads the Vault's version for exactly this level, so one is here.
            let hash = upstream.expect("the Vault's version is read for the Vault level");
            let number = read_version(index, runtime, hash)
                .ok()
                .and_then(|version| runtime.block_on(index.version_num(&version)).ok());

            Ok((hash, number))
        }
    }
}

/// The version the Vault `alias` names records for the entry `id`.
///
/// What is read is the copy of the Vault's own Layout the Workspace keeps, and one that was never
/// fetched is fetched first — the only step of a retrack that may leave the machine. Which Vault
/// `alias` names is the Workspace's to say, and no check is made that the Layout being worked in
/// tracks that Vault: the Vault named is the upstream, whichever it is.
fn upstream_version(
    held: &Workspace,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    alias: &str,
    path: &LayoutPath,
    id: Uuid,
) -> Result<Blake3Hash, Next> {
    let name = remote
        .get_ref()
        .name_or_default(alias)
        .map_err(mingling::Routable::to_chain)?;

    let dir = readonly_layout_dir(held, &name, VAULT_LAYOUT_NAME);

    // A copy that is here is read as it is, however old: reaching for the Vault is what a run that
    // has not fetched is for, not something every run pays for.
    if !dir.is_dir() {
        let target = remote
            .get_ref()
            .vault_or_default(name.clone())
            .map_err(mingling::Routable::to_chain)?;
        let account_name = current.get_ref().must_bind().map_err(Into::<Next>::into)?;
        let account = account_named(&account_name, Some(held), None).map_err(Into::<Next>::into)?;

        action_fetch_layout(held, &account, target.to_string(), name.clone())
            .map_err(mingling::Routable::to_chain)?;
    }

    let copy = Layout::open(&dir).map_err(|error| failed(error.to_string()))?;

    let Some(data) = copy.entry(id) else {
        return Err(ErrorRetrackUpstream {
            vault: name,
            path: path.as_str().to_owned(),
        }
        .into());
    };

    // The root version is what an entry is at before anything was recorded, so an upstream that
    // names it records no version either: there is nothing to be consistent with.
    if data.version() == [0; 32] {
        return Err(ErrorRetrackUpstream {
            vault: name,
            path: path.as_str().to_owned(),
        }
        .into());
    }

    Ok(data.version())
}

/// Writes the version's content back over the file, and remembers that the two agree.
fn restore(
    storage: &mut LazyRes<ResRorolalaStorage>,
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    layout: &Layout,
    root: &Path,
    path: &LayoutPath,
    version: Blake3Hash,
) -> Result<(), Next> {
    let Some(store) = storage.get_ref().as_ref() else {
        return Err(failed(t!("retrack.err_no_store").trim().to_owned()));
    };

    let version = read_version(vcs, runtime, version)?;
    let recorded = stored_hash(vcs, runtime, &version)?;
    let disk = root.join(path.to_path_buf());

    if let Some(parent) = disk.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return Err(failed(error.to_string()));
    }

    if let Err(error) = runtime.block_on(store.extract_file(&Key::new(recorded), &disk)) {
        return Err(failed(error.to_string()));
    }

    // What was written is what the Layout names again, so the reading has to be told: a path left
    // marked as disagreed would be reported as changed by the very next reading.
    if let Err(cause) = remember(layout, root, path.as_str()) {
        return Err(failed(cause));
    }

    Ok(())
}

/// Writes down what the file is to agree with now that the Layout names an older version.
///
/// Moving the pointer and leaving the file alone leaves the two apart — the file is what the newer
/// version held, and the Layout names an older one — but the reading does not tell that from the
/// pointer: what it compares is the file against what the Layout was last known to agree with. So
/// the agreement is written afresh here rather than left standing, or the very next `rola status`
/// would call a file with changes never recorded one with nothing to record.
///
/// What is written is the content the new version holds, so a file that is then put back to it by
/// hand reads as agreed again — a plain "nothing" would leave such a file reported as changed with
/// `rola track` having nothing to say about it. The content of a version the index does not hold is
/// nothing that can be read, so the agreement is taken back rather than guessed at.
///
/// A path the tree does not hold is nothing to write down: it is read as lost however the cache
/// stands, and there is no file to read a stamp from.
fn disagree(
    layout: &Layout,
    root: &Path,
    path: &LayoutPath,
    agreed: Option<Blake3Hash>,
) -> Result<(), Next> {
    let disk = root.join(path.to_path_buf());
    if !disk.is_file() {
        return Ok(());
    }

    let file = cache_path(root, layout);
    let mut cache = Cache::read(&file);
    let entry = entry_of(&disk).map_err(|error| failed(error.to_string()))?;

    cache.insert(path.clone(), entry.with_agreed(agreed));
    cache
        .write(&file)
        .map_err(|error| failed(error.to_string()))
}

/// The versions from `top` back to the first, the top first.
fn ancestors(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    top: &Version,
) -> Result<Vec<Version>, Next> {
    let mut chain = Vec::new();
    let mut current = top.clone();

    while !current.is_root() {
        let variant = read_variant(vcs, runtime, *current.variant())?;
        chain.push(current);
        current = read_version(vcs, runtime, *variant.base_version())?;
    }

    Ok(chain)
}

/// The number of the version `at` steps back from a chain whose top is numbered `top`.
fn number_at(top: usize, at: usize) -> u64 {
    u64::try_from(top.saturating_sub(at)).unwrap_or(u64::MAX)
}

/// The `Version` stored under `hash`.
fn read_version(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: Blake3Hash,
) -> Result<Version, Next> {
    runtime
        .block_on(vcs.read(Key::new(hash)))
        .map_err(|error| failed(error.reason()))?
        .expect_version()
        .map_err(|error| failed(error.to_string()))
}

/// The `Variant` stored under `hash`.
fn read_variant(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: Blake3Hash,
) -> Result<Variant, Next> {
    runtime
        .block_on(vcs.read(Key::new(hash)))
        .map_err(|error| failed(error.reason()))?
        .expect_variant()
        .map_err(|error| failed(error.to_string()))
}

/// The stored hash the variant of `version` points at.
fn stored_hash(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    version: &Version,
) -> Result<Blake3Hash, Next> {
    let variant = read_variant(vcs, runtime, *version.variant())?;

    Ok(*variant.storage_hash())
}

/// The path the run named, read as one the Layout being worked in names.
///
/// Only a local path is taken. A `Uuid`, a hash and a remote name are refused by shape rather than
/// answered as a path that happens not to be there, since a run that wrote one meant the other
/// command that takes one. A name holds `@` often enough — `logo@2x.png` — that a plain one is
/// left to be a path: what is refused is the Vault-side marker and the one name a Vault's Layout
/// is kept under.
fn locate(root: &Path, cwd: &Path, given: &str) -> Result<LayoutPath, Next> {
    if Uuid::from_str(given).is_ok() || Key::from_str(given).is_ok() || is_remote(given) {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::NotLocal).into());
    }

    let root = normalize(root);
    let disk = normalize(&resolve(cwd, given));

    let Ok(relative) = disk.strip_prefix(&root) else {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::OutsideWorkspace).into());
    };

    let Ok(path) = LayoutPath::from_relative(relative) else {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::OutsideWorkspace).into());
    };

    if path.as_str().split('/').any(|part| part == ".rola") {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::InsideData).into());
    }

    Ok(path)
}

/// Whether `given` is a name of the Vault's side rather than a path in this tree.
fn is_remote(given: &str) -> bool {
    // `@` is the marker a Vault's own path starts with; `truth@VAULT` is how a Vault's one Layout
    // is named. A file of one's own may hold an `@` and is left to be read as the path it is.
    given.starts_with('@')
        || matches!(remote_spec(given), Some((layout, _)) if layout == VAULT_LAYOUT_NAME)
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// `path` with its `.` dropped and its `..` climbed, worked out lexically rather than on disk.
///
/// A name is written against where the run was made, so `..` climbs out of the Workspace as
/// readily as it climbs into it, and what is then left is refused for being outside it.
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

/// Where a run's `--until` is to move the pointer to.
enum Until {
    /// N versions back.
    Steps(u64),
    /// The version numbered N.
    Number(u64),
    /// The version a hash names.
    Hash(Key),
    /// The version the Vault named records.
    Vault(String),
}

/// Reads what a run gave `--until` as where the pointer is to go.
///
/// One step back is what saying nothing means. `~N` is a step count, digits are a version number,
/// a hash is a version, and anything else is taken as a Vault's name — the order a run is likely
/// to have meant, with the two shapes a Vault's name can never be settled first.
fn until_of(text: Option<&str>) -> Result<Until, ErrorRetrackLevel> {
    let Some(text) = text else {
        return Ok(Until::Steps(1));
    };

    let not_a_level = || ErrorRetrackLevel {
        until: text.to_owned(),
        kind: RetrackLevelError::NotALevel,
    };

    if text == "-1" {
        return Ok(Until::Steps(1));
    }

    // Nothing at all names no Vault either: a name is something, and `--until=` is not.
    if text.is_empty() {
        return Err(not_a_level());
    }

    if let Some(rest) = text.strip_prefix('~') {
        return rest
            .parse::<u64>()
            .map(Until::Steps)
            .map_err(|_| not_a_level());
    }

    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        return text
            .parse::<u64>()
            .map(Until::Number)
            .map_err(|_| not_a_level());
    }

    // A sign that is not the `-1` above reads as an option or a mistyped step count, never as a
    // Vault's name: a name is not what a run wrote.
    if text.starts_with('-') || text.starts_with('+') {
        return Err(not_a_level());
    }

    if let Ok(key) = Key::from_str(text) {
        return Ok(Until::Hash(key));
    }

    Ok(Until::Vault(text.to_owned()))
}

/// The stored hash `hash` is written as, in hex.
fn hex(hash: Blake3Hash) -> String {
    Key::new(hash).hex()
}

/// The answer a store, an index or a Layout refusing is given.
fn failed(cause: impl Into<String>) -> Next {
    ErrorRetrackFailed {
        cause: cause.into(),
    }
    .into()
}

/// Names a version for a person: its number when it is known, and the head of its hash.
fn named(version: &str, number: Option<u64>) -> String {
    let short = &version[..version.len().min(7)];

    number.map_or_else(|| short.to_owned(), |number| format!("V{number} ({short})"))
}

/// Result: a file's version pointer was moved, or was already where it was asked to go.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultRetrack {
    /// Where the entry sits in the Layout.
    path: String,
    /// The version it is at now, as hex.
    version: String,
    /// That version's number in its chain, when it is known.
    number: Option<u64>,
    /// Whether the pointer moved.
    moved: bool,
    /// Whether the version's content was written back over the file.
    restored: bool,
}

#[renderer(buffer)]
pub fn render_result_retrack(result: ResultRetrack) {
    let version = named(&result.version, result.number);
    let said = match (result.moved, result.restored) {
        (true, false) => t!(
            "retrack.result_retracked",
            path = result.path,
            version = version
        ),
        (true, true) => t!(
            "retrack.result_retracked_restored",
            path = result.path,
            version = version
        ),
        (false, false) => t!(
            "retrack.result_unchanged",
            path = result.path,
            version = version
        ),
        (false, true) => t!(
            "retrack.result_unchanged_restored",
            path = result.path,
            version = version
        ),
    };

    r_println!("{}", said.trim());
}

/// Error: no file was named.
#[derive(Grouped)]
pub struct ErrorRetrackNoFile;

impl Failure for ErrorRetrackNoFile {
    fn name(&self) -> &'static str {
        "error_retrack_no_file"
    }

    fn reason(&self) -> String {
        t!("retrack.err_no_file").trim().to_string()
    }
}

failure!(ErrorRetrackNoFile);

#[renderer(buffer)]
pub fn render_error_retrack_no_file(_: ErrorRetrackNoFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("retrack.err_no_file").trim()));
    r_eprintln!("{}", help_line!(t!("retrack.err_no_file_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: more than one file was named.
#[derive(Grouped)]
pub struct ErrorRetrackManyFiles {
    /// How many were named.
    count: usize,
}

impl Failure for ErrorRetrackManyFiles {
    fn name(&self) -> &'static str {
        "error_retrack_many_files"
    }

    fn reason(&self) -> String {
        t!("retrack.err_many_files", count = self.count)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackManyFiles);

#[renderer(buffer)]
pub fn render_error_retrack_many_files(error: ErrorRetrackManyFiles, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_many_files_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: a file named cannot be retracked.
#[derive(Grouped)]
pub struct ErrorRetrackFile {
    /// What was named.
    path: String,
    /// What is wrong with it.
    kind: RetrackFileError,
}

impl ErrorRetrackFile {
    /// The failure of retracking `path`, for the reason `kind` names.
    fn new(path: &str, kind: RetrackFileError) -> Self {
        Self {
            path: path.to_owned(),
            kind,
        }
    }
}

/// The ways a named file cannot be retracked.
pub enum RetrackFileError {
    /// It is a `Uuid`, a hash or a remote name rather than a local path.
    NotLocal,
    /// It is not inside the Workspace.
    OutsideWorkspace,
    /// It is part of the Workspace's own data rather than its work.
    InsideData,
    /// The Layout names nothing there.
    NotRecorded,
}

impl Failure for ErrorRetrackFile {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackFileError::NotLocal => "error_retrack_not_local",
            RetrackFileError::OutsideWorkspace => "error_retrack_outside_workspace",
            RetrackFileError::InsideData => "error_retrack_inside_data",
            RetrackFileError::NotRecorded => "error_retrack_not_recorded",
        }
    }

    fn reason(&self) -> String {
        let said = match self.kind {
            RetrackFileError::NotLocal => t!("retrack.err_not_local", path = self.path),
            RetrackFileError::OutsideWorkspace => {
                t!("retrack.err_outside_workspace", path = self.path)
            }
            RetrackFileError::InsideData => t!("retrack.err_inside_data", path = self.path),
            RetrackFileError::NotRecorded => t!("retrack.err_not_recorded", path = self.path),
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackFile);

#[renderer(buffer)]
pub fn render_error_retrack_file(error: ErrorRetrackFile, ec: &mut ResExitCode) {
    let help = match error.kind {
        RetrackFileError::NotLocal | RetrackFileError::OutsideWorkspace => {
            t!("retrack.err_not_local_help")
        }
        RetrackFileError::InsideData => t!("retrack.err_inside_data_help"),
        RetrackFileError::NotRecorded => t!("retrack.err_not_recorded_help"),
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: the version named was never recorded, so there is nowhere to go back to.
#[derive(Grouped)]
pub struct ErrorRetrackNothing {
    /// Where the entry sits in the Layout.
    path: String,
}

impl Failure for ErrorRetrackNothing {
    fn name(&self) -> &'static str {
        "error_retrack_nothing"
    }

    fn reason(&self) -> String {
        t!("retrack.err_nothing", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackNothing);

#[renderer(buffer)]
pub fn render_error_retrack_nothing(error: ErrorRetrackNothing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_nothing_help").trim()));
    ec.exit_code = EC_ERR_RETRACK;
}

/// Error: `--until` names nothing the entry can be moved to.
#[derive(Grouped)]
pub struct ErrorRetrackLevel {
    /// What was given.
    until: String,
    /// What is wrong with it.
    kind: RetrackLevelError,
}

/// The ways `--until` names no version.
pub enum RetrackLevelError {
    /// It is not a level, a hash or a Vault's name.
    NotALevel,
    /// It is a level past an end of the chain the entry is the top of.
    OutOfChain {
        /// Where the entry sits in the Layout.
        path: String,
    },
    /// The hash names no version in the index.
    NotAVersion,
    /// The hash names a version that is not on the way back from where the entry is.
    NotAncestor {
        /// Where the entry sits in the Layout.
        path: String,
    },
}

impl Failure for ErrorRetrackLevel {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackLevelError::NotALevel => "error_retrack_not_a_level",
            RetrackLevelError::OutOfChain { .. } => "error_retrack_out_of_chain",
            RetrackLevelError::NotAVersion => "error_retrack_not_a_version",
            RetrackLevelError::NotAncestor { .. } => "error_retrack_not_ancestor",
        }
    }

    fn reason(&self) -> String {
        let said = match &self.kind {
            RetrackLevelError::NotALevel => t!("retrack.err_not_a_level", until = self.until),
            RetrackLevelError::OutOfChain { path } => {
                t!("retrack.err_out_of_chain", until = self.until, path = path)
            }
            RetrackLevelError::NotAVersion => {
                t!("retrack.err_not_a_version", until = self.until)
            }
            RetrackLevelError::NotAncestor { path } => {
                t!("retrack.err_not_ancestor", until = self.until, path = path)
            }
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackLevel);

#[renderer(buffer)]
pub fn render_error_retrack_level(error: ErrorRetrackLevel, ec: &mut ResExitCode) {
    let (help, code) = match error.kind {
        RetrackLevelError::NotALevel => {
            (t!("retrack.err_not_a_level_help"), EC_ERR_RETRACK_ARGUMENT)
        }
        RetrackLevelError::OutOfChain { .. } => {
            (t!("retrack.err_out_of_chain_help"), EC_ERR_RETRACK)
        }
        RetrackLevelError::NotAVersion => (t!("retrack.err_not_a_version_help"), EC_ERR_RETRACK),
        RetrackLevelError::NotAncestor { .. } => {
            (t!("retrack.err_not_ancestor_help"), EC_ERR_RETRACK)
        }
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
    ec.exit_code = code;
}

/// Error: the Vault's Layout records no version for the file.
#[derive(Grouped)]
pub struct ErrorRetrackUpstream {
    /// The Vault whose Layout records none.
    vault: String,
    /// Where the entry sits in this Layout.
    path: String,
}

impl Failure for ErrorRetrackUpstream {
    fn name(&self) -> &'static str {
        "error_retrack_upstream"
    }

    fn reason(&self) -> String {
        t!("retrack.err_upstream", vault = self.vault, path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackUpstream);

#[renderer(buffer)]
pub fn render_error_retrack_upstream(error: ErrorRetrackUpstream, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_upstream_help").trim()));
    ec.exit_code = EC_ERR_RETRACK;
}

/// Error: a store, an index or a Layout would not do what retracking needs.
#[derive(Grouped)]
pub struct ErrorRetrackFailed {
    /// Why it would not.
    cause: String,
}

impl Failure for ErrorRetrackFailed {
    fn name(&self) -> &'static str {
        "error_retrack_failed"
    }

    fn reason(&self) -> String {
        t!("retrack.err_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackFailed);

#[renderer(buffer)]
pub fn render_error_retrack_failed(error: ErrorRetrackFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_failed_help").trim()));
    ec.exit_code = EC_ERR_RETRACK;
}

#[cfg(test)]
mod tests {
    use super::{Until, until_of};
    use librorolala::storage::Key;

    /// The default and the two spellings of one step back are the same level.
    #[test]
    fn one_step_back_is_the_default_and_the_two_spellings_of_it() {
        assert!(matches!(until_of(None), Ok(Until::Steps(1))));
        assert!(matches!(until_of(Some("-1")), Ok(Until::Steps(1))));
        assert!(matches!(until_of(Some("~1")), Ok(Until::Steps(1))));
    }

    /// A step count is `~N`, a version number is digits, a hash is a hash, and anything else is
    /// the name of a Vault.
    #[test]
    fn each_shape_is_read_as_the_level_it_is() {
        assert!(matches!(until_of(Some("~3")), Ok(Until::Steps(3))));
        assert!(matches!(until_of(Some("~0")), Ok(Until::Steps(0))));
        assert!(matches!(until_of(Some("0")), Ok(Until::Number(0))));
        assert!(matches!(until_of(Some("12")), Ok(Until::Number(12))));
        assert!(matches!(until_of(Some("origin")), Ok(Until::Vault(name)) if name == "origin"));

        let hex = "0f".repeat(32);
        let Ok(Until::Hash(key)) = until_of(Some(&hex)) else {
            panic!("a hash is read as a hash");
        };
        assert_eq!(key, Key::new([0x0f; 32]));
    }

    /// What is neither a level nor a hash is refused where it cannot be a name at all: a sign
    /// that is not `-1`, a step count with no number, and a hash of the wrong width.
    #[test]
    fn a_level_that_is_not_one_is_refused() {
        assert!(until_of(Some("~")).is_err());
        assert!(until_of(Some("~x")).is_err());
        assert!(until_of(Some("-5")).is_err());
        assert!(until_of(Some("+1")).is_err());
        assert!(until_of(Some("")).is_err());
    }
}
