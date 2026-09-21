//! The preamble a Workspace and a Vault exchange before an action runs.
//!
//! The channel is up, but neither side yet knows what the other wants. The Workspace
//! speaks first: it names the action it wants run, the account it acts as, and which Vault
//! under the daemon it is reached with — one daemon may serve several, so the Vault is not
//! the daemon's to assume. The Vault answers with two bytes that echo the action's id,
//! which is what tells the Workspace the Vault read the same request — and only then do
//! both sides start the action itself.
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
/// A request is an id and a few names, so anything longer than this is not one; the bound is
/// what keeps a peer from asking for a name megabytes long to be allocated.
const MAX_REQUEST: usize = 4 * 1024;

/// What a Workspace asks a Vault for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Request {
    /// The id of the action to run.
    pub(crate) id: u32,
    /// The account the Workspace acts as.
    pub(crate) account: String,
    /// The Vault under the daemon's own that is being reached for.
    ///
    /// Nothing names the Vault the daemon serves, which is what an address without a sub-vault
    /// asks for; anything else names one below the root the daemon's own sits in. The daemon is
    /// reached as the authority of the link and is not told which port or host was dialled, so
    /// which Vault is meant has to travel: it is the one part of an address a connection cannot
    /// imply.
    pub(crate) sub: String,
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
    use rorolala_auth::{KeyAlgorithm, SecureStream, SigningKey};
    use rorolala_protocol::{ActionError, Channel, Socket};
    use tokio::io::duplex;

    use super::{MAX_REQUEST, Request, confirmation, read_request, write_request};

    /// A key with a given seed, so a test names the same identity twice.
    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(KeyAlgorithm::Ed25519, [seed; 32]).unwrap()
    }

    /// The two ends of one encrypted session over an in-memory stream.
    async fn session() -> (Channel, Channel) {
        let (client_io, server_io) = duplex(64 * 1024);
        let server = key(1);
        let expected = server.public_key();

        let accepting = tokio::spawn(async move {
            SecureStream::accept(Box::new(server_io) as Box<dyn Socket>, &server).await
        });

        let client =
            SecureStream::connect(Box::new(client_io) as Box<dyn Socket>, &key(2), &expected)
                .await
                .unwrap();
        let server = accepting.await.unwrap().unwrap();

        (client, server)
    }

    #[test]
    fn a_confirmation_is_the_low_two_bytes_of_an_id() {
        assert_eq!(confirmation(0), [0, 0]);
        assert_eq!(confirmation(1), [0, 1]);
        assert_eq!(confirmation(256), [1, 0]);
        assert_eq!(confirmation(0x1234_5678), [0x56, 0x78]);
        // The high two bytes are not part of what comes back, whatever they say.
        assert_eq!(confirmation(0xFFFF_0001), [0, 1]);
    }

    #[tokio::test]
    async fn a_request_round_trips_through_its_frame() {
        let (mut workspace, mut vault) = session().await;
        let request = Request {
            id: 7,
            account: "alice".to_string(),
            sub: "vaults/alpha".to_string(),
        };

        write_request(&mut workspace, &request).await.unwrap();
        let read = read_request(&mut vault).await.unwrap();

        assert_eq!(read, request);
    }

    #[tokio::test]
    async fn a_frame_longer_than_the_bound_is_refused() {
        let (mut workspace, mut vault) = session().await;
        // A name that could not be a request: the bound is what stops a peer from asking
        // for a name megabytes long to be allocated.
        let request = Request {
            id: 1,
            account: "a".repeat(MAX_REQUEST + 1),
            sub: String::new(),
        };

        write_request(&mut workspace, &request).await.unwrap();
        let error = read_request(&mut vault).await.unwrap_err();

        assert!(matches!(error, ActionError::ValueTooLarge));
    }
}
