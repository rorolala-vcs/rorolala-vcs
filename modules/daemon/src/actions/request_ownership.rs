//! `request-ownership`: taking entries of the Vault's Layout as one's own.
//!
//! A Layout names who holds each entry, and a Vault is where that naming is shared: a Workspace
//! asks to be the holder of some `Uuid`s, the Vault names each of them if nobody else holds it, and
//! the answer says which of those happened for each. What the Workspace keeps a copy of is changed
//! to match, the command that asked does that, so what a fetch reads next agrees with the Vault.
//!
//! The `Uuid`s are taken in one run rather than one connection apiece: the whole batch crosses over
//! one session, one entry at a time, which is what a run that names a directory wants.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use rorolala_utils_lazyffi::{StrSpan, lazyffi};

use super::layout_remote::{OwnershipChange, Ownerships, ownership_batch, take};

/// An Action that names the Vault's entries as held by the account that asks.
///
/// # Input
///
/// What to take: the words the bar is drawn under and the `Uuid`s of the entries, as text. The
/// `Uuid`s belong to the caller, so they are read on the Workspace and cross to the Vault, which is
/// the side that has a Layout to name them in; the words cross with them because the caller is the
/// side that has them, see `ownership_batch`. Which account asks is not input: the session already
/// says, and a name sent beside it would be a claim the Vault could not hold the caller to.
///
/// # Output
///
/// What became of each, one outcome per `Uuid`, in the order they were asked for. Each is what one
/// request answers on its own: the entry is now held by the caller, the Vault holds no such entry,
/// or another account holds it. [`Ownerships`](super::layout_remote::Ownerships) is what a C caller
/// reads.
pub struct ActionRequestOwnership;

/// What to take, and under what words the bar is drawn.
///
/// # FFI
///
/// A `RolaDaemonRequestOwnershipRequest` the caller fills. `name` is borrowed for the call and
/// released by the caller. `uuids` is a `RolaStrSpan` of the `Uuid`s to take, each read as text and
/// borrowed for the call.
#[lazyffi(export = RolaDaemonRequestOwnershipRequest, input)]
#[derive(Debug, Clone, Default)]
pub struct RequestOwnershipRequest {
    /// The words the bar is drawn under, or empty for none.
    pub name: String,
    /// The `Uuid`s of the entries to take, in the order they are answered.
    pub uuids: StrSpan,
}

impl RequestOwnershipRequest {
    /// The request a run inside Rust fills, taking the `Uuid`s as strings.
    #[must_use]
    pub fn new(name: String, uuids: Vec<String>) -> Self {
        Self {
            name,
            uuids: StrSpan::from(uuids),
        }
    }
}

impl From<RequestOwnershipRequest> for OwnershipChange {
    fn from(request: RequestOwnershipRequest) -> Self {
        Self {
            name: request.name,
            uuids: request.uuids.into_vec(),
        }
    }
}

impl Action for ActionRequestOwnership {
    /// Taking entries is id 11: the first half of the ownership exchange.
    const ID: u32 = 11;

    type Input = OwnershipChange;
    type Output = Ownerships;

    /// Asks the Vault to name the caller as the holder of each entry, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if a `Uuid` does not
    /// read or a payload will not cross, [`ActionError::Store`] if the Vault's Layout could not be
    /// written, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcomes = ownership_batch(&mut ctx, input, take).await?;

        Ok(Ownerships::from(outcomes))
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnershipChange, RequestOwnershipRequest};

    #[test]
    fn a_request_becomes_the_change_the_exchange_carries() {
        let request =
            RequestOwnershipRequest::new("note".to_owned(), vec!["a".to_owned(), "b".to_owned()]);

        let change: OwnershipChange = request.into();

        assert_eq!(change.name, "note");
        assert_eq!(change.uuids, ["a", "b"]);
    }
}
