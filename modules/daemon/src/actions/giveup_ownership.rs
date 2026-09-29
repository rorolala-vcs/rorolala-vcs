//! `giveup-ownership`: letting go of an entry of the Vault's Layout.
//!
//! The other half of [`request-ownership`](crate::ActionRequestOwnership): a Layout names who holds
//! each entry, and an account that is done with one asks the Vault to name nobody. Only the holder
//! can: an entry another account holds is refused, and one nobody holds is already what was asked
//! for. What the Workspace keeps a copy of is changed to match — the command that asked does that.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{giveup, ownership_change};

/// An Action that names nobody as the holder of the Vault's entry `Uuid`.
///
/// # Input
///
/// The `Uuid` of the entry, as text, read on the Workspace and crossed to the Vault — as
/// [`request-ownership`](crate::ActionRequestOwnership) reads it. Which account lets go is not
/// input: the session already says.
///
/// # Output
///
/// What became of it, as JSON: the entry is now held by nobody, the Vault holds no such entry, or
/// another account holds it and it was left alone. It crosses as text because an action's output is
/// what the C ABI hands back, and an outcome is not a type a C caller reads — [`Ownership`](super::layout_remote::Ownership)
/// is.
pub struct ActionGiveupOwnership;

impl Action for ActionGiveupOwnership {
    /// Letting go is id 12: the second half of the ownership exchange.
    const ID: u32 = 12;

    type Input = String;
    type Output = String;

    /// Asks the Vault to name nobody as the entry's holder, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, [`ActionError::Codec`] if the `Uuid` does not read or a payload will not cross,
    /// [`ActionError::Store`] if the Vault's Layout could not be written, [`ActionError::Json`] if
    /// the outcome will not encode, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcome = ownership_change(&mut ctx, input, giveup).await?;

        serde_json::to_string(&outcome).map_err(|error| ActionError::Json(error.into()))
    }
}
