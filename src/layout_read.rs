//! The Layout a place holds, as the C ABI reads it.
//!
//! A Layout is the record a Workspace or a Vault works from: which path names which `Uuid`, and what
//! each entry holds. The reading is already the layout module's; what is added here is the resolving
//! of a *name* to a Layout, which is a Workspace's or a Vault's to answer rather than the record's.
//!
//! A name is one of four things. Nothing names the Layout being worked in, `main` names a
//! Workspace's, `truth` names a Vault's, and `NAME@VAULT` names the read-only copy a fetch left of
//! a Vault's Layout. The last is why this lives in the crate that ties the modules together: only a
//! Workspace knows where it keeps its fetched copies, and only it can say one was never fetched.
//!
//! What crosses is the JSON `rola layout ls` and `rola layout entries` already write, so a caller
//! that read those commands reads the same fields here.

use std::fmt;
use std::path::{Component, Path};

use rorolala_layout::{Layout, LayoutError};
use rorolala_storage::Key;
use rorolala_utils_constants::{DEFAULT_LAYOUT_NAME, LAYOUT_TRACK_FILE, VAULT_LAYOUT_NAME};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;
use serde::Serialize;

/// The marker a Vault puts in front of a path it has deprecated.
///
/// It is the same marker `rola layout entries` reads: a file moved under it is one the Vault has
/// taken out of the work rather than lost, and the reading says so rather than leaving a caller to
/// compare prefixes itself.
const REMOVED_PREFIX: &str = "@/removed/";

/// What reading a Layout failed with.
///
/// # FFI
///
/// The tag says which case it is. `Read` carries the reason the Layout could not be read.
#[lazyffi(export = RolaLayoutError)]
#[derive(Debug)]
pub enum LayoutReadError {
    /// No Workspace holds the directory, and it is not inside a Vault either.
    NoWorkspace,
    /// The Workspace is not working in a Layout.
    NoLayout,
    /// The copy of a Vault's Layout a name asked for was never fetched here.
    NotCached,
    /// The name is not one a place has a Layout under.
    Missing,
    /// The Layout is there, but it could not be read.
    Read {
        /// Why it could not.
        reason: String,
    },
}

impl fmt::Display for LayoutReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWorkspace => formatter.write_str("no Workspace or Vault holds the directory"),
            Self::NoLayout => formatter.write_str("the Workspace works in no Layout"),
            Self::NotCached => formatter.write_str("the Layout's copy was never fetched here"),
            Self::Missing => formatter.write_str("no Layout is known under that name"),
            Self::Read { reason } => write!(formatter, "the Layout could not be read: {reason}"),
        }
    }
}

impl std::error::Error for LayoutReadError {}

/// The Layouts a place holds, as the C ABI writes them.
#[derive(Debug, Serialize)]
struct LayoutsAnswer {
    /// The Layouts, which is the one the place works in.
    layouts: Vec<LayoutItem>,
}

/// One Layout, as `rola layout ls` shows it.
#[derive(Debug, Serialize)]
struct LayoutItem {
    /// The label it is listed under.
    name: &'static str,
    /// Whether it is the one the run is working in.
    is_current: bool,
    /// The Vault upstream it tracks, empty when it tracks none.
    tracking: String,
}

/// Which Layouts the place holding `directory` has, as `rola layout ls --json` writes them.
///
/// A Workspace holds one Layout and a Vault keeps one, so the answer is one item: the label it goes
/// by, that it is the one being worked in, and the Vault it tracks. A directory no Workspace or
/// Vault holds is answered with the JSON `null`.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"layouts":[{"name","is_current",
/// "tracking"}]}`. `name` is `main` for a Workspace and `truth` for a Vault, `tracking` is empty
/// when it tracks none, and `null` is answered when neither holds the directory or a Workspace
/// holds no Layout.
#[must_use]
#[lazyffi(export = rola_layouts_json)]
pub fn layouts_json(directory: &Path) -> String {
    place(directory).map_or_else(
        || "null".to_owned(),
        |(layout, name)| {
            let answer = LayoutsAnswer {
                layouts: vec![LayoutItem {
                    name,
                    is_current: true,
                    tracking: track_of(&layout),
                }],
            };

            serde_json::to_string(&answer).unwrap_or_else(|_| "null".to_owned())
        },
    )
}

/// The Layout the place holding `directory` works in, with the label it is listed under.
fn place(directory: &Path) -> Option<(Layout, &'static str)> {
    if let Some(workspace) = Workspace::locate(directory) {
        let layout = workspace.layout_readonly().ok().flatten()?;

        return Some((layout, DEFAULT_LAYOUT_NAME));
    }

    let vault = Vault::locate(directory)?;
    let layout = vault.layout_if_there().ok().flatten()?;

    Some((layout, VAULT_LAYOUT_NAME))
}

/// The Vault a Layout tracks, as read from the file beside it, or an empty string when it tracks
/// none.
fn track_of(layout: &Layout) -> String {
    std::fs::read_to_string(layout.dir().join(LAYOUT_TRACK_FILE))
        .map_or_else(|_| String::new(), |track| track.trim().to_owned())
}

/// What a Layout's entries hold, as the C ABI writes them.
#[derive(Debug, Serialize)]
struct EntriesAnswer {
    /// Each entry, in path order.
    entries: Vec<EntryItem>,
}

/// One entry, as `rola layout entries` shows it.
#[derive(Debug, Serialize)]
struct EntryItem {
    /// The path it is at, `.` when it is at none.
    path: String,
    /// The `Uuid` it is known by.
    uuid: String,
    /// The version it is at, as hex.
    version: String,
    /// The account that holds it, empty when none does.
    owner: String,
    /// What it says about itself.
    description: String,
    /// The cut it prefers, empty when none.
    preference: String,
    /// Whether the Vault has deprecated it, by moving it under `@/removed/`.
    deprecated: bool,
}

/// What a Layout's entries hold, as `rola layout entries --json` writes it.
///
/// `layout` names which Layout to read. An empty string or `main` reads the one a Workspace is
/// working in, `truth` reads a Vault's own, `NAME@VAULT` reads the copy a fetch left in the
/// Workspace, and a name that is none of those is [`LayoutReadError::Missing`].
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"entries":[{"path","uuid",
/// "version","owner","description","preference","deprecated"}]}`, in path order and then `Uuid`
/// order. `path` is
/// `.` for the root entry, `version` is the hex the version is stored under, `owner` is empty when
/// no account holds it, `preference` is empty when the store may choose, and `deprecated` says
/// whether the Vault moved it under `@/removed/`. A
/// failure is a `RolaLayoutError *` the caller releases with `free_rola_layout_error`.
///
/// # Errors
///
/// Returns [`LayoutReadError::NoWorkspace`] when neither a Workspace nor a Vault holds `directory`,
/// [`LayoutReadError::NoLayout`] when the Workspace works in no Layout,
/// [`LayoutReadError::NotCached`] when `NAME@VAULT` names a copy that was never fetched,
/// [`LayoutReadError::Missing`] when the name is not one the place has a Layout under, and
/// [`LayoutReadError::Read`] when the Layout or the answer could not be read.
#[lazyffi(export = rola_layout_entries_json)]
pub fn layout_entries_json(directory: &Path, layout: &str) -> Result<String, LayoutReadError> {
    let layout = chosen(directory, layout.trim())?;

    let mut entries: Vec<EntryItem> = layout
        .entries()
        .into_iter()
        .map(|(entry, data)| {
            let path = layout.path_of(entry);
            let deprecated = path
                .as_ref()
                .is_some_and(|path| path.as_str().starts_with(REMOVED_PREFIX));

            EntryItem {
                // An entry at no path is answered as the root, `.`, which is what a path of the
                // root itself prints as: the two are one answer to a caller that has no entry.
                path: path.map_or_else(|| ".".to_owned(), |path| path.to_string()),
                uuid: entry.to_string(),
                version: Key::new(data.version()).hex(),
                owner: data.owner().unwrap_or_default().to_owned(),
                description: data.description().to_owned(),
                preference: data.preference().unwrap_or_default().to_owned(),
                deprecated,
            }
        })
        .collect();

    entries.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.uuid.cmp(&right.uuid))
    });

    serde_json::to_string(&EntriesAnswer { entries }).map_err(|error| LayoutReadError::Read {
        reason: error.to_string(),
    })
}

/// The Layout `name` resolves to in the place holding `directory`.
///
/// It is the one reading of a Layout's name: a Layout command reaches a copy through it, and so
/// does the content fetch that reads its words, see [`crate::write::fetch_with`]. A second reading
/// would be a second set of names to keep right.
pub(crate) fn chosen(directory: &Path, name: &str) -> Result<Layout, LayoutReadError> {
    if let Some((layout, vault)) = remote_spec(name) {
        let workspace = Workspace::locate(directory).ok_or(LayoutReadError::NoWorkspace)?;
        let Some(copy) = workspace.readonly_layout_copy(vault, layout) else {
            return Err(LayoutReadError::NotCached);
        };

        return match Layout::open_existing(&copy) {
            Ok(copy) => Ok(copy),
            Err(error) if error.is_not_found() => Err(LayoutReadError::NotCached),
            Err(error) => Err(read(&error)),
        };
    }

    if let Some(workspace) = Workspace::locate(directory) {
        if !matches!(name, "" | DEFAULT_LAYOUT_NAME) {
            return Err(LayoutReadError::Missing);
        }

        return workspace
            .layout_readonly()
            .map_err(|error| read(&error))?
            .ok_or(LayoutReadError::NoLayout);
    }

    if let Some(vault) = Vault::locate(directory) {
        if !matches!(name, "" | VAULT_LAYOUT_NAME) {
            return Err(LayoutReadError::Missing);
        }

        return vault
            .layout_if_there()
            .map_err(|error| read(&error))?
            .ok_or(LayoutReadError::NoLayout);
    }

    Err(LayoutReadError::NoWorkspace)
}

/// The Vault and the Layout a name written `NAME@VAULT` names, when it is written that way with two
/// names a Layout can have.
pub(crate) fn remote_spec(name: &str) -> Option<(&str, &str)> {
    let (layout, vault) = name.split_once('@')?;

    (is_plain_name(layout) && is_plain_name(vault)).then_some((layout, vault))
}

/// Whether `name` is one path component and nothing else.
///
/// It is the check `rola layout entries` makes: a name carrying a separator or a step would reach
/// outside the directory a Workspace keeps its copies in, which is not where a name of a Layout
/// goes.
fn is_plain_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// What a Layout that could not be read is reported as.
fn read(error: &LayoutError) -> LayoutReadError {
    LayoutReadError::Read {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::{LayoutPath, MutableData};
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{LayoutReadError, is_plain_name, layout_entries_json, layouts_json, remote_spec};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let name = format!(
            "rorolala-layout-read-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );

        for base in [Some(std::env::temp_dir()), program_dir()] {
            let Some(base) = base else {
                continue;
            };
            let dir = base.join(&name);
            let _ = fs::remove_dir_all(&dir);

            if fs::create_dir_all(&dir).is_ok() {
                return dir;
            }
        }

        panic!("no writable directory to make {name} in");
    }

    /// The directory this test's program sits in, or nothing when it cannot be found.
    fn program_dir() -> Option<PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
    }

    /// A Workspace working in a Layout that names one file and holds one entry at no path.
    fn recorded(label: &str) -> PathBuf {
        let dir = scratch(label);

        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();

        layout
            .create_entry(
                Uuid::from_u128(1),
                MutableData::new(Some("alice".to_owned()), [7; 32], "first".to_owned()),
            )
            .unwrap();
        layout
            .create_path(
                &LayoutPath::new("art/hero.psd").unwrap(),
                Uuid::from_u128(1),
            )
            .unwrap();
        layout
            .create_entry(
                Uuid::from_u128(2),
                MutableData::new(None, [8; 32], String::new()),
            )
            .unwrap();

        dir
    }

    #[test]
    fn a_place_with_no_layout_is_answered_with_null() {
        let dir = scratch("absent");

        assert_eq!(layouts_json(&dir), "null");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_lists_its_one_layout() {
        let dir = recorded("list");

        let answer: serde_json::Value = serde_json::from_str(&layouts_json(&dir)).unwrap();
        let layouts = answer["layouts"].as_array().unwrap();

        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0]["name"], "main");
        assert_eq!(layouts[0]["is_current"], true);
        assert_eq!(layouts[0]["tracking"], "");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_entry_carries_its_path_version_owner_and_description() {
        let dir = recorded("entries");

        let answer: serde_json::Value =
            serde_json::from_str(&layout_entries_json(&dir, "").unwrap()).unwrap();
        let entries = answer["entries"].as_array().unwrap();

        assert_eq!(entries.len(), 2);

        let held = entries
            .iter()
            .find(|entry| entry["uuid"] == Uuid::from_u128(1).to_string())
            .unwrap();
        assert_eq!(held["path"], "art/hero.psd");
        assert_eq!(held["owner"], "alice");
        assert_eq!(held["description"], "first");
        assert_eq!(held["deprecated"], false);
        assert_eq!(held["version"].as_str().unwrap().len(), 64);

        // An entry at no path is answered as the root rather than left out.
        let root = entries
            .iter()
            .find(|entry| entry["uuid"] == Uuid::from_u128(2).to_string())
            .unwrap();
        assert_eq!(root["path"], ".");
        assert_eq!(root["owner"], "");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_without_a_layout_is_no_layout() {
        let dir = scratch("no-layout");
        Workspace::create(&dir).unwrap();
        fs::remove_dir_all(dir.join(".rola/layout")).unwrap();

        assert!(matches!(
            layout_entries_json(&dir, ""),
            Err(LayoutReadError::NoLayout)
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_that_is_no_place_is_no_workspace() {
        let dir = scratch("nowhere");

        assert!(matches!(
            layout_entries_json(&dir, ""),
            Err(LayoutReadError::NoWorkspace)
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_that_was_never_fetched_is_not_cached() {
        let dir = recorded("not-cached");

        assert!(matches!(
            layout_entries_json(&dir, "truth@studio"),
            Err(LayoutReadError::NotCached)
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_name_a_place_does_not_have_is_missing() {
        let dir = recorded("missing");

        assert!(matches!(
            layout_entries_json(&dir, "other"),
            Err(LayoutReadError::Missing)
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_remote_name_is_two_plain_names() {
        assert_eq!(remote_spec("truth@studio"), Some(("truth", "studio")));
        assert_eq!(remote_spec("studio"), None);
        assert_eq!(remote_spec("@studio"), None);
        assert_eq!(remote_spec("truth@"), None);
        assert_eq!(remote_spec("../truth@studio"), None);
        assert!(is_plain_name("truth"));
        assert!(!is_plain_name(""));
        assert!(!is_plain_name("a/b"));
    }
}
