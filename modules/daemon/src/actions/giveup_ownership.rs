//! `giveup-ownership`: letting go of entries of the Vault's Layout.
//!
//! The other half of [`request-ownership`](crate::ActionRequestOwnership): a Layout names who holds
//! each entry, and an account that is done with some asks the Vault to name nobody for each of them.
//! Only the holder can: an entry another account holds is refused, and one nobody holds is already
//! what was asked for. What the Workspace keeps a copy of is changed to match, the command that
//! asked does that.
//!
//! As the other half is, the `Uuid`s are let go of in one run rather than one connection apiece.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use rorolala_utils_lazyffi::{StrSpan, lazyffi};

use super::layout_remote::{OwnershipChange, Ownerships, giveup, ownership_batch};

/// An Action that names nobody as the holder of each of the Vault's entries.
///
/// # Input
///
/// What to let go of: the words the bar is drawn under and the `Uuid`s of the entries, as text. The
/// `Uuid`s belong to the caller, so they are read on the Workspace and cross to the Vault, which is
/// the side that has a Layout to name them in; the words cross with them because the caller is the
/// side that has them, see `ownership_batch`. Which account lets go is not input: the session
/// already says.
///
/// # Output
///
/// What became of each, one outcome per `Uuid`, in the order they were asked for. Each is what one
/// let-go answers on its own: the entry is now held by nobody, the Vault holds no such entry, or
/// another account holds it. [`Ownerships`](super::layout_remote::Ownerships) is what a C caller
/// reads.
pub struct ActionGiveupOwnership;

/// What to let go of, and under what words the bar is drawn.
///
/// # FFI
///
/// A `RolaDaemonGiveupOwnershipRequest` the caller fills. `name` is borrowed for the call and
/// released by the caller. `uuids` is a `RolaStrSpan` of the `Uuid`s to let go of, each read as text
/// and borrowed for the call.
#[lazyffi(export = RolaDaemonGiveupOwnershipRequest, input)]
#[derive(Debug, Clone, Default)]
pub struct GiveupOwnershipRequest {
    /// The words the bar is drawn under, or empty for none.
    pub name: String,
    /// The `Uuid`s of the entries to let go of, in the order they are answered.
    pub uuids: StrSpan,
}

impl GiveupOwnershipRequest {
    /// The request a run inside Rust fills, taking the `Uuid`s as strings.
    #[must_use]
    pub fn new(name: String, uuids: Vec<String>) -> Self {
        Self {
            name,
            uuids: StrSpan::from(uuids),
        }
    }
}

impl From<GiveupOwnershipRequest> for OwnershipChange {
    fn from(request: GiveupOwnershipRequest) -> Self {
        Self {
            name: request.name,
            uuids: request.uuids.into_vec(),
        }
    }
}

impl Action for ActionGiveupOwnership {
    /// Letting go is id 12: the second half of the ownership exchange.
    const ID: u32 = 12;

    type Input = OwnershipChange;
    type Output = Ownerships;

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

        Ok(Ownerships::from(outcomes))
    }
}
