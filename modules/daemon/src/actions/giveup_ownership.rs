//! `giveup-ownership`: letting go of entries of the Vault's Layout.
//!
//! The other half of [`request-ownership`](crate::ActionRequestOwnership): a Layout names who holds
//! each entry, and an account that is done with some asks the Vault to name nobody for each of them.
//! Only the holder can: an entry another account holds is refused, and one nobody holds is already
//! what was asked for. What the Workspace keeps a copy of is changed to match — the command that
//! asked does that.
//!
//! As the other half is, the `Uuid`s are let go of in one run rather than one connection apiece.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{giveup, ownership_batch};

/// An Action that names nobody as the holder of each of the Vault's entries.
///
/// # Input
///
/// What to let go of, as JSON: an array of the words the bar is drawn under and the `Uuid`s of the
/// entries, as text. The `Uuid`s belong to the caller, so they are read on the Workspace and cross to
/// the Vault, which is the side that has a Layout to name them in; the words cross with them because
/// the caller is the side that has them — see `ownership_batch`. A list is not a thing the C ABI can
/// hand over, so the pair crosses as one JSON string. Which account lets go is not input: the session
/// already says.
///
/// # Output
///
/// What became of each, as JSON: an array of outcomes, one per `Uuid`, in the order they were asked
/// for. Each is what one let-go answers on its own — the entry is now held by nobody, the Vault holds
/// no such entry, or another account holds it. It crosses as text because an action's output is what
/// the C ABI hands back, and an outcome is not a type a C caller reads — [`Ownership`](super::layout_remote::Ownership)
/// is.
pub struct ActionGiveupOwnership;

impl Action for ActionGiveupOwnership {
    /// Letting go is id 12: the second half of the ownership exchange.
    const ID: u32 = 12;

    type Input = String;
    type Output = String;

    /// Asks the Vault to name nobody as the holder of each entry, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if a `Uuid` does not
    /// read or a payload will not cross, [`ActionError::Store`] if the Vault's Layout could not be
    /// written, [`ActionError::Json`] if the outcomes will not encode, and whatever the exchange
    /// fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcomes = ownership_batch(&mut ctx, input, giveup).await?;

        serde_json::to_string(&outcomes).map_err(|error| ActionError::Json(error.into()))
    }
}
