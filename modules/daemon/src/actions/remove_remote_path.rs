//! `remove-remote-path`: taking an entry out of the Vault's Layout.
//!
//! Deleting a file of the Vault's is the entry going rather than the entry moving: a move keeps what
//! is at the path and changes where it is, while this is the Vault's Layout no longer naming it. What
//! the entry's version names stays in the store, and the version and the objects under it stay in the
//! index, so what is lost is the file as a thing the Vault holds — its path, its holder and what it
//! says about itself.
//!
//! Who may remove is the account that holds it, or one that administrates the Vault, which is the
//! rule a version change already follows: taking an entry away is a change of who holds what, and an
//! account's own entries are its own to let go of.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{EntryRemove, remote_transact, remove_remote};

/// An Action that removes the entry a path names from the Vault's Layout.
///
/// # Input
///
/// The path, as the Vault's own Layout writes it. It belongs to the caller, so it is read on the
/// Workspace and crosses to the Vault, which is the side that has a Layout to remove it from.
///
/// # Output
///
/// What became of it, as JSON: the entry is gone, the path names nothing, the account may not remove
/// it, or the path is not one a remote Layout may name. It crosses as text for the reason an
/// ownership outcome does — see `request-ownership`.
pub struct ActionRemoveRemotePath;

impl Action for ActionRemoveRemotePath {
    /// Removing a path is id 19: the first of the writes a file's deletion makes.
    const ID: u32 = 19;

    /// The path to remove.
    type Input = String;
    type Output = String;

    /// Asks the Vault to remove the path, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if a path or an outcome
    /// will not cross, [`ActionError::Store`] if the Vault's Layout could not be written,
    /// [`ActionError::Json`] if the outcome will not encode, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcome: EntryRemove = remote_transact(&mut ctx, input, |layout, path, me, admin| {
            remove_remote(layout, &path, me, admin)
        })
        .await?;

        serde_json::to_string(&outcome).map_err(|error| ActionError::Json(error.into()))
    }
}
