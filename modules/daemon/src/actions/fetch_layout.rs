//! `fetch-layout`: the Layout a Vault holds, brought here whole.
//!
//! A Workspace works in Layouts of its own and keeps one it did not make: a copy of what a Vault
//! holds. It is fetched rather than synced, and nothing moves the other way — what a Workspace
//! finds in the copy is who holds what upstream, so the copy is read and not worked in.
//!
//! What crosses is the Vault's Layout directory as it stands, every file of it, and the Workspace
//! writes that under its cache named by the Vault. The whole directory is one blob: a Layout is a
//! fixed set of small files beside snapshots, and carrying them one at a time would say the same
//! thing in more exchanges.

use std::fs;
use std::path::{Component, Path};

use rorolala_layout::Layout;
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_utils_constants::{VAULT_LAYOUT_NAME, WORKSPACE_READONLY_LAYOUTS_DIR};
use rorolala_utils_location::Locate as _;

use super::layout_remote::local_layout;
use super::sync_storage::{Side, carry_blob};

/// An Action that brings the Vault's Layout to the Workspace's read-only cache.
///
/// # Input
///
/// The name the Workspace keeps the fetched Layout under — the name it bound the Vault by. It
/// belongs to the Workspace and is read there; the Vault is handed it and has no use for it, since
/// which Vault is reached is the address's to say. The Vault keeps one Layout, so what lands under
/// that name is the one known as [`VAULT_LAYOUT_NAME`].
///
/// # Output
///
/// Nothing: what a fetch comes to is a directory the Workspace now holds, and a caller that wants
/// to say where it landed says it in its own words.
pub struct ActionFetchLayout;

impl Action for ActionFetchLayout {
    /// The fetch is id 10: the first thing asked of a Vault that is not a store or an index.
    const ID: u32 = 10;

    type Input = String;
    type Output = ();

    /// Asks the Vault for its Layout, and writes what it answers into the Workspace's cache.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, [`ActionError::Store`] if the Vault's Layout could not be read or the cache written,
    /// and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // Which directory the copy lands in is the Workspace's own business, so the name is read
        // here and never looked at on the Vault.
        let cache = input.into_inner().unwrap_or_default();

        // What the Vault holds is read there: it is the side with a Layout of its own, and the
        // Workspace side has none to read.
        let held = if ctx.is_vault() {
            Some(read_layout(&local_layout(&ctx)?)?)
        } else {
            None
        };
        let mine = held
            .as_ref()
            .map(rorolala_protocol::Encodable::encode)
            .transpose()
            .map_err(codec_failed)?;

        // What the Vault holds crosses back, and the Workspace is where it is written: the same
        // call reads both ways, so the side that sent it keeps its own.
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        if ctx.is_workspace() {
            let files = Vec::<(String, Vec<u8>)>::decode(bytes).map_err(codec_failed)?;
            write_cache(&ctx, &cache, &files)?;
        }

        Ok(())
    }
}

/// Every file the Layout keeps, by the name it is kept under, in name order.
///
/// The directory is read as it stands rather than by the names a Layout is known to keep: what a
/// fetch is for is the Layout as the Vault has it, and a file this build does not know is still
/// part of what a later build may read. Directories are passed over, since a Layout keeps none.
///
/// # Errors
///
/// Returns [`ActionError::Io`] if the directory or a file in it cannot be read.
fn read_layout(layout: &Layout) -> Result<Vec<(String, Vec<u8>)>, ActionError> {
    let mut files = Vec::new();

    for entry in fs::read_dir(layout.dir())? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };

        files.push((name, fs::read(entry.path())?));
    }

    files.sort();

    Ok(files)
}

/// Writes the fetched files under the Workspace's cache, replacing whatever was there.
///
/// The old copy is taken away rather than written over, so a file the Vault no longer keeps does
/// not survive in the copy: what is read afterwards is the Vault's Layout and not that Layout with
/// older files beside it.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if this side has no Workspace, and [`ActionError::Io`] if
/// the copy cannot be made.
fn write_cache(
    ctx: &ActionContext<'_>,
    name: &str,
    files: &[(String, Vec<u8>)],
) -> Result<(), ActionError> {
    let workspace = ctx
        .current_workspace()
        .into_inner()
        .ok_or(ActionError::MissingValue)?;

    if !is_plain_name(name) {
        return Err(ActionError::Store(format!(
            "`{name}` is not a name a fetched Layout can be kept under"
        )));
    }

    let dir = workspace
        .get_root()
        .join(WORKSPACE_READONLY_LAYOUTS_DIR)
        .join(name)
        .join(VAULT_LAYOUT_NAME);

    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::create_dir_all(&dir)?;

    for (file, bytes) in files {
        if !is_plain_name(file) {
            return Err(ActionError::Store(format!(
                "the Vault sent `{file}`, which is not a file name"
            )));
        }

        fs::write(dir.join(file), bytes)?;
    }

    Ok(())
}

/// Whether `name` is one path component and nothing else.
///
/// What a Vault sends is written under the cache by the name it was sent under, so a name that
/// names somewhere else — a path, a climbing name, a drive — would write outside the one directory
/// the fetch is for. A name this refuses is a Layout directory that is not one.
fn is_plain_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(rorolala_errors::BincodeError::new(error.to_string()))
}
