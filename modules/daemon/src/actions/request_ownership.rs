//! `request-ownership`: taking entries of the Vault's Layout as one's own.
//!
//! A Layout names who holds each entry, and a Vault is where that naming is shared: a Workspace
//! asks to be the holder of some `Uuid`s, the Vault names each of them if nobody else holds it, and
//! the answer says which of those happened for each. What the Workspace keeps a copy of is changed
//! to match — the command that asked does that — so what a fetch reads next agrees with the Vault.
//!
//! The `Uuid`s are taken in one run rather than one connection apiece: the whole batch crosses over
//! one session, one entry at a time, which is what a run that names a directory wants.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{ownership_batch, take};

/// An Action that names the Vault's entries as held by the account that asks.
///
/// # Input
///
/// What to take, as JSON: an array of the words the bar is drawn under and the `Uuid`s of the entries,
/// as text. The `Uuid`s belong to the caller, so they are read on the Workspace and cross to the Vault,
/// which is the side that has a Layout to name them in; the words cross with them because the caller is
/// the side that has them — see `ownership_batch`. A list is not a thing the C ABI can hand over, so the
/// pair crosses as one JSON string, the same way a sync's batch of writes does. Which account asks is
/// not input: the session already says, and a name sent beside it would be a claim the Vault could not
/// hold the caller to.
///
/// # Output
///
/// What became of each, as JSON: an array of outcomes, one per `Uuid`, in the order they were asked
/// for. Each is what one request answers on its own — the entry is now held by the caller, the Vault
/// holds no such entry, or another account holds it. It crosses as text because an action's output is
/// what the C ABI hands back, and an outcome is not a type a C caller reads — [`Ownership`](super::layout_remote::Ownership)
/// is.
pub struct ActionRequestOwnership;

impl Action for ActionRequestOwnership {
    /// Taking entries is id 11: the first half of the ownership exchange.
    const ID: u32 = 11;

    type Input = String;
    type Output = String;

    /// Asks the Vault to name the caller as the holder of each entry, and answers with what it said.
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
        let outcomes = ownership_batch(&mut ctx, input, take).await?;

        serde_json::to_string(&outcomes).map_err(|error| ActionError::Json(error.into()))
    }
}
