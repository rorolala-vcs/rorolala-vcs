//! Identity primitives: what a key is, and what it proves.
//!
//! A key here is an **identity**, not a session secret. The public half is what a peer is
//! known by — and, since a name is only a local label, the [fingerprint](PublicKey::fingerprint)
//! is what says whether two identities are the same. The private half proves the
//! correspondence by [signing](SigningKey::sign) a transcript that both sides derive the
//! same way.

use std::fmt;

use ed25519_dalek::{
    Signature as Ed25519Signature, Signer as _, SigningKey as Ed25519SigningKey, Verifier as _,
    VerifyingKey as Ed25519VerifyingKey,
};
use sha2::{Digest as _, Sha256};

use crate::Error;

/// The signature algorithm an identity key is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAlgorithm {
    /// Ed25519 (RFC 8032): 32-byte public keys, 64-byte signatures, no randomness.
    Ed25519,
}

impl KeyAlgorithm {
    /// The name this algorithm is written down by, on the wire and in a fingerprint.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
        }
    }

    /// The algorithm a name refers to, if this build knows one by it.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ed25519" => Some(Self::Ed25519),
            _ => None,
        }
    }

    /// How long a public key of this algorithm is, in bytes.
    #[must_use]
    pub const fn public_len(self) -> usize {
        match self {
            Self::Ed25519 => 32,
        }
    }

    /// How long a private key of this algorithm is, in bytes.
    #[must_use]
    pub const fn secret_len(self) -> usize {
        match self {
            Self::Ed25519 => 32,
        }
    }

    /// How long a signature of this algorithm is, in bytes.
    #[must_use]
    pub const fn signature_len(self) -> usize {
        match self {
            Self::Ed25519 => 64,
        }
    }
}

/// What an identity is keyed by: a hash of its algorithm and its public key, and nothing
/// else.
///
/// A name is a local label — two peers can each call the same key something different —
/// so the fingerprint is what a member table is looked up by, and what makes "I am Alice"
/// mean the key that is Alice rather than a word a peer chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// The fingerprint of `algorithm`'s key `bytes`.
    #[must_use]
    pub fn of(algorithm: KeyAlgorithm, bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(algorithm.name().as_bytes());
        // A separator is folded in, so the name and the key can never run together into
        // one preimage.
        hasher.update([0]);
        hasher.update(bytes);

        Self(hasher.finalize().into())
    }

    /// The fingerprint's bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }

        Ok(())
    }
}

/// A signature, and the algorithm that made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// The algorithm the signature was made with.
    algorithm: KeyAlgorithm,
    /// The signature's bytes.
    bytes: Vec<u8>,
}

impl Signature {
    /// Wraps `bytes` as a signature of `algorithm`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if `bytes` is not the length the algorithm states.
    pub fn from_bytes(algorithm: KeyAlgorithm, bytes: impl Into<Vec<u8>>) -> Result<Self, Error> {
        let bytes = bytes.into();
        if bytes.len() != algorithm.signature_len() {
            return Err(Error::Malformed);
        }

        Ok(Self { algorithm, bytes })
    }

    /// The algorithm this signature was made with.
    #[must_use]
    pub const fn algorithm(&self) -> KeyAlgorithm {
        self.algorithm
    }

    /// The signature's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The public half of an identity: what a peer proves itself *by*.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicKey {
    /// The algorithm the key is for.
    algorithm: KeyAlgorithm,
    /// The key's bytes.
    bytes: Vec<u8>,
}

impl PublicKey {
    /// Wraps `bytes` as a public key of `algorithm`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if `bytes` is not the length the algorithm states.
    pub fn from_bytes(algorithm: KeyAlgorithm, bytes: impl Into<Vec<u8>>) -> Result<Self, Error> {
        let bytes = bytes.into();
        if bytes.len() != algorithm.public_len() {
            return Err(Error::Malformed);
        }

        Ok(Self { algorithm, bytes })
    }

    /// The algorithm this key is for.
    #[must_use]
    pub const fn algorithm(&self) -> KeyAlgorithm {
        self.algorithm
    }

    /// The key's bytes, as they travel on the wire.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// What this identity is known by, wherever a name would be ambiguous.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(self.algorithm, &self.bytes)
    }

    /// Checks that `signature` is `message`, signed by this key's private half.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadSignature`] if the signature is for another algorithm or does
    /// not verify, and [`Error::Malformed`] if the key cannot be read as its algorithm.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), Error> {
        if self.algorithm != signature.algorithm {
            return Err(Error::BadSignature);
        }

        match self.algorithm {
            KeyAlgorithm::Ed25519 => {
                let key = verifying_key(&self.bytes)?;
                let signature = ed25519_signature(&signature.bytes)?;
                key.verify(message, &signature)
                    .map_err(|_| Error::BadSignature)
            }
        }
    }
}

/// The private half of an identity: what proves it.
#[derive(Clone)]
pub struct SigningKey {
    /// The algorithm the key is for.
    algorithm: KeyAlgorithm,
    /// The key, in the shape its algorithm signs with.
    signing: Ed25519SigningKey,
}

impl SigningKey {
    /// Reads `bytes` as a private key of `algorithm`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if `bytes` is not the length the algorithm states, or
    /// is not a key that algorithm accepts.
    pub fn from_bytes(algorithm: KeyAlgorithm, bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        match algorithm {
            KeyAlgorithm::Ed25519 => {
                let seed: &[u8; 32] = bytes.as_ref().try_into().map_err(|_| Error::Malformed)?;
                Ok(Self {
                    algorithm,
                    signing: Ed25519SigningKey::from_bytes(seed),
                })
            }
        }
    }

    /// The algorithm this key is for.
    #[must_use]
    pub const fn algorithm(&self) -> KeyAlgorithm {
        self.algorithm
    }

    /// The public half, which is what a peer checks a signature against.
    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        match self.algorithm {
            KeyAlgorithm::Ed25519 => PublicKey {
                algorithm: self.algorithm,
                bytes: self.signing.verifying_key().to_bytes().to_vec(),
            },
        }
    }

    /// Signs `message`, proving this identity to whoever holds [`Self::public_key`].
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> Signature {
        match self.algorithm {
            KeyAlgorithm::Ed25519 => Signature {
                algorithm: self.algorithm,
                bytes: self.signing.sign(message).to_bytes().to_vec(),
            },
        }
    }
}

impl fmt::Debug for SigningKey {
    /// Names the algorithm, never the key: a private key should not reach a log through a
    /// `Debug` that looked harmless at the call site.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SigningKey")
            .field("algorithm", &self.algorithm)
            .finish_non_exhaustive()
    }
}

/// Reads `bytes` as an Ed25519 verifying key.
fn verifying_key(bytes: &[u8]) -> Result<Ed25519VerifyingKey, Error> {
    let key: &[u8; 32] = bytes.try_into().map_err(|_| Error::Malformed)?;
    Ed25519VerifyingKey::from_bytes(key).map_err(|_| Error::Malformed)
}

/// Reads `bytes` as an Ed25519 signature.
fn ed25519_signature(bytes: &[u8]) -> Result<Ed25519Signature, Error> {
    let signature: &[u8; 64] = bytes.try_into().map_err(|_| Error::BadSignature)?;
    Ok(Ed25519Signature::from_bytes(signature))
}

#[cfg(test)]
mod tests {
    use super::{Fingerprint, KeyAlgorithm, PublicKey, Signature, SigningKey};
    use crate::Error;

    /// A key with a given seed, so a test is deterministic.
    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(KeyAlgorithm::Ed25519, [seed; 32]).unwrap()
    }

    #[test]
    fn a_signature_verifies_under_the_key_it_was_made_by() {
        let signing = key(1);
        let message = b"rorolala";

        let signature = signing.sign(message);

        assert!(signing.public_key().verify(message, &signature).is_ok());
    }

    #[test]
    fn another_key_does_not_verify_the_signature() {
        let signature = key(1).sign(b"rorolala");

        assert!(matches!(
            key(2).public_key().verify(b"rorolala", &signature),
            Err(Error::BadSignature)
        ));
    }

    #[test]
    fn a_changed_message_does_not_verify() {
        let signing = key(1);
        let signature = signing.sign(b"rorolala");

        assert!(matches!(
            signing.public_key().verify(b"rorolala!", &signature),
            Err(Error::BadSignature)
        ));
    }

    #[test]
    fn a_fingerprint_names_the_key_and_its_algorithm() {
        let public = key(1).public_key();

        assert_eq!(public.fingerprint(), key(1).public_key().fingerprint());
        assert_ne!(public.fingerprint(), key(2).public_key().fingerprint());
        assert_eq!(
            public.fingerprint(),
            Fingerprint::of(KeyAlgorithm::Ed25519, public.as_bytes())
        );
    }

    #[test]
    fn a_key_of_the_wrong_length_is_rejected() {
        assert!(matches!(
            PublicKey::from_bytes(KeyAlgorithm::Ed25519, vec![0; 31]),
            Err(Error::Malformed)
        ));
        assert!(matches!(
            Signature::from_bytes(KeyAlgorithm::Ed25519, vec![0; 63]),
            Err(Error::Malformed)
        ));
        assert!(matches!(
            SigningKey::from_bytes(KeyAlgorithm::Ed25519, [0; 31]),
            Err(Error::Malformed)
        ));
    }

    #[test]
    fn a_key_round_trips_through_its_bytes() {
        let public = key(7).public_key();
        let read = PublicKey::from_bytes(public.algorithm(), public.as_bytes()).unwrap();

        assert_eq!(read, public);
    }
}
