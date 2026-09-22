//! Driving a transfer between two ends over a stream.
//!
//! The exchange is one round of asking and one of sending, over any stream that reads and writes
//! bytes. One end brings a list of keys and speaks first; the other is told them. Each says which of
//! them it holds, and each sends the ones the other does not — so one pass leaves both ends holding
//! all of them, and neither end has to know in advance which way the content will actually move.
//!
//! ```text
//! initiate                              respond
//!   write MAGIC     ───────────────▶      read MAGIC, refuse if it is not this kind's
//!   read MAGIC      ◀───────────────      write MAGIC
//!   write keys      ───────────────▶      read keys
//!   write presence  ───────────────▶      read presence
//!   read presence   ◀───────────────      write presence
//!   for each key: send what the peer lacks, take what it has and this end lacks
//! ```
//!
//! The password is exchanged before anything else, so a peer speaking another protocol is turned
//! away before a key is named. The two ends write their presence in that order — the one that spoke
//! first writes before it reads, the other reads before it writes — so neither has to drain its own
//! write buffer while the other waits to write, which is what a pair of large writes would ask of
//! it. Each key then crosses in one direction only, which the two ends agree on from the two
//! presences, so the pass is the same sequence of moves on both sides.

use std::fmt;
use std::io;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::{
    BLAKE3_HASH_LEN, Blake3Hash, Key, PROTOCOL_MAGIC_LEN, Presence, TransferableBackend, split_u64,
};

/// How many bytes of content one frame may carry.
///
/// A frame's length is written by the peer, and a peer is only as trustworthy as the password it
/// gave — which is a check against a mistake, not against an enemy. This bounds what a reader is
/// asked to hold on the word of what it has read, and it is checked on the way out as well as the
/// way in, so content too large to cross is refused where it is known rather than by a peer that
/// then reads a frame it will not take.
const MAX_FRAME: usize = 1 << 30;

/// What a transfer failed with.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// The peer does not speak this protocol: the password it gave is not this kind's.
    Mismatched,
    /// What came over the stream does not read as this protocol.
    Malformed,
    /// A frame is larger than this protocol carries.
    TooLarge,
    /// The stream itself failed.
    Stream(io::Error),
    /// The store failed.
    Store(E),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatched => formatter.write_str("the peer does not speak this protocol"),
            Self::Malformed => formatter.write_str("what the peer sent does not read"),
            Self::TooLarge => formatter.write_str("a frame is too large to cross"),
            Self::Stream(source) => write!(formatter, "the stream failed: {source}"),
            Self::Store(source) => write!(formatter, "the store failed: {source}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stream(source) => Some(source),
            Self::Store(source) => Some(source),
            _ => None,
        }
    }
}

/// Runs the exchange as the end that speaks first, and answers when it is done.
///
/// Both ends bring the same `keys`. What this end holds and the other does not is sent, and what the
/// other holds and this end does not is taken, so one pass leaves both holding all of them.
///
/// # Errors
///
/// [`Error::Mismatched`] if the peer does not speak this protocol, [`Error::Malformed`] if what it
/// sends does not read, and whatever the store or the stream fails with.
pub async fn initiate<B, S>(backend: &B, mut stream: S, keys: &[Key]) -> Result<(), Error<B::Error>>
where
    B: TransferableBackend,
    S: AsyncRead + AsyncWrite + Unpin,
{
    write_magic(&mut stream, B::PROTOCOL_MAGIC).await?;
    let theirs = read_magic(&mut stream).await?;
    if theirs != B::PROTOCOL_MAGIC {
        return Err(Error::Mismatched);
    }

    let mine = holds(backend, keys).await?;
    let mut their = Presence::all_missing(keys.len());

    write_frame(&mut stream, &keys_to_bytes(keys)).await?;
    write_frame(&mut stream, &Vec::from(mine.clone())).await?;
    read_presence(&mut stream, keys.len(), &mut their).await?;

    for (at, key) in keys.iter().enumerate() {
        match (mine.held(at), their.held(at)) {
            (true, false) => send(backend, &mut stream, key).await?,
            (false, true) => receive(backend, &mut stream, key).await?,
            _ => {}
        }
    }

    stream.flush().await.map_err(Wire::Stream)?;

    Ok(())
}

/// Answers the end that spoke first, and returns when the exchange is done.
///
/// The keys are the peer's to name, so this end is told them rather than bringing them.
///
/// # Errors
///
/// [`Error::Mismatched`] if the peer does not speak this protocol, [`Error::Malformed`] if what it
/// sends does not read, and whatever the store or the stream fails with.
pub async fn respond<B, S>(backend: &B, mut stream: S) -> Result<(), Error<B::Error>>
where
    B: TransferableBackend,
    S: AsyncRead + AsyncWrite + Unpin,
{
    // The password is written back before it is judged, so that a peer which gave another kind's
    // password still receives one to judge: both ends then answer "mismatched" rather than the one
    // that spoke first waiting on a reply that is never coming.
    let theirs = read_magic(&mut stream).await?;
    write_magic(&mut stream, B::PROTOCOL_MAGIC).await?;
    if theirs != B::PROTOCOL_MAGIC {
        return Err(Error::Mismatched);
    }

    let keys = keys_from_bytes(&read_frame(&mut stream).await?)?;

    let mine = holds(backend, &keys).await?;
    let mut their = Presence::all_missing(keys.len());

    read_presence(&mut stream, keys.len(), &mut their).await?;
    write_frame(&mut stream, &Vec::from(mine.clone())).await?;

    for (at, key) in keys.iter().enumerate() {
        match (mine.held(at), their.held(at)) {
            (true, false) => send(backend, &mut stream, key).await?,
            (false, true) => receive(backend, &mut stream, key).await?,
            _ => {}
        }
    }

    stream.flush().await.map_err(Wire::Stream)?;

    Ok(())
}

/// Sends what this end holds under `key`, marking it absent if it cannot produce it now.
///
/// The mark is what keeps the two ends in step when a key was held when presence was answered and is
/// not held when its turn comes: the reader is told to expect nothing rather than left waiting for
/// content that will not come.
async fn send<B, S>(backend: &B, stream: &mut S, key: &Key) -> Result<(), Error<B::Error>>
where
    B: TransferableBackend,
    S: AsyncWrite + Unpin,
{
    match backend.content(key).await.map_err(Error::Store)? {
        Some(content) => {
            write_frame(stream, &[1]).await?;
            write_frame(stream, &content).await?;
        }
        None => write_frame(stream, &[0]).await?,
    }

    Ok(())
}

/// Takes what the peer holds under `key`, if it said it was sending any.
async fn receive<B, S>(backend: &B, stream: &mut S, key: &Key) -> Result<(), Error<B::Error>>
where
    B: TransferableBackend,
    S: AsyncRead + Unpin,
{
    match read_frame(stream).await?.first() {
        Some(0) => Ok(()),
        Some(_) => {
            let content = read_frame(stream).await?;

            backend.accept(key, content).await.map_err(Error::Store)
        }
        None => Err(Wire::Malformed.into()),
    }
}

/// Which of `keys` the backend can produce.
async fn holds<B>(backend: &B, keys: &[Key]) -> Result<Presence, Error<B::Error>>
where
    B: TransferableBackend,
{
    backend.contains_keys(keys).await.map_err(Error::Store)
}

/// A failure of the wire itself: not the store's, and not this protocol's.
enum Wire {
    /// What the peer sent does not read.
    Malformed,
    /// A frame is larger than this protocol carries.
    TooLarge,
    /// The stream failed.
    Stream(io::Error),
}

impl<E> From<Wire> for Error<E> {
    fn from(wire: Wire) -> Self {
        match wire {
            Wire::Malformed => Self::Malformed,
            Wire::TooLarge => Self::TooLarge,
            Wire::Stream(source) => Self::Stream(source),
        }
    }
}

/// Writes the password, which is as wide as it is and carries no length.
async fn write_magic<W>(stream: &mut W, magic: [u8; PROTOCOL_MAGIC_LEN]) -> Result<(), Wire>
where
    W: AsyncWrite + Unpin,
{
    stream.write_all(&magic).await.map_err(Wire::Stream)
}

/// Reads the password.
async fn read_magic<R>(stream: &mut R) -> Result<[u8; PROTOCOL_MAGIC_LEN], Wire>
where
    R: AsyncRead + Unpin,
{
    let mut theirs = [0_u8; PROTOCOL_MAGIC_LEN];
    stream.read_exact(&mut theirs).await.map_err(Wire::Stream)?;

    Ok(theirs)
}

/// Writes a length-prefixed run of bytes.
async fn write_frame<W>(stream: &mut W, bytes: &[u8]) -> Result<(), Wire>
where
    W: AsyncWrite + Unpin,
{
    if bytes.len() > MAX_FRAME {
        return Err(Wire::TooLarge);
    }

    stream
        .write_all(&(bytes.len() as u64).to_be_bytes())
        .await
        .map_err(Wire::Stream)?;
    stream.write_all(bytes).await.map_err(Wire::Stream)
}

/// Reads a length-prefixed run of bytes.
async fn read_frame<R>(stream: &mut R) -> Result<Vec<u8>, Wire>
where
    R: AsyncRead + Unpin,
{
    let mut length = [0_u8; size_of::<u64>()];
    stream.read_exact(&mut length).await.map_err(Wire::Stream)?;
    let length = u64::from_be_bytes(length);
    let length = usize::try_from(length).map_err(|_| Wire::Malformed)?;
    if length > MAX_FRAME {
        return Err(Wire::TooLarge);
    }

    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes).await.map_err(Wire::Stream)?;

    Ok(bytes)
}

/// Writes the keys, as the digest of each.
fn keys_to_bytes(keys: &[Key]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(size_of::<u64>() + keys.len() * BLAKE3_HASH_LEN);
    bytes.extend_from_slice(&(keys.len() as u64).to_be_bytes());

    for key in keys {
        bytes.extend_from_slice(key.digest());
    }

    bytes
}

/// Reads the keys [`keys_to_bytes`] wrote.
fn keys_from_bytes(bytes: &[u8]) -> Result<Vec<Key>, Wire> {
    let (count, mut rest) = split_u64(bytes).ok_or(Wire::Malformed)?;
    let count = usize::try_from(count).map_err(|_| Wire::Malformed)?;
    let mut keys = Vec::with_capacity(count.min(rest.len() / BLAKE3_HASH_LEN));

    for _ in 0..count {
        let (digest, after) = rest
            .split_at_checked(BLAKE3_HASH_LEN)
            .ok_or(Wire::Malformed)?;
        let digest: Blake3Hash = digest.try_into().map_err(|_| Wire::Malformed)?;

        keys.push(Key::new(digest));
        rest = after;
    }

    if rest.is_empty() {
        Ok(keys)
    } else {
        Err(Wire::Malformed)
    }
}

/// Reads a presence that has to be exactly as long as the batch it answers.
async fn read_presence<R>(stream: &mut R, len: usize, into: &mut Presence) -> Result<(), Wire>
where
    R: AsyncRead + Unpin,
{
    let bytes = read_frame(stream).await?;
    if bytes.len() != len {
        return Err(Wire::Malformed);
    }

    *into = Presence::from(bytes);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _, duplex};

    use super::{Error, initiate, respond};
    use crate::{
        AlgorithmChoice, Chunking, Codec, Key, RorolalaStorage, StorageBackend as _,
        TransferableBackend as _,
    };

    /// A store of its own, over a directory of its own.
    fn store(label: &str) -> RorolalaStorage {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-transfer-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);

        RorolalaStorage::create(&dir)
    }

    /// Writes `content` into `store` as an object, and answers its key.
    async fn writes(store: &RorolalaStorage, content: &[u8]) -> Key {
        store
            .write_object(content, Codec::Raw)
            .await
            .expect("the object is written")
    }

    /// Whether `store` can produce `key`, asked of it in a batch of one.
    async fn holds(store: &RorolalaStorage, key: &Key) -> bool {
        store
            .contains_keys(&[*key])
            .await
            .expect("the store answers")
            .held(0)
    }

    #[tokio::test]
    async fn two_ends_both_end_up_holding_everything() {
        let left = store("both-left");
        let right = store("both-right");
        let first = writes(&left, b"what the left end holds").await;
        let second = writes(&right, b"what the right end holds").await;
        let keys = [first, second];

        let (mut mine, mut theirs) = duplex(64 * 1024);
        let answering_with = right.clone();
        let answering = tokio::spawn(async move { respond(&answering_with, &mut theirs).await });

        initiate(&left, &mut mine, &keys)
            .await
            .expect("the transfer is made");
        answering
            .await
            .expect("the answering end finishes")
            .expect("the answering end is happy");

        for key in keys {
            assert!(holds(&left, &key).await, "the left end holds it");
            assert!(holds(&right, &key).await, "the right end holds it");
        }
    }

    #[tokio::test]
    async fn content_another_end_holds_is_taken_whole() {
        let left = store("take-left");
        let right = store("take-right");

        // Content the right end holds, which the left end is to take, kept as chunks so that what
        // crosses is content rather than entries.
        let content: Vec<u8> = b"the same words over and over, "
            .iter()
            .cycle()
            .take(64 * 1024)
            .copied()
            .collect();
        let path: PathBuf =
            std::env::temp_dir().join(format!("rorolala-transfer-chunked-{}", std::process::id()));
        fs::write(&path, &content).expect("the content is written to a file");
        let key = right
            .write_file(
                &path,
                AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 4096 }),
            )
            .await
            .expect("the content is written");
        let _ = fs::remove_file(&path);

        assert!(
            right
                .read_manifest(&key)
                .await
                .expect("the manifest is read")
                .is_some(),
            "the content was kept as chunks"
        );

        let (mut mine, mut theirs) = duplex(64 * 1024);
        let answering = tokio::spawn(async move { respond(&right, &mut theirs).await });

        initiate(&left, &mut mine, &[key])
            .await
            .expect("the transfer is made");
        answering
            .await
            .expect("the answering end finishes")
            .expect("the answering end is happy");

        assert!(holds(&left, &key).await, "the content is now held");
        assert_eq!(
            left.read_object(&key).await.ok(),
            Some(content),
            "what crossed is the content"
        );
    }

    #[tokio::test]
    async fn content_the_left_holds_is_stored_by_the_right() {
        let left = store("push-left");
        let right = store("push-right");
        let content = b"what the left end sends";
        let key = writes(&left, content).await;

        let (mut mine, mut theirs) = duplex(64 * 1024);
        let answering_with = right.clone();
        let answering = tokio::spawn(async move { respond(&answering_with, &mut theirs).await });

        initiate(&left, &mut mine, &[key])
            .await
            .expect("the transfer is made");
        answering
            .await
            .expect("the answering end finishes")
            .expect("the answering end is happy");

        assert_eq!(
            right.read_object(&key).await.ok(),
            Some(content.to_vec()),
            "what crossed is the content"
        );
    }

    #[tokio::test]
    async fn a_peer_that_speaks_another_protocol_is_turned_away() {
        let backend = store("wrong-magic");
        let (mut peer, mut mine) = duplex(64);

        // A peer whose password is not this kind's: it is refused before a key is named. It stays to
        // read the reply, since the refusing end writes its own password back before judging.
        let speaking = tokio::spawn(async move {
            let _ = peer.write_all(b"NOTROLA!").await;

            let mut reply = [0_u8; 8];
            let _ = peer.read_exact(&mut reply).await;
        });

        let result = respond(&backend, &mut mine).await;
        let _ = speaking.await;

        assert!(matches!(result, Err(Error::Mismatched)), "{result:?}");
    }

    #[tokio::test]
    async fn a_peer_that_stops_halfway_is_reported() {
        let left = store("half-left");
        let key = writes(&left, b"the object").await;

        // The answering end goes away with the stream rather than answering.
        let (mut mine, theirs) = duplex(64 * 1024);
        drop(theirs);

        let result = initiate(&left, &mut mine, &[key]).await;

        assert!(matches!(result, Err(Error::Stream(_))), "{result:?}");
    }

    #[tokio::test]
    async fn a_foreign_password_is_turned_away_by_the_end_that_spoke_first() {
        let left = store("wrong-magic-first");
        let key = writes(&left, b"the object").await;
        let (mut peer, mut mine) = duplex(64);

        // A peer that reads the password and gives one that is not this kind's.
        let speaking = tokio::spawn(async move {
            let mut theirs = [0_u8; 8];
            let _ = peer.read_exact(&mut theirs).await;
            let _ = peer.write_all(b"NOTROLA!").await;
        });

        let result = initiate(&left, &mut mine, &[key]).await;
        let _ = speaking.await;

        assert!(matches!(result, Err(Error::Mismatched)), "{result:?}");
    }

    #[tokio::test]
    async fn a_key_list_that_does_not_read_is_refused_on_the_wire() {
        let backend = store("bad-keys");
        let (mut peer, mut mine) = duplex(64 * 1024);

        // The password, then a key list whose count promises a record it does not carry. The peer
        // stays to read the reply password before the key list is judged.
        let speaking = tokio::spawn(async move {
            let _ = peer.write_all(b"ROLASTOR").await;
            let _ = peer.write_all(&8_u64.to_be_bytes()).await;
            let _ = peer.write_all(&1_u64.to_be_bytes()).await;

            let mut reply = [0_u8; 8];
            let _ = peer.read_exact(&mut reply).await;
        });

        let result = respond(&backend, &mut mine).await;
        let _ = speaking.await;

        assert!(matches!(result, Err(Error::Malformed)), "{result:?}");
    }

    #[tokio::test]
    async fn a_key_held_by_both_ends_and_one_by_neither_move_nothing() {
        let left = store("shared-left");
        let right = store("shared-right");
        let shared = writes(&left, b"what both ends hold").await;
        let _ = writes(&right, b"what both ends hold").await;
        let absent = Key::new([9; 32]);
        let keys = [shared, absent];

        let (mut mine, mut theirs) = duplex(64 * 1024);
        let answering_with = right.clone();
        let answering = tokio::spawn(async move { respond(&answering_with, &mut theirs).await });

        initiate(&left, &mut mine, &keys)
            .await
            .expect("the transfer is made");
        answering
            .await
            .expect("the answering end finishes")
            .expect("the answering end is happy");

        assert!(holds(&left, &shared).await, "what both hold is held");
        assert!(holds(&right, &shared).await, "what both hold is held");
        assert!(
            !holds(&left, &absent).await && !holds(&right, &absent).await,
            "a key neither holds is not conjured up"
        );
    }

    #[tokio::test]
    async fn an_empty_batch_moves_nothing() {
        let left = store("empty-batch-left");
        let right = store("empty-batch-right");

        let (mut mine, mut theirs) = duplex(64 * 1024);
        let answering_with = right.clone();
        let answering = tokio::spawn(async move { respond(&answering_with, &mut theirs).await });

        initiate(&left, &mut mine, &[])
            .await
            .expect("the transfer is made");
        answering
            .await
            .expect("the answering end finishes")
            .expect("the answering end is happy");

        assert!(
            left.list_all_keys().await.unwrap_or_default().is_empty()
                && right.list_all_keys().await.unwrap_or_default().is_empty(),
            "nothing was stored"
        );
    }

    #[tokio::test]
    async fn what_arrives_is_written_the_receivers_own_way() {
        let left = store("own-way-left");
        let right = store("own-way-right");

        // Text, which the right end writes as chunks where the left end holds it whole.
        let mut text = String::new();
        for line in 0..2_000 {
            let _ = writeln!(text, "line {line}");
        }
        let content = text.into_bytes();
        let key = writes(&left, &content).await;

        assert!(
            left.read_manifest(&key)
                .await
                .expect("the manifest is read")
                .is_none(),
            "the left end holds it whole"
        );

        let (mut mine, mut theirs) = duplex(64 * 1024);
        let answering_with = right.clone();
        let answering = tokio::spawn(async move { respond(&answering_with, &mut theirs).await });

        initiate(&left, &mut mine, &[key])
            .await
            .expect("the transfer is made");
        answering
            .await
            .expect("the answering end finishes")
            .expect("the answering end is happy");

        assert!(
            right
                .read_manifest(&key)
                .await
                .expect("the manifest is read")
                .is_some(),
            "the right end wrote it its own way"
        );
        assert_eq!(
            right.content(&key).await.ok(),
            Some(Some(content)),
            "what arrived is the content that was sent"
        );
    }

    #[test]
    fn a_key_list_that_does_not_read_is_refused() {
        // Keys are written as a count and then a record each, so a count that does not fit the
        // bytes after it is not one to read past.
        assert!(matches!(
            super::keys_from_bytes(&1_u64.to_be_bytes()),
            Err(super::Wire::Malformed)
        ));
        assert!(matches!(
            super::keys_from_bytes(b"too short by far"),
            Err(super::Wire::Malformed)
        ));
    }
}
