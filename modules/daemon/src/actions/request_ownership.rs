//! `request-ownership`: taking an entry of the Vault's Layout as one's own.
//!
//! A Layout names who holds each entry, and a Vault is where that naming is shared: a Workspace
//! asks to be the holder of one `Uuid`, the Vault names it if nobody else holds it, and the answer
//! says which of those happened. What the Workspace keeps a copy of is changed to match — the
//! command that asked does that — so what a fetch reads next agrees with the Vault.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{ownership_change, take};

/// An Action that names the Vault's entry `Uuid` as held by the account that asks.
///
/// # Input
///
/// The `Uuid` of the entry, as text. It belongs to the caller, so it is read on the Workspace and
/// crosses to the Vault, which is the side that has a Layout to name it in. Which account asks is
/// not input: the session already says, and a name sent beside it would be a claim the Vault could
/// not hold the caller to.
///
/// # Output
///
/// What became of it, as JSON: the entry is now held by the caller, the Vault holds no such entry,
/// or another account holds it. It crosses as text because an action's output is what the C ABI
/// hands back, and an outcome is not a type a C caller reads — [`Ownership`](super::layout_remote::Ownership)
/// is.
pub struct ActionRequestOwnership;

impl Action for ActionRequestOwnership {
    /// Taking an entry is id 11: the first half of the ownership exchange.
    const ID: u32 = 11;

    type Input = String;
    type Output = String;

    /// Asks the Vault to name the caller as the entry's holder, and answers with what it said.
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
        let outcome = ownership_change(&mut ctx, input, take).await?;

        serde_json::to_string(&outcome).map_err(|error| ActionError::Json(error.into()))
    }
}
