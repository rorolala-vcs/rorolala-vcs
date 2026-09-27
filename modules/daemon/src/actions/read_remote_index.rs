//! `read-remote-index`: read one index object from the other end of an exchange.
//!
//! The one-object reach of [`list_remote_index`](crate::list_remote_index): a Workspace names a hash,
//! the Vault reads the object under it from its own index, and what comes back is the object and, for
//! a Version, the number its chain traces to. The number is worked out on the Vault because the chain
//! the Version sits in is there, not here — and it crosses beside the object rather than in it, since
//! a Version is written down without its number.

use std::str::FromStr as _;

use rorolala_errors::{BincodeError, Failure as _};
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_storage::Key;
use rorolala_vcs::{
    UNKNOWN_VERSION, VCSIndex, VCSIndexObject, VCSIndexReadingError, VCSWrite as _, Version,
};

use super::sync_index::local_index;
use super::sync_storage::{Side, carry_blob};

/// An Action that reads one index object from the other end's index.
///
/// # Input
///
/// The hash of the object, as hex or named with `blake3:` in front of it. It is read on the
/// Workspace and crosses to the Vault, which is the side that has an index to read it from.
///
/// # Output
///
/// The object. A Version carries the number its chain traces to, worked out on the Vault — or
/// [`UNKNOWN_VERSION`] when the chain will not trace — since the chain is there and not here.
pub struct ActionReadRemoteIndex;

impl Action for ActionReadRemoteIndex {
    /// The read is id 9.
    const ID: u32 = 9;

    type Input = String;
    type Output = VCSIndexObject;

    /// Asks the other end for the object under the hash, and answers with what it read.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a Vault,
    /// [`ActionError::MissingObject`] if no object is held under the hash, [`ActionError::Codec`] if
    /// the hash does not read or a payload will not cross, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_index(&ctx)?;

        // The hash is the caller's, so it belongs to the Workspace and crosses to the Vault first.
        let here = input.into_inner();
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let named = arrived
            .or(to_vault)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        let key = Key::from_str(named.trim()).map_err(|_| {
            ActionError::Codec(BincodeError::new("the hash does not read".to_owned()))
        })?;

        // What is read is read on the Vault: it is the side with the index. What crosses back is the
        // object's bytes and its number, or nothing when no object is held under the hash — and a
        // missing object is answered as nothing rather than as a failure, so both ends reach the one
        // decision after the carrying rather than the Vault leaving the Workspace waiting.
        let held = if ctx.is_vault() {
            read_the_object(&local, &key).await?
        } else {
            None
        };

        // The number is not part of a Version's bytes — it is derived, and written down nowhere — so
        // the object and its number cross together as one payload, and the two ends read the same.
        let mine = if ctx.is_vault() {
            Some(held.encode().map_err(codec_failed)?)
        } else {
            None
        };
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        let payload: Option<(Vec<u8>, Option<u64>)> = if ctx.is_vault() {
            held
        } else {
            Option::decode(bytes).map_err(codec_failed)?
        };
        let Some((object, number)) = payload else {
            return Err(ActionError::MissingObject);
        };

        object_with_number(&object, number)
    }
}

/// The object under `key` in `local` and its number, or nothing when none is held.
///
/// A Version is the only kind whose number is worth carrying: the chain it sits in is here on the
/// Vault, so it is traced here rather than left for a side that cannot reach it. A chain that will
/// not trace leaves the number as nothing, as a listing that could not work one out does.
async fn read_the_object(
    local: &VCSIndex,
    key: &Key,
) -> Result<Option<(Vec<u8>, Option<u64>)>, ActionError> {
    let object = match local.read(*key).await {
        Ok(object) => object,
        Err(VCSIndexReadingError::NotFound { .. }) => return Ok(None),
        Err(error) => return Err(ActionError::Store(error.reason())),
    };

    let number = match &object {
        VCSIndexObject::Version(version) => local.version_num(version).await.ok(),
        _ => None,
    };

    Ok(Some((object.encode(), number)))
}

/// The object the bytes hold, with `number` written into it when it is a Version.
///
/// A Version is read back without its number — it is written down nowhere — so the number that
/// crossed beside it is put back in, which is what a reader is shown. Anything else is itself.
fn object_with_number(bytes: &[u8], number: Option<u64>) -> Result<VCSIndexObject, ActionError> {
    let object =
        VCSIndexObject::decode(bytes).map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(match object {
        VCSIndexObject::Version(version) => VCSIndexObject::Version(Version::new_bare_version(
            *version.variant(),
            number.unwrap_or(UNKNOWN_VERSION),
        )),
        other => other,
    })
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(BincodeError::new(error.to_string()))
}
