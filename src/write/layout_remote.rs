//! Moving a path, and removing an entry, in the copy of a Vault's Layout a fetch left here.
//!
//! A copy is read, never worked in. What changes it is the Vault agreeing to the change, which is an
//! action, and the copy following what the action answered, in that order: a copy that says a path
//! moved is one the Vault has moved. The command line's own path and removal commands make the same
//! two steps, and this is where they are for a caller that is not a command.

use std::path::Path;
use std::str::FromStr as _;

use rorolala_layout::{LayoutError, LayoutPath};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::auth::Account;
use crate::daemon::{EntryRemove, PathMove, action_move_remote_path, action_remove_remote_path};
use crate::remote::Reached;
use crate::write::WriteError;
use crate::write::layout::{cached_copy, path_of};
use crate::write::request::{Common, Target};

/// Moves a path in the fetched copy of a Vault's own Layout.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `truth@VAULT`, the name a fetch left the copy under. The
/// Vault is asked first, and the copy follows only what it answered; a failure leaves the copy as
/// it was.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, [`WriteError::NoAccount`]
/// when the work acts as no account, [`WriteError::InvalidPath`] when a path or the Layout name is
/// not one this accepts, [`WriteError::Precondition`] when the Vault is not one the Workspace bound
/// or the path is taken, [`WriteError::NotFound`] when the path names nothing,
/// [`WriteError::Action`] when the Vault cannot be asked, and [`WriteError::Layout`] when the copy
/// cannot be read or written.
#[lazyffi(export = rola_write_layout_move_fetched)]
pub fn move_path_fetched(
    directory: &Path,
    layout: &str,
    from: &str,
    to: &str,
) -> Result<(), WriteError> {
    let workspace = workspace_at(directory)?;
    let vault = fetched(layout)?;
    let from = path_of(from)?;
    let to = path_of(to)?;
    let reached = reached(&workspace, vault)?;
    let account = account(&workspace)?;

    let said = action_move_remote_path(
        &workspace,
        &account,
        reached.address.to_string(),
        from.as_str().to_owned(),
        to.as_str().to_owned(),
    )
    .map_err(|error| WriteError::Action {
        reason: error.to_string(),
    })?;

    let outcome: PathMove = answer(&said)?;

    follow_move(&workspace, &reached.name, &from, &to, &outcome)
}

/// Removes the entry a fetched copy names, path and all.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `truth@VAULT`, the name a fetch left the copy under, and
/// `uuid` is the entry as the copy names it. The Vault is asked first, and the copy follows only
/// what it answered; a failure leaves the copy as it was.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, [`WriteError::NoAccount`]
/// when the work acts as no account, [`WriteError::InvalidPath`] when the `Uuid` or the Layout name
/// is not one this accepts, [`WriteError::Precondition`] when the Vault is not one the Workspace
/// bound, [`WriteError::NotCached`] when the Vault's Layout was never fetched here,
/// [`WriteError::NotFound`] when the copy names no such entry, [`WriteError::Ownership`] when
/// another account holds it, [`WriteError::Action`] when the Vault cannot be asked, and
/// [`WriteError::Layout`] when the copy cannot be read or written.
#[lazyffi(export = rola_write_layout_remove_fetched_entry)]
pub fn remove_fetched_entry(directory: &Path, layout: &str, uuid: &str) -> Result<(), WriteError> {
    let workspace = workspace_at(directory)?;
    let vault = fetched(layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    let reached = reached(&workspace, vault)?;
    let account = account(&workspace)?;

    // The Vault's removal names a path, and the copy is what says which one the entry is at. A copy
    // that is not here is not a copy a removal can name a path in, so it is refused before the
    // Vault is asked, the same way the command line reads the copy before it writes one.
    let copy = cached_copy(&workspace, &reached.name)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })?
        .ok_or(WriteError::NotCached)?;
    let path = copy.path_of(id).ok_or_else(|| WriteError::NotFound {
        what: format!("Layout entry {id} in the fetched copy"),
    })?;

    let said = action_remove_remote_path(
        &workspace,
        &account,
        reached.address.to_string(),
        path.as_str().to_owned(),
    )
    .map_err(|error| WriteError::Action {
        reason: error.to_string(),
    })?;

    let outcome: EntryRemove = answer(&said)?;

    follow_remove(&workspace, &reached.name, &path, &outcome)
}

/// The Workspace `directory` sits in.
fn workspace_at(directory: &Path) -> Result<Workspace, WriteError> {
    Workspace::locate(directory).ok_or(WriteError::NoWorkspace)
}

/// The Vault a fetched copy is named under.
///
/// A Vault keeps one Layout, so a copy is only ever the one known as [`VAULT_LAYOUT_NAME`]; a name
/// that says otherwise is not a copy this can write, whichever Vault it names.
fn fetched(layout: &str) -> Result<&str, WriteError> {
    let (named, vault) = layout.split_once('@').ok_or_else(|| not_fetched(layout))?;

    if named != VAULT_LAYOUT_NAME || vault.is_empty() {
        return Err(not_fetched(layout));
    }

    Ok(vault)
}

/// The refusal a name that is not a fetched copy is answered with.
fn not_fetched(layout: &str) -> WriteError {
    WriteError::InvalidPath {
        path: layout.to_owned(),
        reason: format!("a fetched copy is named `{VAULT_LAYOUT_NAME}@VAULT`"),
    }
}

/// The Vault a fetched copy names, and where it answers.
///
/// Resolving the name and the address together is what the request layer already does for every
/// other write, so a fetched copy reaches the Vault by the same rules as the rest.
fn reached(workspace: &Workspace, vault: &str) -> Result<Reached, WriteError> {
    Common {
        target: Target::Named {
            name: vault.to_owned(),
        },
        ..Common::default()
    }
    .vault(workspace)
}

/// The account the work acts as, from Rorolala's own file for the user.
///
/// It is the answer `rola` reads, so a write made in the window acts as the account the command
/// line would act as, rather than as a second choice made here.
fn account(workspace: &Workspace) -> Result<Account, WriteError> {
    let name = crate::auth::current_account().ok_or(WriteError::NoAccount)?;

    crate::keys::account_named(&name, Some(workspace), None).ok_or(WriteError::NoAccount)
}

/// The outcome a Vault answered with, read as the shape its action writes.
fn answer<T: DeserializeOwned>(text: &str) -> Result<T, WriteError> {
    serde_json::from_str(text).map_err(|error| WriteError::Action {
        reason: format!("the Vault's answer did not read: {error}"),
    })
}

/// Writes a move the Vault agreed to into the copy the Workspace keeps.
///
/// A copy that is not there, or one that does not name the path, is left as it is: the Vault is the
/// truth, and a fetch is what brings a copy up to date.
fn follow_move(
    workspace: &Workspace,
    vault: &str,
    from: &LayoutPath,
    to: &LayoutPath,
    outcome: &PathMove,
) -> Result<(), WriteError> {
    match outcome {
        PathMove::Moved => {
            set_cached_path(workspace, vault, from, to).map_err(|error| WriteError::Layout {
                reason: error.to_string(),
            })
        }
        PathMove::Missing => Err(WriteError::NotFound {
            what: format!("Layout path `{}`", from.as_str()),
        }),
        PathMove::Taken => Err(WriteError::Precondition {
            what: format!("Layout path `{}`", to.as_str()),
            reason: "the path already names an entry".to_owned(),
        }),
        PathMove::Malformed => Err(WriteError::InvalidPath {
            path: format!("{} -> {}", from.as_str(), to.as_str()),
            reason: "a remote Layout does not accept the path".to_owned(),
        }),
    }
}

/// Writes a removal the Vault agreed to into the copy the Workspace keeps.
///
/// It follows a removal the Vault accepted, so that a copy read afterwards says what the Vault says,
/// the same reason [`follow_move`] follows a move.
fn follow_remove(
    workspace: &Workspace,
    vault: &str,
    path: &LayoutPath,
    outcome: &EntryRemove,
) -> Result<(), WriteError> {
    match outcome {
        EntryRemove::Removed => {
            set_cached_removed(workspace, vault, path).map_err(|error| WriteError::Layout {
                reason: error.to_string(),
            })
        }
        EntryRemove::Missing => Err(WriteError::NotFound {
            what: format!("Layout path `{}`", path.as_str()),
        }),
        EntryRemove::Refused { owner } => Err(WriteError::Ownership {
            path: path.as_str().to_owned(),
            what: "held by another account".to_owned(),
            reason: owner.as_ref().map_or_else(
                || "no account holds it, so only an administrator may remove it".to_owned(),
                |owner| format!("`{owner}` holds it"),
            ),
        }),
        EntryRemove::Malformed => Err(WriteError::InvalidPath {
            path: path.as_str().to_owned(),
            reason: "a remote Layout does not accept the path".to_owned(),
        }),
    }
}

/// Moves one path of the copy the Workspace keeps, when the copy names it.
fn set_cached_path(
    workspace: &Workspace,
    vault: &str,
    from: &LayoutPath,
    to: &LayoutPath,
) -> Result<(), LayoutError> {
    let Some(copy) = cached_copy(workspace, vault)? else {
        return Ok(());
    };

    if copy.id_of(from).is_none() {
        return Ok(());
    }

    copy.move_path(from, to)
}

/// Takes the entry a path names out of the copy the Workspace keeps, when the copy names it.
fn set_cached_removed(
    workspace: &Workspace,
    vault: &str,
    path: &LayoutPath,
) -> Result<(), LayoutError> {
    let Some(copy) = cached_copy(workspace, vault)? else {
        return Ok(());
    };
    let Some(id) = copy.id_of(path) else {
        return Ok(());
    };

    copy.remove_entry(id)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{
        EntryRemove, PathMove, fetched, follow_move, follow_remove, move_path_fetched,
        remove_fetched_entry,
    };
    use crate::write::WriteError;

    /// A Workspace with one fetched copy of `origin`'s truth Layout, holding one entry at `a.psd`.
    fn fetched_workspace(label: &str) -> (PathBuf, Workspace) {
        let dir = std::env::temp_dir().join(format!(
            "rola-write-layout-remote-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Workspace::create(&dir).expect("a Workspace");
        let workspace = Workspace::locate(&dir).expect("the Workspace is there");
        let copy = Layout::open(workspace.readonly_layouts_dir("origin", "truth"))
            .expect("a fetched copy");
        copy.create_entry(
            Uuid::from_u128(1),
            MutableData::new(None, [1; 32], "first".to_owned()),
        )
        .expect("an entry");
        copy.create_path(
            &LayoutPath::new("a.psd").expect("a path"),
            Uuid::from_u128(1),
        )
        .expect("a path bound");

        (dir, workspace)
    }

    /// The copy a fetched Workspace keeps, read back.
    fn copy_of(workspace: &Workspace) -> Layout {
        Layout::open_existing(workspace.readonly_layouts_dir("origin", "truth"))
            .expect("the copy reads")
    }

    /// What the Vault answered is what the copy says, and a refusal changes nothing.
    #[test]
    fn a_fetched_move_and_removal_follow_what_the_vault_answered() {
        let (dir, workspace) = fetched_workspace("round-trip");
        let from = LayoutPath::new("a.psd").expect("a path");
        let to = LayoutPath::new("b.psd").expect("a path");
        let id = Uuid::from_u128(1);

        follow_move(&workspace, "origin", &from, &to, &PathMove::Moved).expect("a followed move");

        let moved = copy_of(&workspace);
        assert_eq!(moved.id_of(&to), Some(id));
        assert_eq!(moved.id_of(&from), None);

        // A refusal leaves the copy naming what it named: the Vault is the truth, and a copy that
        // followed an answer the Vault did not give would say a change happened that did not.
        assert!(matches!(
            follow_move(&workspace, "origin", &to, &from, &PathMove::Missing),
            Err(WriteError::NotFound { .. })
        ));
        assert_eq!(copy_of(&workspace).id_of(&to), Some(id));

        follow_remove(&workspace, "origin", &to, &EntryRemove::Removed)
            .expect("a followed removal");

        let removed = copy_of(&workspace);
        assert_eq!(removed.id_of(&to), None);
        assert!(removed.entry(id).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    /// A name that is not a fetched copy is refused as an argument, before anything is read.
    #[test]
    fn a_layout_that_is_not_a_fetched_copy_is_refused() {
        assert_eq!(fetched("truth@origin").expect("a fetched copy"), "origin");
        for name in ["main", "", "truth@", "@origin", "file@origin"] {
            assert!(
                matches!(fetched(name), Err(WriteError::InvalidPath { .. })),
                "{name}"
            );
        }
    }

    /// The exports refuse an argument that does not read, whether or not an account is bound.
    #[test]
    fn the_fetched_exports_refuse_an_argument_that_does_not_read() {
        let (dir, _workspace) = fetched_workspace("refused-name");

        assert!(matches!(
            move_path_fetched(&dir, "main", "a.psd", "b.psd"),
            Err(WriteError::InvalidPath { .. })
        ));
        assert!(matches!(
            remove_fetched_entry(&dir, "truth@origin", "not-a-uuid"),
            Err(WriteError::InvalidPath { .. })
        ));
        assert!(matches!(
            remove_fetched_entry(&dir, "main", "00000000-0000-0000-0000-000000000001"),
            Err(WriteError::InvalidPath { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    /// A directory no Workspace holds is answered before anything else.
    #[test]
    fn a_directory_no_workspace_holds_is_refused() {
        let dir = std::env::temp_dir().join(format!(
            "rola-write-layout-remote-{}-nowhere",
            std::process::id()
        ));

        assert!(matches!(
            move_path_fetched(&dir, "truth@origin", "a.psd", "b.psd"),
            Err(WriteError::NoWorkspace)
        ));
        assert!(matches!(
            remove_fetched_entry(&dir, "truth@origin", "00000000-0000-0000-0000-000000000001"),
            Err(WriteError::NoWorkspace)
        ));
    }
}
