//! The encrypted channel: a socket whose bytes are a session's plaintext.
//!
//! A [`SecureStream`] is the only thing the layer above sees, and it is a socket in every
//! way that matters: it implements [`AsyncRead`] and [`AsyncWrite`], so `read_exact`,
//! `write_all`, `split`, the `tokio_util` codecs and anything else generic over a stream
//! work on it unchanged. The record layer underneath — the length-prefixed, sealed
//! records, the counters, the rekeying — is invisible.
//!
//! One thing is not left to a caller's discipline: a read also puts out whatever a write
//! left behind, so writing a request and then waiting for the reply cannot stall on a
//! stream that stopped accepting the write part-way through it. A socket behaves that way
//! because the kernel drains it; this has to say so itself.
//!
//! The handshake is *signed* Diffie–Hellman, not Noise's implicitly-authenticated one,
//! because an identity here is a **signature** key: a peer proves itself by signing the
//! transcript, which is exactly what [`PublicKey::verify`] checks. Which identity that is,
//! is the key's [fingerprint](crate::PublicKey::fingerprint) — a name is only a local
//! label, so a peer can never claim an identity other than the key it holds.
//!
//! The exchange itself is hybrid: X25519 for forward secrecy *now*, and ML-KEM-768 for
//! forward secrecy against a key that is only recovered *later* — the "store now, decrypt
//! later" that a session recorded today would otherwise be open to.

use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use chacha20poly1305::aead::{Aead as _, KeyInit as _};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use ml_kem::{
    Decapsulate as _, Encapsulate as _, EncapsulationKey768, Kem as _, Key as KemKey,
    KeyExport as _, MlKem768,
};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, ReadBuf};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

use crate::Error;
use crate::key::{KeyAlgorithm, PublicKey, Signature, SigningKey};

/// The largest plaintext a single record carries.
pub const MAX_PLAINTEXT: usize = 16 * 1024;

/// The bytes an authentication tag adds to a record.
const TAG_LEN: usize = 16;

/// The largest record a session will read: a full plaintext and its tag.
const MAX_CIPHERTEXT: usize = MAX_PLAINTEXT + TAG_LEN;

/// The largest handshake message a session will read.
const MAX_HANDSHAKE: usize = 8 * 1024;

/// How many records one key seals before it is ratcheted to the next.
///
/// Both sides count their own records and reach this at the same point, so they stay in
/// step without saying anything about it.
const REKEY_EVERY: u64 = 1 << 32;

/// The length of an ML-KEM-768 encapsulation key, as FIPS 203 states it.
///
/// Checked before one is read off the wire, since reading it is a cast that assumes the
/// length; a test holds the crate to this number.
const KEM_ENCAPSULATION_LEN: usize = 1184;

/// The length of an ML-KEM-768 ciphertext, as FIPS 203 states it.
const KEM_CIPHERTEXT_LEN: usize = 1088;

/// The suite this build offers: Ed25519 identities over a hybrid X25519 + ML-KEM-768
/// exchange.
const SUITE: u8 = 1;

/// What every derivation is labelled with, so one version's keys are not another's.
const LABEL: &[u8] = b"rorolala/1";

/// A stream whose bytes are the plaintext of an encrypted session.
///
/// Handshaking is not something a caller does: [`SecureStream::connect`] and
/// [`SecureStream::accept`] finish it before returning, so there is no window in which a
/// stream exists that is not yet encrypted.
pub struct SecureStream<S> {
    /// The stream the records travel over.
    inner: S,
    /// What seals, and the key it is ratcheted from.
    sealing: ChaCha20Poly1305,
    sealing_key: [u8; 32],
    sealing_counter: u64,
    /// What opens, and the key it is ratcheted from.
    opening: ChaCha20Poly1305,
    opening_key: [u8; 32],
    opening_counter: u64,
    /// The identity the peer proved during the handshake.
    peer: PublicKey,
    /// Plaintext decrypted but not yet read out.
    plain: VecDeque<u8>,
    /// Ciphertext read but not yet a whole record.
    incoming: Vec<u8>,
    /// Ciphertext produced but not yet written out.
    outgoing: Vec<u8>,
    /// How much of [`Self::outgoing`] has reached the stream.
    outgoing_len: usize,
    /// How many records one key seals before it is stepped to the next.
    rekey_every: u64,
}

impl<S> SecureStream<S> {
    /// Builds a session out of the keys a handshake agreed on.
    fn new(inner: S, sealing_key: [u8; 32], opening_key: [u8; 32], peer: PublicKey) -> Self {
        Self {
            inner,
            sealing: cipher(&sealing_key),
            sealing_key,
            sealing_counter: 0,
            opening: cipher(&opening_key),
            opening_key,
            opening_counter: 0,
            peer,
            plain: VecDeque::new(),
            incoming: Vec::new(),
            outgoing: Vec::new(),
            outgoing_len: 0,
            rekey_every: REKEY_EVERY,
        }
    }

    /// The identity the peer proved during the handshake.
    ///
    /// This is a key, never a name: what a peer is *called* is resolved by the side that
    /// keeps the member list, from this fingerprint.
    #[must_use]
    pub const fn peer(&self) -> &PublicKey {
        &self.peer
    }

    /// The stream underneath, once the session is no longer wanted.
    ///
    /// Anything still buffered for writing goes with the session, so flush first if it
    /// matters whether the peer saw it.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.inner
    }

    /// Steps the key every `every` records instead of the usual count, so a test can cross
    /// a boundary without sending two to the power of thirty-two of them.
    #[cfg(test)]
    const fn rekey_every(mut self, every: u64) -> Self {
        self.rekey_every = every;

        self
    }

    /// Takes a whole record out of what has been read, once there is one.
    fn take_record(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.incoming.len() < 4 {
            return Ok(None);
        }

        let header: [u8; 4] = self.incoming[..4]
            .try_into()
            .map_err(|_| record_error("a record header was short"))?;
        let length = usize::try_from(u32::from_be_bytes(header))
            .map_err(|_| record_error("a record was longer than a session reads"))?;

        // A record shorter than a tag can never be one, and one longer than a plaintext
        // cannot have been written by a session. A peer that says "a gigabyte" must not
        // make one be allocated, either.
        if !(TAG_LEN..=MAX_CIPHERTEXT).contains(&length) {
            return Err(record_error("a record was not a length a session writes"));
        }
        if self.incoming.len() < 4 + length {
            return Ok(None);
        }

        let record = self.incoming[4..4 + length].to_vec();
        self.incoming.drain(..4 + length);

        Ok(Some(record))
    }
}

impl<S: AsyncWrite + Unpin> SecureStream<S> {
    /// Writes out whatever sealed bytes are still waiting.
    fn poll_drain(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.outgoing_len < self.outgoing.len() {
            let written = ready!(
                Pin::new(&mut self.inner).poll_write(cx, &self.outgoing[self.outgoing_len..])
            )?;
            if written == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
            self.outgoing_len += written;
        }

        self.outgoing.clear();
        self.outgoing_len = 0;

        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SecureStream<S> {
    /// Connects to a peer whose identity is already known, and proves our own.
    ///
    /// `expected` is the identity the peer must turn out to hold — a key that was pinned,
    /// or looked up. A peer that proves anything else is rejected before a byte of
    /// session data is exchanged.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedIdentity`] if the peer proves a key other than
    /// `expected`, [`Error::BadSignature`] if its proof does not verify,
    /// [`Error::Handshake`] if it does not follow the protocol, and [`Error::Io`] if the
    /// stream fails.
    pub async fn connect(
        mut inner: S,
        me: &SigningKey,
        expected: &PublicKey,
    ) -> Result<Self, Error> {
        let ephemeral = EphemeralSecret::random();
        let our_ephemeral = X25519PublicKey::from(&ephemeral);
        let identity = me.public_key();
        // The half only we can decapsulate, so the session key takes both an exchange and
        // a lattice problem to recover.
        let (decapsulation, encapsulation) = MlKem768::generate_keypair();
        let encapsulation = encapsulation.to_bytes();

        let mut handshake = Handshake::new(SUITE)?;
        let offer = Offer::encode(&our_ephemeral, &identity, encapsulation.as_ref())?;
        handshake.offer(
            our_ephemeral.as_bytes(),
            identity.as_bytes(),
            encapsulation.as_ref(),
        )?;
        write_message(&mut inner, &offer).await?;

        let message = read_message(&mut inner).await?;
        let answer = Answer::decode(&message)?;
        let peer_ephemeral = answer.ephemeral()?;
        let peer = answer.identity()?;
        let ciphertext = answer.ciphertext()?;

        // The expected identity is checked before anything it signed is believed.
        if peer.fingerprint() != expected.fingerprint() {
            return Err(Error::UnexpectedIdentity);
        }

        let answering = handshake.answer(peer_ephemeral.as_bytes(), peer.as_bytes(), ciphertext)?;
        let proof = answer.proof()?;
        peer.verify(&answering, &proof)?;

        let shared = ephemeral.diffie_hellman(&peer_ephemeral);
        // A peer that offers a low-order point would drive the exchange's half of the
        // secret to zero, leaving only the KEM's; refuse rather than quietly weaken it.
        if !shared.was_contributory() {
            return Err(Error::Handshake);
        }

        let encapsulated = decapsulation
            .decapsulate_slice(ciphertext)
            .map_err(|_| Error::Handshake)?;
        let handshake_keys = Schedule::extract(
            &answering,
            &hybrid_shared(shared.as_bytes(), encapsulated.as_ref()),
        );
        let to_server = handshake_keys.expand("hs to server");

        let proving = handshake.server_proof(proof.as_bytes())?;
        let ours = me.sign(&proving);
        write_message(&mut inner, &seal_once(&to_server, ours.as_bytes())?).await?;

        let finished = handshake.client_proof(ours.as_bytes())?;
        let session = Schedule::extract(&finished, handshake_keys.bytes());

        Ok(Self::new(
            inner,
            session.expand("app to server"),
            session.expand("app to client"),
            peer,
        ))
    }

    /// Accepts a peer, proving our own identity and confirming theirs.
    ///
    /// Whoever connects is taken at their key: [`SecureStream::peer`] is what they proved,
    /// and whether that key is a member of anything is the caller's to decide.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadSignature`] if the peer does not prove the key it presented,
    /// [`Error::Handshake`] if it does not follow the protocol, and [`Error::Io`] if the
    /// stream fails.
    pub async fn accept(mut inner: S, me: &SigningKey) -> Result<Self, Error> {
        let message = read_message(&mut inner).await?;
        let offer = Offer::decode(&message)?;
        let peer_ephemeral = offer.ephemeral()?;
        let peer = offer.identity()?;
        let peer_encapsulation = offer.encapsulation()?;

        let mut handshake = Handshake::new(SUITE)?;
        handshake.offer(
            peer_ephemeral.as_bytes(),
            peer.as_bytes(),
            offer.encapsulation,
        )?;

        let ephemeral = EphemeralSecret::random();
        let our_ephemeral = X25519PublicKey::from(&ephemeral);
        let identity = me.public_key();
        // The client's key is what the session's post-quantum half is encapsulated to.
        let (ciphertext, encapsulated) = peer_encapsulation.encapsulate();

        let answering = handshake.answer(
            our_ephemeral.as_bytes(),
            identity.as_bytes(),
            ciphertext.as_ref(),
        )?;
        let proof = me.sign(&answering);
        let answer = Answer::encode(&our_ephemeral, &identity, ciphertext.as_ref(), &proof)?;
        write_message(&mut inner, &answer).await?;

        let shared = ephemeral.diffie_hellman(&peer_ephemeral);
        // As on the client side: an exchange that contributes nothing is not one.
        if !shared.was_contributory() {
            return Err(Error::Handshake);
        }

        let handshake_keys = Schedule::extract(
            &answering,
            &hybrid_shared(shared.as_bytes(), encapsulated.as_ref()),
        );
        let to_server = handshake_keys.expand("hs to server");

        let proving = handshake.server_proof(proof.as_bytes())?;
        let message = read_message(&mut inner).await?;
        let opened = open_once(&to_server, &message)?;
        let ours = Signature::from_bytes(KeyAlgorithm::Ed25519, opened)?;
        peer.verify(&proving, &ours)?;

        let finished = handshake.client_proof(ours.as_bytes())?;
        let session = Schedule::extract(&finished, handshake_keys.bytes());

        Ok(Self::new(
            inner,
            session.expand("app to client"),
            session.expand("app to server"),
            peer,
        ))
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for SecureStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let me = self.get_mut();

        // A socket puts what it has queued on the wire as it goes, and this is what keeps
        // "write a request, then read the reply" from stalling: if the stream stopped
        // accepting writes part-way through a record, waiting for the reply is itself
        // progress on it.
        if let Poll::Ready(Err(error)) = me.poll_drain(cx) {
            return Poll::Ready(Err(error));
        }

        loop {
            // Plaintext already opened and not yet handed out.
            if !me.plain.is_empty() {
                let take = buf.remaining().min(me.plain.len());
                let chunk: Vec<u8> = me.plain.drain(..take).collect();
                buf.put_slice(&chunk);

                return Poll::Ready(Ok(()));
            }

            // A whole record, opened into more plaintext.
            if let Some(record) = me.take_record()? {
                let plaintext = me
                    .opening
                    .decrypt(&nonce(me.opening_counter), record.as_slice())
                    .map_err(|_| record_error("a record did not open"))?;

                me.opening_counter += 1;
                if me.opening_counter.is_multiple_of(me.rekey_every) {
                    me.opening_key = ratchet(&me.opening_key);
                    me.opening = cipher(&me.opening_key);
                }

                me.plain.extend(plaintext);
                continue;
            }

            // Neither, so more has to come off the stream.
            let mut scratch = [0u8; MAX_PLAINTEXT];
            let mut read = ReadBuf::new(&mut scratch);
            match Pin::new(&mut me.inner).poll_read(cx, &mut read) {
                // Nothing read and nothing left is the end of the stream — unless half a
                // record is already behind it, which is a stream that was cut short.
                Poll::Ready(Ok(())) if read.filled().is_empty() => {
                    return if me.incoming.is_empty() {
                        Poll::Ready(Ok(()))
                    } else {
                        Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()))
                    };
                }
                Poll::Ready(Ok(())) => me.incoming.extend_from_slice(read.filled()),
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for SecureStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let me = self.get_mut();

        // Whatever the last call left unwritten goes out first: a caller that keeps
        // writing into a peer that is not reading must feel the backpressure, not a
        // buffer growing without end.
        if !me.outgoing.is_empty() {
            ready!(me.poll_drain(cx))?;
        }

        if data.is_empty() {
            return Poll::Ready(Ok(0));
        }

        let take = data.len().min(MAX_PLAINTEXT);
        let sealed = me
            .sealing
            .encrypt(&nonce(me.sealing_counter), &data[..take])
            .map_err(|_| io::Error::other("a record could not be sealed"))?;
        let length = u32::try_from(sealed.len())
            .map_err(|_| io::Error::other("a record was longer than a session writes"))?;

        me.outgoing.extend_from_slice(&length.to_be_bytes());
        me.outgoing.extend_from_slice(&sealed);
        me.sealing_counter += 1;
        if me.sealing_counter.is_multiple_of(me.rekey_every) {
            me.sealing_key = ratchet(&me.sealing_key);
            me.sealing = cipher(&me.sealing_key);
        }

        // A `Pending` here is fine — the bytes are buffered, and a later write, flush or
        // read will drain them. A hard error is not: reporting it as a successful write
        // would lose it, since a caller may never write or flush again.
        if let Poll::Ready(Err(error)) = me.poll_drain(cx) {
            return Poll::Ready(Err(error));
        }

        Poll::Ready(Ok(take))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.get_mut();
        ready!(me.poll_drain(cx))?;

        Pin::new(&mut me.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.get_mut();
        ready!(me.poll_drain(cx))?;
        ready!(Pin::new(&mut me.inner).poll_flush(cx))?;

        Pin::new(&mut me.inner).poll_shutdown(cx)
    }
}

/// The error a record that cannot be read is reported as.
///
/// The stream traits can only report `io::Error`, so which of the crate's own variants a
/// bad record would have been is not something a caller of `AsyncRead` can see; what it
/// can see is the kind, which says the bytes were not what they claimed to be.
fn record_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// The secret the session keys are extracted from: the exchange's and the KEM's, in a
/// fixed order.
fn hybrid_shared(exchange: &[u8; 32], encapsulation: &[u8]) -> Vec<u8> {
    let mut secret = Vec::with_capacity(exchange.len() + encapsulation.len());
    secret.extend_from_slice(exchange);
    secret.extend_from_slice(encapsulation);

    secret
}

/// The cipher a 32-byte key names.
fn cipher(key: &[u8; 32]) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(key))
}

/// The nonce a record at `counter` is sealed or opened with.
///
/// A counter that only ever goes up is what keeps a nonce from repeating, which is the
/// one way an AEAD's guarantees are lost.
fn nonce(counter: u64) -> Nonce {
    let mut raw = [0u8; 12];
    raw[4..].copy_from_slice(&counter.to_be_bytes());

    Nonce::from(raw)
}

/// Advances a key by one step, for the session that has spent it.
fn ratchet(key: &[u8; 32]) -> [u8; 32] {
    let mut next = [0u8; 32];
    Hkdf::<Sha256>::from_prk(key)
        .expect("a 32-byte key is a valid pseudo-random key")
        .expand(b"rekey", &mut next)
        .expect("32 bytes always fits an HKDF-SHA256 expansion");

    next
}

/// Seals one message under a key that has never sealed anything else.
fn seal_once(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    cipher(key)
        .encrypt(&nonce(0), plaintext)
        .map_err(|_| Error::Handshake)
}

/// Opens one message sealed by [`seal_once`].
fn open_once(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
    cipher(key)
        .decrypt(&nonce(0), ciphertext)
        .map_err(|_| Error::BadRecord)
}

/// The key schedule: one extraction, then one expansion per key it hands out.
struct Schedule([u8; 32]);

impl Schedule {
    /// Mixes `ikm` with `salt` into the secret every key of this stage is expanded from.
    fn extract(salt: &[u8], ikm: &[u8]) -> Self {
        let (secret, _) = Hkdf::<Sha256>::extract(Some(salt), ikm);

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&secret);

        Self(bytes)
    }

    /// The 32-byte key `label` names.
    fn expand(&self, label: &str) -> [u8; 32] {
        let mut key = [0u8; 32];
        Hkdf::<Sha256>::from_prk(&self.0)
            .expect("a 32-byte secret is a valid pseudo-random key")
            .expand(label.as_bytes(), &mut key)
            .expect("32 bytes always fits an HKDF-SHA256 expansion");

        key
    }

    /// The secret itself, to be mixed into the next stage.
    const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The running hash of the handshake, fed in the same order by both sides.
///
/// Every field goes in — the suite included — and both proofs sign a digest of it, so a
/// peer that alters any of them, or talks one side down to a weaker suite, changes what
/// the signature covers and is rejected.
struct Transcript(Sha256);

impl Transcript {
    /// An empty transcript.
    fn new() -> Self {
        Self(Sha256::new())
    }

    /// Folds one field in, length-prefixed so two fields cannot run together.
    fn push(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let length = u64::try_from(bytes.len()).map_err(|_| Error::Handshake)?;
        self.0.update(length.to_be_bytes());
        self.0.update(bytes);

        Ok(())
    }

    /// What has been folded in so far.
    fn digest(&self) -> [u8; 32] {
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&self.0.clone().finalize());

        digest
    }
}

/// The transcript, wrapped so both sides feed it through the same steps.
struct Handshake {
    /// The running hash.
    transcript: Transcript,
}

impl Handshake {
    /// Starts a transcript for `suite`.
    fn new(suite: u8) -> Result<Self, Error> {
        let mut transcript = Transcript::new();
        transcript.push(LABEL)?;
        transcript.push(&[suite])?;

        Ok(Self { transcript })
    }

    /// Folds in the client's offer.
    fn offer(
        &mut self,
        ephemeral: &[u8],
        identity: &[u8],
        encapsulation: &[u8],
    ) -> Result<(), Error> {
        self.transcript.push(ephemeral)?;
        self.transcript.push(identity)?;
        self.transcript.push(encapsulation)
    }

    /// Folds in the server's answer, and returns what the server signs.
    fn answer(
        &mut self,
        ephemeral: &[u8],
        identity: &[u8],
        ciphertext: &[u8],
    ) -> Result<[u8; 32], Error> {
        self.transcript.push(ephemeral)?;
        self.transcript.push(identity)?;
        self.transcript.push(ciphertext)?;

        Ok(self.transcript.digest())
    }

    /// Folds in the server's proof, and returns what the client signs.
    fn server_proof(&mut self, signature: &[u8]) -> Result<[u8; 32], Error> {
        self.transcript.push(signature)?;

        Ok(self.transcript.digest())
    }

    /// Folds in the client's proof, and returns what the session keys are salted with.
    fn client_proof(&mut self, signature: &[u8]) -> Result<[u8; 32], Error> {
        self.transcript.push(signature)?;

        Ok(self.transcript.digest())
    }
}

/// The client's opening message: who is calling, and where to encapsulate to.
///
/// The suite is read and checked, but not kept: it is a constant this build offers, and
/// the transcript already carries it.
struct Offer<'a> {
    /// The client's ephemeral exchange key.
    ephemeral: &'a [u8],
    /// The identity the client presents.
    identity: &'a [u8],
    /// The encapsulation key the session's post-quantum half is sealed to.
    encapsulation: &'a [u8],
}

impl<'a> Offer<'a> {
    /// Writes the opening message.
    fn encode(
        ephemeral: &X25519PublicKey,
        identity: &PublicKey,
        encapsulation: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let mut message = vec![SUITE];
        put_field(&mut message, ephemeral.as_bytes())?;
        put_field(&mut message, identity.as_bytes())?;
        put_field(&mut message, encapsulation)?;

        Ok(message)
    }

    /// Reads the opening message, rejecting a suite this build does not offer.
    fn decode(message: &'a [u8]) -> Result<Self, Error> {
        let mut reader = Reader::new(message);
        let suite = reader.byte()?;
        if suite != SUITE {
            return Err(Error::Handshake);
        }

        let offer = Self {
            ephemeral: reader.field()?,
            identity: reader.field()?,
            encapsulation: reader.field()?,
        };
        reader.at_end()?;

        Ok(offer)
    }

    /// The client's ephemeral key.
    fn ephemeral(&self) -> Result<X25519PublicKey, Error> {
        exchange_key(self.ephemeral)
    }

    /// The identity the client presented.
    fn identity(&self) -> Result<PublicKey, Error> {
        PublicKey::from_bytes(KeyAlgorithm::Ed25519, self.identity)
    }

    /// The key the session's post-quantum half is encapsulated to.
    fn encapsulation(&self) -> Result<EncapsulationKey768, Error> {
        if self.encapsulation.len() != KEM_ENCAPSULATION_LEN {
            return Err(Error::Handshake);
        }

        let key = KemKey::<EncapsulationKey768>::try_from(self.encapsulation)
            .map_err(|_| Error::Handshake)?;
        EncapsulationKey768::new(&key).map_err(|_| Error::Handshake)
    }
}

/// The server's answering message: its own ephemeral key, its identity, the encapsulated
/// key, and its proof.
struct Answer<'a> {
    /// The server's ephemeral exchange key.
    ephemeral: &'a [u8],
    /// The identity the server presents.
    identity: &'a [u8],
    /// What the session's post-quantum half was encapsulated into.
    ciphertext: &'a [u8],
    /// The server's signature over the transcript.
    proof: &'a [u8],
}

impl<'a> Answer<'a> {
    /// Writes the answering message.
    fn encode(
        ephemeral: &X25519PublicKey,
        identity: &PublicKey,
        ciphertext: &[u8],
        proof: &Signature,
    ) -> Result<Vec<u8>, Error> {
        let mut message = vec![SUITE];
        put_field(&mut message, ephemeral.as_bytes())?;
        put_field(&mut message, identity.as_bytes())?;
        put_field(&mut message, ciphertext)?;
        put_field(&mut message, proof.as_bytes())?;

        Ok(message)
    }

    /// Reads the answering message, rejecting a suite this build does not offer.
    fn decode(message: &'a [u8]) -> Result<Self, Error> {
        let mut reader = Reader::new(message);
        let suite = reader.byte()?;
        if suite != SUITE {
            return Err(Error::Handshake);
        }

        let answer = Self {
            ephemeral: reader.field()?,
            identity: reader.field()?,
            ciphertext: reader.field()?,
            proof: reader.field()?,
        };
        reader.at_end()?;

        Ok(answer)
    }

    /// The server's ephemeral key.
    fn ephemeral(&self) -> Result<X25519PublicKey, Error> {
        exchange_key(self.ephemeral)
    }

    /// The identity the server presented.
    fn identity(&self) -> Result<PublicKey, Error> {
        PublicKey::from_bytes(KeyAlgorithm::Ed25519, self.identity)
    }

    /// What the session's post-quantum half was encapsulated into.
    const fn ciphertext(&self) -> Result<&'a [u8], Error> {
        if self.ciphertext.len() != KEM_CIPHERTEXT_LEN {
            return Err(Error::Handshake);
        }

        Ok(self.ciphertext)
    }

    /// The server's signature over the transcript.
    fn proof(&self) -> Result<Signature, Error> {
        Signature::from_bytes(KeyAlgorithm::Ed25519, self.proof)
    }
}

/// Reads a 32-byte exchange key.
fn exchange_key(bytes: &[u8]) -> Result<X25519PublicKey, Error> {
    let key: [u8; 32] = bytes.try_into().map_err(|_| Error::Handshake)?;

    Ok(X25519PublicKey::from(key))
}

/// Writes a length-prefixed field.
fn put_field(message: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Error> {
    let length = u16::try_from(bytes.len()).map_err(|_| Error::Handshake)?;
    message.extend_from_slice(&length.to_be_bytes());
    message.extend_from_slice(bytes);

    Ok(())
}

/// A cursor over one handshake message.
struct Reader<'a> {
    /// What is being read.
    message: &'a [u8],
    /// How far in it has got.
    at: usize,
}

impl<'a> Reader<'a> {
    /// A cursor at the start of `message`.
    const fn new(message: &'a [u8]) -> Self {
        Self { message, at: 0 }
    }

    /// Reads one byte.
    fn byte(&mut self) -> Result<u8, Error> {
        let byte = *self.message.get(self.at).ok_or(Error::Handshake)?;
        self.at += 1;

        Ok(byte)
    }

    /// Reads `count` bytes.
    fn bytes(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(count).ok_or(Error::Handshake)?;
        let bytes = self.message.get(self.at..end).ok_or(Error::Handshake)?;
        self.at = end;

        Ok(bytes)
    }

    /// Reads a field written by [`put_field`].
    fn field(&mut self) -> Result<&'a [u8], Error> {
        let length = u16::from_be_bytes(self.bytes(2)?.try_into().map_err(|_| Error::Handshake)?);

        self.bytes(usize::from(length))
    }

    /// Rejects a message with anything left over.
    const fn at_end(&self) -> Result<(), Error> {
        if self.at == self.message.len() {
            Ok(())
        } else {
            Err(Error::Handshake)
        }
    }
}

/// Writes one length-prefixed handshake message.
async fn write_message<S: AsyncWrite + Unpin>(stream: &mut S, message: &[u8]) -> Result<(), Error> {
    let length = u32::try_from(message.len()).map_err(|_| Error::Handshake)?;
    stream.write_all(&length.to_be_bytes()).await?;
    stream.write_all(message).await?;
    stream.flush().await?;

    Ok(())
}

/// Reads one length-prefixed handshake message.
async fn read_message<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Vec<u8>, Error> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;

    let length = usize::try_from(u32::from_be_bytes(header)).map_err(|_| Error::Handshake)?;
    if length == 0 || length > MAX_HANDSHAKE {
        return Err(Error::Handshake);
    }

    let mut message = vec![0u8; length];
    stream.read_exact(&mut message).await?;

    Ok(message)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::Duration;

    use ml_kem::{Decapsulate as _, Encapsulate as _, Kem as _, KeyExport as _, MlKem768};
    use tokio::io::{
        AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, ReadBuf, duplex,
    };
    use tokio::time::timeout;

    use super::{KEM_CIPHERTEXT_LEN, KEM_ENCAPSULATION_LEN, MAX_PLAINTEXT, SecureStream};
    use crate::Error;
    use crate::key::{KeyAlgorithm, SigningKey};

    /// A key with a given seed, so a test is deterministic.
    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(KeyAlgorithm::Ed25519, [seed; 32]).unwrap()
    }

    /// Runs a handshake over an in-memory stream and hands back both ends.
    async fn session(
        client: &SigningKey,
        server: &SigningKey,
    ) -> (
        SecureStream<tokio::io::DuplexStream>,
        SecureStream<tokio::io::DuplexStream>,
    ) {
        let (client_io, server_io) = duplex(64 * 1024);
        let expected = server.public_key();
        let running = server.clone();

        let accepting =
            tokio::spawn(async move { SecureStream::accept(server_io, &running).await });

        let client = SecureStream::connect(client_io, client, &expected)
            .await
            .unwrap();
        let server = accepting.await.unwrap().unwrap();

        (client, server)
    }

    #[tokio::test]
    async fn a_session_carries_bytes_both_ways() {
        let client_key = key(1);
        let server_key = key(2);

        let (mut client, mut server) = session(&client_key, &server_key).await;

        client.write_all(b"hello").await.unwrap();
        let mut heard = [0u8; 5];
        server.read_exact(&mut heard).await.unwrap();
        assert_eq!(&heard, b"hello");

        server.write_all(b"world").await.unwrap();
        let mut answer = [0u8; 5];
        client.read_exact(&mut answer).await.unwrap();
        assert_eq!(&answer, b"world");
    }

    #[tokio::test]
    async fn a_payload_spanning_several_records_comes_back_whole() {
        let client_key = key(3);
        let server_key = key(4);

        let (mut client, mut server) = session(&client_key, &server_key).await;

        // Three records' worth, so the read side has to reassemble.
        let length = MAX_PLAINTEXT * 2 + 123;
        let mut payload = Vec::new();
        while payload.len() < length {
            payload.extend_from_slice(b"rorolala ");
        }
        payload.truncate(length);
        let sent = payload.clone();
        let writer = tokio::spawn(async move {
            client.write_all(&sent).await.unwrap();
            client.flush().await.unwrap();
            client
        });

        let mut read = vec![0u8; payload.len()];
        server.read_exact(&mut read).await.unwrap();
        let _client = writer.await.unwrap();

        assert_eq!(read, payload);
    }

    #[tokio::test]
    async fn each_side_sees_the_identity_the_other_proved() {
        let client_key = key(5);
        let server_key = key(6);

        let (client, server) = session(&client_key, &server_key).await;

        assert_eq!(client.peer(), &server_key.public_key());
        assert_eq!(server.peer(), &client_key.public_key());
    }

    #[tokio::test]
    async fn a_peer_that_is_not_the_expected_one_is_rejected() {
        let client_key = key(7);
        let server_key = key(8);
        let someone_else = key(9).public_key();

        let (client_io, server_io) = duplex(64 * 1024);
        let running = server_key.clone();
        let accepting = tokio::spawn(async move {
            // The client gives up before proving itself, so this never finishes.
            let _ = SecureStream::accept(server_io, &running).await;
        });

        let connected = SecureStream::connect(client_io, &client_key, &someone_else).await;

        accepting.abort();
        let _ = accepting.await;

        assert!(matches!(connected, Err(Error::UnexpectedIdentity)));
    }

    #[tokio::test]
    async fn a_record_sealed_with_another_key_does_not_open() {
        let (one, other) = duplex(4 * 1024);
        let peer = key(10).public_key();

        // The sealing key of one side is not the opening key of the other.
        let mut writer = SecureStream::new(one, [1; 32], [2; 32], peer.clone());
        let mut reader = SecureStream::new(other, [3; 32], [4; 32], peer);

        writer.write_all(b"hello").await.unwrap();
        writer.flush().await.unwrap();

        let mut heard = [0u8; 5];
        assert!(reader.read_exact(&mut heard).await.is_err());
    }

    #[test]
    fn the_kem_agrees_with_the_lengths_the_wire_assumes() {
        let (decapsulation, encapsulation) = MlKem768::generate_keypair();

        assert_eq!(encapsulation.to_bytes().len(), KEM_ENCAPSULATION_LEN);

        let (ciphertext, sent) = encapsulation.encapsulate();
        assert_eq!(ciphertext.len(), KEM_CIPHERTEXT_LEN);

        // And the two halves really do agree on the secret, which is the whole point.
        let received = decapsulation
            .decapsulate_slice(ciphertext.as_ref())
            .unwrap();
        assert_eq!(received, sent);
    }

    #[tokio::test]
    async fn a_session_survives_its_keys_being_stepped() {
        let (client_io, server_io) = duplex(64 * 1024);
        let client_key = key(12);
        let server_key = key(13);
        let expected = server_key.public_key();
        let running = server_key.clone();

        // A step every three records, so ten records cross three boundaries.
        let accepting = tokio::spawn(async move {
            SecureStream::accept(server_io, &running)
                .await
                .map(|stream| stream.rekey_every(3))
        });

        let mut client = SecureStream::connect(client_io, &client_key, &expected)
            .await
            .unwrap()
            .rekey_every(3);
        let mut server = accepting.await.unwrap().unwrap();

        for round in 0..10u8 {
            client.write_all(&[round]).await.unwrap();
            client.flush().await.unwrap();
            let mut heard = [0u8; 1];
            server.read_exact(&mut heard).await.unwrap();
            assert_eq!(heard, [round]);

            server.write_all(&[round]).await.unwrap();
            server.flush().await.unwrap();
            let mut answer = [0u8; 1];
            client.read_exact(&mut answer).await.unwrap();
            assert_eq!(answer, [round]);
        }
    }

    #[tokio::test]
    async fn a_request_that_fills_the_send_buffer_does_not_stall() {
        // A stream far smaller than one record, so the write cannot all go out at once.
        let (client_io, server_io) = duplex(256);
        let client_key = key(14);
        let server_key = key(15);
        let expected = server_key.public_key();
        let running = server_key.clone();

        let serving = tokio::spawn(async move {
            let mut server = SecureStream::accept(server_io, &running).await.unwrap();
            let mut request = vec![0u8; 4096];
            server.read_exact(&mut request).await.unwrap();
            server.write_all(b"done").await.unwrap();
            server.flush().await.unwrap();

            request.iter().all(|byte| *byte == 7)
        });

        // One task writes a request and then waits for the reply, which is the shape this
        // stream promises to behave like a socket under.
        let asking = tokio::spawn(async move {
            let mut client = SecureStream::connect(client_io, &client_key, &expected)
                .await
                .unwrap();
            client.write_all(&[7u8; 4096]).await.unwrap();
            let mut reply = [0u8; 4];
            client.read_exact(&mut reply).await.unwrap();

            reply
        });

        let reply = timeout(Duration::from_secs(10), asking)
            .await
            .expect("the request never reached the peer")
            .unwrap();
        assert_eq!(&reply, b"done");

        assert!(
            timeout(Duration::from_secs(10), serving)
                .await
                .unwrap()
                .unwrap()
        );
    }

    #[tokio::test]
    async fn a_record_cut_in_half_is_not_a_clean_end() {
        let (mut wire, other) = duplex(64);
        let mut stream = SecureStream::new(other, [1; 32], [2; 32], key(16).public_key());

        // A header promising twenty bytes, three of them, and then the peer goes away: the
        // reader must not mistake that for an orderly close.
        wire.write_all(&[0, 0, 0, 20, 1, 2, 3]).await.unwrap();
        wire.shutdown().await.unwrap();

        let mut heard = [0u8; 8];
        let error = stream.read_exact(&mut heard).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn a_record_shorter_than_a_tag_is_refused_at_once() {
        let (mut wire, other) = duplex(64);
        let mut stream = SecureStream::new(other, [1; 32], [2; 32], key(19).public_key());

        // Ten bytes can never be a record, so it is refused rather than waited on.
        wire.write_all(&[0, 0, 0, 10, 1, 2, 3]).await.unwrap();
        wire.flush().await.unwrap();

        let mut heard = [0u8; 8];
        let error = stream.read_exact(&mut heard).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn a_write_error_reaches_the_caller() {
        let mut stream = SecureStream::new(Failing, [1; 32], [2; 32], key(17).public_key());

        // The bytes are buffered and then cannot go out; a caller must hear about that
        // rather than be told the write succeeded.
        assert!(stream.write_all(b"hello").await.is_err());
    }

    #[tokio::test]
    async fn a_malformed_offer_is_rejected() {
        // A suite this build does not offer.
        assert!(accept_message(&framed(&[9])).await.is_err());

        // Nothing at all: a zero-length frame, and no frame.
        assert!(accept_message(&framed(&[])).await.is_err());
        assert!(accept_message(&[]).await.is_err());

        // A field that promises more bytes than it carries.
        assert!(accept_message(&framed(&[1, 0, 5, 1, 2])).await.is_err());

        // A well-formed offer with a byte left over.
        let mut trailing = vec![1];
        field(&mut trailing, &[0; 32]);
        field(&mut trailing, &[0; 32]);
        field(&mut trailing, &[0; KEM_ENCAPSULATION_LEN]);
        trailing.push(0);
        assert!(accept_message(&framed(&trailing)).await.is_err());

        // An encapsulation key of a length no session writes.
        let mut short = vec![1];
        field(&mut short, &[0; 32]);
        field(&mut short, &[0; 32]);
        field(&mut short, &[0; 10]);
        assert!(accept_message(&framed(&short)).await.is_err());

        // An oversized handshake is refused before anything is allocated for it.
        let mut oversized = u32::try_from(super::MAX_HANDSHAKE + 1)
            .unwrap()
            .to_be_bytes()
            .to_vec();
        oversized.extend_from_slice(&[1, 0, 0]);
        assert!(accept_message(&oversized).await.is_err());
    }

    /// A stream that fails every write, so a write error can be seen to reach the caller.
    struct Failing;

    impl AsyncRead for Failing {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for Failing {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _data: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Err(io::Error::other("this stream never writes")))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    /// Writes one field the way the handshake frames them.
    fn field(message: &mut Vec<u8>, bytes: &[u8]) {
        super::put_field(message, bytes).unwrap();
    }

    /// Frames a handshake message the way the handshake does.
    fn framed(message: &[u8]) -> Vec<u8> {
        let mut framed = u32::try_from(message.len()).unwrap().to_be_bytes().to_vec();
        framed.extend_from_slice(message);

        framed
    }

    /// Runs `accept` against one raw handshake message.
    async fn accept_message(
        message: &[u8],
    ) -> Result<SecureStream<tokio::io::DuplexStream>, Error> {
        let (client_io, server_io) = duplex(8 * 1024);
        let me = key(18);
        let sending = message.to_vec();

        let writing = tokio::spawn(async move {
            let mut client = client_io;
            let _ = client.write_all(&sending).await;
            let _ = client.shutdown().await;
        });

        let accepted = SecureStream::accept(server_io, &me).await;
        let _ = writing.await;

        accepted
    }
}
