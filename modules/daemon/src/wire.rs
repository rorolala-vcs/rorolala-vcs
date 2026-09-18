//! The preamble a Workspace and a Vault exchange before an action runs.
//!
//! The channel is up, but neither side yet knows what the other wants. The Workspace
//! speaks first: it names the action it wants run and the account it acts as. The Vault
//! answers with two bytes that echo the action's id, which is what tells the Workspace
//! the Vault read the same request — and only then do both sides start the action itself.
//!
//! The request is framed the way a synced value is: a four-byte length, then the encoding.
//! The confirmation is not a value but a check, so it is exactly the two bytes it is. Both
//! live here rather than at either end, so the two ends cannot come to disagree about what
//! the bytes mean.

use rorolala_protocol::{ActionError, Channel, Encodable};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// The largest request a Vault will read.
///
/// A request is an id and a name, so anything longer than this is not one; the bound is
/// what keeps a peer from asking for a name megabytes long to be allocated.
const MAX_REQUEST: usize = 4 * 1024;

/// What a Workspace asks a Vault for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Request {
    /// The id of the action to run.
    pub(crate) id: u32,
    /// The account the Workspace acts as.
    pub(crate) account: String,
}

/// The two bytes that confirm an action id: its low two, in big-endian order.
///
/// An id travels in four bytes; what comes back is the half that fits, which is enough to
/// tell one action from another for as long as ids stay small — which the registry, laid
/// out by id, already assumes.
#[must_use]
pub(crate) const fn confirmation(id: u32) -> [u8; 2] {
    let bytes = id.to_be_bytes();

    [bytes[2], bytes[3]]
}

/// Writes `request` to `channel`.
pub(crate) async fn write_request(
    channel: &mut Channel,
    request: &Request,
) -> Result<(), ActionError> {
    write_frame(channel, request).await
}

/// Reads the request `channel` holds.
pub(crate) async fn read_request(channel: &mut Channel) -> Result<Request, ActionError> {
    read_frame(channel).await
}

/// Writes the confirmation of `id` to `channel`.
pub(crate) async fn write_confirmation(channel: &mut Channel, id: u32) -> Result<(), ActionError> {
    channel.write_all(&confirmation(id)).await?;
    channel.flush().await?;

    Ok(())
}

/// Reads the confirmation `channel` holds.
pub(crate) async fn read_confirmation(channel: &mut Channel) -> Result<[u8; 2], ActionError> {
    let mut bytes = [0_u8; 2];
    channel.read_exact(&mut bytes).await?;

    Ok(bytes)
}

/// Writes one length-prefixed frame to `channel`.
async fn write_frame<Value>(channel: &mut Channel, value: &Value) -> Result<(), ActionError>
where
    Value: Encodable + Sync,
{
    let bytes = value.encode()?;
    let length = u32::try_from(bytes.len()).map_err(|_| ActionError::ValueTooLarge)?;

    channel.write_all(&length.to_be_bytes()).await?;
    channel.write_all(&bytes).await?;
    channel.flush().await?;

    Ok(())
}

/// Reads one length-prefixed frame from `channel` and decodes it.
async fn read_frame<Value>(channel: &mut Channel) -> Result<Value, ActionError>
where
    Value: Encodable,
{
    let mut header = [0_u8; 4];
    channel.read_exact(&mut header).await?;

    let length =
        usize::try_from(u32::from_be_bytes(header)).map_err(|_| ActionError::ValueTooLarge)?;
    if length > MAX_REQUEST {
        return Err(ActionError::ValueTooLarge);
    }

    let mut bytes = vec![0_u8; length];
    channel.read_exact(&mut bytes).await?;

    Ok(Value::decode(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::confirmation;

    #[test]
    fn a_confirmation_is_the_low_two_bytes_of_an_id() {
        assert_eq!(confirmation(0), [0, 0]);
        assert_eq!(confirmation(1), [0, 1]);
        assert_eq!(confirmation(256), [1, 0]);
        assert_eq!(confirmation(0x1234_5678), [0x56, 0x78]);
    }
}
