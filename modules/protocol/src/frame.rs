//! Frames: a value written as its length and then its bytes.
//!
//! A value that crosses a sync is mostly bytes already — a packet of entries, a blob, the answer to
//! "which of these do you hold" — and putting one of those through serde is a copy and a visit per
//! element for a shape that is bytes and nothing else. So the frame is a layer of its own: it knows a
//! length and a run of bytes, and [`send_value`]/[`receive_value`] in [`sync`](crate::sync) are the
//! serde shell over it for the values that really are numbers and structs.
//!
//! What a frame is for is *one* value: its length is what tells the other side where it ends, and a
//! value too long for one frame — a content bigger than [`MAX_FRAME`] — is carried in pieces by its
//! caller rather than by a frame of its own.

use std::mem::size_of;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::{ActionContext, ActionError, Channel};

/// The longest value one frame will carry.
///
/// A value states its length in four bytes, so a peer could otherwise ask for four gigabytes to be
/// allocated before sending any of it. This is what a value is held to, on the way out as well as the
/// way in, so what is too large to cross is refused where it is known rather than by a peer that then
/// reads a frame it will not take.
///
/// What it bounds is a value *once framed*, which is why a caller carrying something larger than
/// this — a content, or a batch — carries it in pieces rather than in one value.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// The room a frame's own length takes in front of its bytes.
const LENGTH: usize = size_of::<u32>();

impl ActionContext<'_> {
    /// Hands `parts` to the peer as one frame, in order.
    ///
    /// Several parts are one value: what crosses is their total length and then them, so a packet's
    /// header and its entries are one frame rather than a frame each. That is what lets a caller send
    /// what it already holds instead of building one buffer to hold all of it.
    ///
    /// # Errors
    ///
    /// Returns [`NoChannel`](ActionError::NoChannel) if there is nothing to exchange over,
    /// [`ValueTooLarge`](ActionError::ValueTooLarge) if the parts together are longer than a frame,
    /// and [`Io`](ActionError::Io) if the channel failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// ctx.write_frame(&[header, body]).await?;
    /// ```
    pub async fn write_frame(&mut self, parts: &[&[u8]]) -> Result<(), ActionError> {
        let channel = self.channel_mut().ok_or(ActionError::NoChannel)?;

        write_frame(channel, parts).await
    }

    /// Takes one frame from the peer, answering its bytes.
    ///
    /// # Errors
    ///
    /// Returns [`NoChannel`](ActionError::NoChannel) if there is nothing to exchange over,
    /// [`ValueTooLarge`](ActionError::ValueTooLarge) if the peer says it is sending more than a
    /// frame carries, and [`Io`](ActionError::Io) if the channel failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let bytes = ctx.read_frame().await?;
    /// ```
    pub async fn read_frame(&mut self) -> Result<Vec<u8>, ActionError> {
        let channel = self.channel_mut().ok_or(ActionError::NoChannel)?;

        read_frame(channel).await
    }
}

/// Writes `parts` to `channel` as one frame: their total length, then each part in order.
///
/// The parts are handed to the channel as they lie rather than joined into one buffer first: what a
/// value already is, a frame should not copy. The channel is what cuts them into records, and it
/// seals what it is handed where it lies.
///
/// # Errors
///
/// Returns [`ValueTooLarge`](ActionError::ValueTooLarge) if the parts together are longer than
/// [`MAX_FRAME`], and [`Io`](ActionError::Io) if the channel failed.
pub async fn write_frame(channel: &mut Channel, parts: &[&[u8]]) -> Result<(), ActionError> {
    let total = parts
        .iter()
        .try_fold(0_usize, |sum, part| sum.checked_add(part.len()))
        .ok_or(ActionError::ValueTooLarge)?;
    if total > MAX_FRAME {
        return Err(ActionError::ValueTooLarge);
    }
    let length = u32::try_from(total)
        .map_err(|_| ActionError::ValueTooLarge)?
        .to_be_bytes();

    // The length is the first part rather than a write of its own: a record holds however much of
    // the frame begins at it, and a length written alone would be 20 bytes of record overhead for
    // every value a run exchanges.
    let mut framed: Vec<&[u8]> = Vec::with_capacity(parts.len() + 1);
    framed.push(&length);
    framed.extend_from_slice(parts);

    channel.write_parts(&framed).await?;
    channel.flush().await?;

    Ok(())
}

/// Reads one frame from `channel`, answering its bytes.
///
/// # Errors
///
/// Returns [`ValueTooLarge`](ActionError::ValueTooLarge) if the peer says it is sending more than
/// [`MAX_FRAME`], and [`Io`](ActionError::Io) if the channel failed — a channel that ended partway
/// through a frame included, which is an `UnexpectedEof` rather than a short value.
pub async fn read_frame(channel: &mut Channel) -> Result<Vec<u8>, ActionError> {
    let mut header = [0_u8; LENGTH];
    channel.read_exact(&mut header).await?;

    let length =
        usize::try_from(u32::from_be_bytes(header)).map_err(|_| ActionError::ValueTooLarge)?;
    if length > MAX_FRAME {
        return Err(ActionError::ValueTooLarge);
    }

    let mut bytes = vec![0_u8; length];
    channel.read_exact(&mut bytes).await?;

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use rorolala_auth::{KeyAlgorithm, MAX_PLAINTEXT, SecureStream, SigningKey};
    use tokio::io::{AsyncWriteExt as _, duplex};

    use super::{MAX_FRAME, read_frame, write_frame};
    use crate::{ActionError, Channel, Socket};

    /// A signing key from a fixed seed, so a test names the same identity twice.
    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(KeyAlgorithm::Ed25519, [seed; 32]).expect("a seed names a key")
    }

    /// A client and a server session, handshaken over an in-memory socket.
    ///
    /// The socket is larger than one record, unlike the one `sync`'s tests use: these write frames
    /// that are several records long before anything reads them, and a buffer smaller than what is
    /// written would wait for the read that has not started yet.
    async fn session_pair() -> (Channel, Channel) {
        let client = key(1);
        let server = key(2);
        let server_key = server.public_key();

        let (client_socket, server_socket) = duplex(4 * MAX_PLAINTEXT);
        let client_socket: Box<dyn Socket> = Box::new(client_socket);
        let server_socket: Box<dyn Socket> = Box::new(server_socket);

        let (client, server) = tokio::join!(
            SecureStream::connect(client_socket, &client, &server_key),
            SecureStream::accept(server_socket, &server),
        );

        (
            client.expect("the client handshakes"),
            server.expect("the server handshakes"),
        )
    }

    #[tokio::test]
    async fn a_frame_of_no_bytes_is_a_frame() {
        let (mut client, mut server) = session_pair().await;

        write_frame(&mut client, &[]).await.unwrap();

        let heard: Vec<u8> = read_frame(&mut server).await.unwrap();
        assert_eq!(heard, Vec::<u8>::new());
    }

    #[tokio::test]
    async fn a_frame_of_several_parts_reads_back_as_their_concatenation() {
        let (mut client, mut server) = session_pair().await;

        write_frame(&mut client, &[b"one", b"", b"two"])
            .await
            .unwrap();
        assert_eq!(read_frame(&mut server).await.unwrap(), b"onetwo");
    }

    #[tokio::test]
    async fn a_frame_larger_than_one_record_reads_back_whole() {
        let (mut client, mut server) = session_pair().await;

        // Three parts of a whole record each: the length and the parts together are several records,
        // which is the branch that does not build one buffer.
        let parts: Vec<Vec<u8>> = (0..3)
            .map(|at| vec![u8::try_from(at).unwrap_or(0); MAX_PLAINTEXT])
            .collect();
        let handed: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();

        write_frame(&mut client, &handed).await.unwrap();

        let read = read_frame(&mut server).await.unwrap();
        assert_eq!(read, parts.concat());
    }

    #[tokio::test]
    async fn a_value_past_the_frame_bound_will_not_be_sent() {
        let (mut client, _server) = session_pair().await;

        let oversized = vec![0_u8; MAX_FRAME + 1];
        let error = write_frame(&mut client, &[oversized.as_slice()])
            .await
            .unwrap_err();
        assert!(matches!(error, ActionError::ValueTooLarge));
    }

    #[tokio::test]
    async fn a_frame_that_claims_more_than_the_bound_is_refused() {
        let (mut client, mut server) = session_pair().await;

        // A peer can frame any four bytes it likes; the header must not be trusted.
        client.write_all(&u32::MAX.to_be_bytes()).await.unwrap();
        client.flush().await.unwrap();

        let error = read_frame(&mut server).await.unwrap_err();
        assert!(matches!(error, ActionError::ValueTooLarge));
    }

    #[tokio::test]
    async fn a_context_without_a_channel_reports_no_channel() {
        let mut context =
            crate::ActionContext::new_workspace_ctx(rorolala_auth::Account::default());

        assert!(matches!(
            context.write_frame(&[b"x"]).await.unwrap_err(),
            ActionError::NoChannel
        ));
        assert!(matches!(
            context.read_frame().await.unwrap_err(),
            ActionError::NoChannel
        ));
    }
}
