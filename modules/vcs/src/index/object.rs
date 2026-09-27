//! What an index object is written down as.
//!
//! An object crosses into the store as bytes and nothing else — the store keeps content under a
//! key and knows nothing of variants or versions — so this is where the two kinds are told apart
//! and put back together. The bytes lead with the kind, so a reader learns which of the two it
//! holds before it reads anything else.

use rorolala_storage::Blake3Hash;

use crate::error::VCSIndexError;
use crate::{Creator, Hash, Message, UNKNOWN_VERSION, Variant, Version};

/// The kind of an index object, as it is written down in the bytes of one.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VCSIndexKind {
    /// A Variant.
    Variant = 0,
    /// A Version.
    Version = 1,
    /// A Creator.
    Creator = 2,
    /// A Message.
    Message = 3,
}

impl VCSIndexKind {
    /// The number this kind leads an object's bytes with.
    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// The kind written down by `number`, if this build knows one.
    #[must_use]
    pub const fn from_id(number: u8) -> Option<Self> {
        match number {
            0 => Some(Self::Variant),
            1 => Some(Self::Version),
            2 => Some(Self::Creator),
            3 => Some(Self::Message),
            _ => None,
        }
    }
}

/// An index object that can be written into the index.
///
/// What the store needs of an object is its kind, the bytes it is kept as, and the key it is
/// found by; everything else about it is the object's own.
pub trait VCSWrite {
    /// Which of the two kinds this object is.
    fn kind(&self) -> VCSIndexKind;

    /// The bytes this object is kept as, leading with its kind.
    fn encode(&self) -> Vec<u8>;

    /// The key this object is stored under.
    fn hash(&self) -> Hash;
}

impl VCSWrite for crate::VCSIndexObject {
    fn kind(&self) -> VCSIndexKind {
        match self {
            Self::Variant(variant) => variant.kind(),
            Self::Version(version) => version.kind(),
            Self::Creator(creator) => creator.kind(),
            Self::Message(message) => message.kind(),
        }
    }

    fn encode(&self) -> Vec<u8> {
        match self {
            Self::Variant(variant) => variant.encode(),
            Self::Version(version) => version.encode(),
            Self::Creator(creator) => creator.encode(),
            Self::Message(message) => message.encode(),
        }
    }

    fn hash(&self) -> Hash {
        match self {
            Self::Variant(variant) => variant.hash(),
            Self::Version(version) => version.hash(),
            Self::Creator(creator) => creator.hash(),
            Self::Message(message) => message.hash(),
        }
    }
}

impl crate::VCSIndexObject {
    /// The object the bytes `plain` hold, whatever kind they lead with.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Malformed`] if the bytes lead with a kind this build does not
    /// know, or do not hold the fields that kind is made of.
    pub fn decode(plain: &[u8]) -> Result<Self, VCSIndexError> {
        let (&tag, rest) = plain.split_first().ok_or(VCSIndexError::Malformed)?;

        match VCSIndexKind::from_id(tag).ok_or(VCSIndexError::Malformed)? {
            VCSIndexKind::Variant => decode_variant(rest).map(Self::Variant),
            VCSIndexKind::Version => decode_version(rest).map(Self::Version),
            VCSIndexKind::Creator => decode_creator(rest).map(Self::Creator),
            VCSIndexKind::Message => decode_message(rest).map(Self::Message),
        }
    }
}

/// A `Variant` from the bytes after its kind: the hashes it points at, and who said what.
fn decode_variant(rest: &[u8]) -> Result<Variant, VCSIndexError> {
    let (storage_hash, rest) = take_hash(rest)?;
    let (base_version, rest) = take_hash(rest)?;
    let (flag, rest) = rest.split_first().ok_or(VCSIndexError::Malformed)?;

    let (join, rest) = match *flag {
        0 => (None, rest),
        1 => {
            let (join, rest) = take_hash(rest)?;
            (Some(join), rest)
        }
        _ => return Err(VCSIndexError::Malformed),
    };

    let (creator, rest) = take_hash(rest)?;
    let (message, rest) = take_hash(rest)?;

    rest.is_empty()
        .then_some(Variant::new_bare_variant(
            storage_hash,
            base_version,
            join,
            creator,
            message,
            UNKNOWN_VERSION,
        ))
        .ok_or(VCSIndexError::Malformed)
}

/// A `Version` from the bytes after its kind: the variant it points at.
fn decode_version(rest: &[u8]) -> Result<Version, VCSIndexError> {
    let (variant, rest) = take_hash(rest)?;

    rest.is_empty()
        .then(|| Version::new_bare_version(variant, UNKNOWN_VERSION))
        .ok_or(VCSIndexError::Malformed)
}

/// A `Creator` from the bytes after its kind: the name, as UTF-8.
fn decode_creator(rest: &[u8]) -> Result<Creator, VCSIndexError> {
    Ok(Creator::try_from(rest.to_vec())?)
}

/// A `Message` from the bytes after its kind: the text, as UTF-8.
fn decode_message(rest: &[u8]) -> Result<Message, VCSIndexError> {
    Ok(Message::try_from(rest.to_vec())?)
}

/// The 32 bytes `rest` starts with, and what follows them.
fn take_hash(rest: &[u8]) -> Result<(Blake3Hash, &[u8]), VCSIndexError> {
    let (head, rest) = rest.split_at_checked(32).ok_or(VCSIndexError::Malformed)?;
    let digest: Blake3Hash = head.try_into().map_err(|_| VCSIndexError::Malformed)?;

    Ok((digest, rest))
}
