//! `create-remote-entry`: putting a `Uuid` only the Workspace holds into the Vault's Layout.
//!
//! A file enters a Layout by being tracked, and enters the Vault's Layout by this: the name it is
//! given is `@/new/<the path it was tracked under>@<short uuid>`, so what a reader sees first is
//! that it is new and where it came from. The holder is the account that asked, which the Vault
//! reads from the session rather than from anything sent.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{create_remote, remote_write};

/// An Action that creates an entry of the Vault's Layout.
///
/// # Input
///
/// The name to hold it under, the `Uuid` it is known by, and the version to hold it at. All three
/// belong to the caller and cross to the Vault, which is the side that has a Layout to write.
///
/// # Output
///
/// What became of it, as JSON: the entry is there, the `Uuid` already was, the path already names
/// something, or something could not be read. It crosses as text for the reason an ownership
/// outcome does — see `request-ownership`.
pub struct ActionCreateRemoteEntry;

impl Action for ActionCreateRemoteEntry {
    /// Creating an entry is id 14, the first of the two writes a sync makes.
    const ID: u32 = 14;

    /// The path, the `Uuid`, and the version.
    ///
    /// - path: String
    /// - uuid: String
    /// - version: String
    type Input = (String, String, String);
    type Output = String;

    /// Asks the Vault to create the entry, and answers with what it said.
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
        let outcome = remote_write(
            &mut ctx,
            input,
            |layout, (path, uuid, version), me, _admin| {
                create_remote(layout, &path, &uuid, &version, me)
            },
        )
        .await?;

        serde_json::to_string(&outcome).map_err(|error| ActionError::Json(error.into()))
    }
}
