//! `set-remote-version`: moving the version the Vault's Layout holds for a `Uuid` on.
//!
//! A version goes up only from the side that holds the entry, so which account is asking decides
//! whether it is written at all — and an administrator of the Vault may write one it does not
//! hold. What a version change touches is the version and nothing else: the path a reader sees and
//! the holder stay as they were.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{remote_write, set_remote};

/// An Action that sets the version an entry of the Vault's Layout is at.
///
/// # Input
///
/// The `Uuid` to set, and the version to set it at. Both belong to the caller and cross to the
/// Vault, which is the side that has a Layout to write.
///
/// # Output
///
/// What became of it, as JSON: the version is set, the Vault holds no such entry, the account may
/// not write it, or something could not be read. It crosses as text for the reason an ownership
/// outcome does — see `request-ownership`.
pub struct ActionSetRemoteVersion;

impl Action for ActionSetRemoteVersion {
    /// Setting a version is id 15, the second of the two writes a sync makes.
    const ID: u32 = 15;

    /// The `Uuid` and the version.
    ///
    /// - uuid: String
    /// - version: String
    type Input = (String, String);
    type Output = String;

    /// Asks the Vault to set the version, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if an input or an
    /// outcome will not cross, [`ActionError::Store`] if the Vault's Layout could not be written,
    /// [`ActionError::Json`] if the outcome will not encode, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcome = remote_write(&mut ctx, input, |layout, (uuid, version), me, admin| {
            set_remote(layout, &uuid, &version, me, admin)
        })
        .await?;

        serde_json::to_string(&outcome).map_err(|error| ActionError::Json(error.into()))
    }
}
