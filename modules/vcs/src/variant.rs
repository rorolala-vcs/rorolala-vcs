use rorolala_storage::Blake3Hash;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

use crate::index::{VCSIndexKind, VCSWrite};
use crate::version::Version;
use crate::{ComputeBlake3, Hash, SALT_VRT};

/// The editing state that has not been fixed as a Version
///
/// The smallest unit of version control logic in Rola, and what a Version is built on.
///
/// ## Variant kinds
///
/// A Variant comes in two kinds: a progressive variant and a merge variant.
///
/// 1. Progressive variant
///
/// It is based on one Version and points at one stored entry.
/// The diagram below shows the relation between the variant `R1A` and two Versions:
/// born from `V1`, `R1A` is based on it and produces `V2`.
///
/// ```graph
/// V2
///   \
///    R1A
///   /
/// V1
/// ```
///
/// 2. Merge variant
///
/// It is based on one Version and, besides that, joins in an existing variant,
/// pointing at the stored entry the two produce together.
/// The diagram below shows the relation between the variant `R2A`, the Version `V2`
/// and the joined variant `R1B`: `V2` yields `R1A`, and `R1A` joins the detached
/// `R1B` to give `R2A`.
///
/// ```graph
/// V3
///   \
///    R2A
///   /  \
/// V2    \
///   \    \
///    R1A  R1B
///   /____/
/// V1
/// ```
///
// Deserializing builds the value out of plain fields; the `unsafe` methods the lint sees are the
// FFI conversions `lazyffi` generates beside them, not anything the data goes through.
#[allow(clippy::unsafe_derive_deserialize)]
#[lazyffi(export = RolaVariant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    /// The hash of the stored entry it points at, which is the content itself
    #[serde(with = "crate::hex::hash")]
    storage_hash: Blake3Hash,

    /// The hash of the base Version this variant is based on
    #[serde(with = "crate::hex::hash")]
    base_version: Blake3Hash,

    /// The hash of the variant joined in (some only when merged)
    #[serde(with = "crate::hex::maybe_hash")]
    join: Option<Blake3Hash>,

    /// The hash of the [`Creator`](crate::Creator) that made it
    #[serde(with = "crate::hex::hash")]
    creator: Blake3Hash,

    /// The hash of the [`Message`](crate::Message) that says what was done
    #[serde(with = "crate::hex::hash")]
    message: Blake3Hash,

    /// The number of the Version this variant is based on
    ///
    /// It is derived rather than stored: it is neither written down nor part of the hash, so a
    /// variant read back from an index carries [`UNKNOWN_VERSION`](crate::UNKNOWN_VERSION) until
    /// [`VCSIndex::version_num`](crate::VCSIndex::version_num) works it out.
    #[serde(skip, default = "crate::unknown_version")]
    base_version_num: u64,
}

impl PartialEq for Variant {
    /// Two variants are the same when the fields that are written down are: the number is
    /// derived, so a variant that has not been worked out is the same variant as one that has.
    fn eq(&self, other: &Self) -> bool {
        self.storage_hash == other.storage_hash
            && self.base_version == other.base_version
            && self.join == other.join
            && self.creator == other.creator
            && self.message == other.message
    }
}

impl Eq for Variant {}

impl Variant {
    /// A Variant built directly from its parts, checking nothing
    ///
    /// It takes the hashes and the number on the caller's word: nothing here reads a Version, a
    /// Creator or a Message to see that they are there, or that `version` is the number the chain
    /// would work out. It is for a variant that came out of bytes, and for the making of one
    /// through [`Version::new_variant`] and its kin.
    #[must_use]
    pub const fn new_bare_variant(
        storage_hash: Blake3Hash,
        base_version: Blake3Hash,
        join: Option<Blake3Hash>,
        creator: Blake3Hash,
        message: Blake3Hash,
        version: u64,
    ) -> Self {
        Self {
            storage_hash,
            base_version,
            join,
            creator,
            message,
            base_version_num: version,
        }
    }

    /// The hash of the stored entry this variant points at.
    #[must_use]
    pub const fn storage_hash(&self) -> &Blake3Hash {
        &self.storage_hash
    }

    /// The hash of the Version this variant is based on.
    #[must_use]
    pub const fn base_version(&self) -> &Blake3Hash {
        &self.base_version
    }

    /// The hash of the variant joined in, when this merges one.
    #[must_use]
    pub const fn join(&self) -> Option<&Blake3Hash> {
        self.join.as_ref()
    }

    /// The hash of the Creator that made this variant.
    #[must_use]
    pub const fn creator(&self) -> &Blake3Hash {
        &self.creator
    }

    /// The hash of the Message that says what was done.
    #[must_use]
    pub const fn message(&self) -> &Blake3Hash {
        &self.message
    }

    /// The number of the Version this variant is based on.
    ///
    /// It is [`UNKNOWN_VERSION`](crate::UNKNOWN_VERSION) when the variant was read and has not
    /// been worked out.
    #[must_use]
    pub const fn base_version_num(&self) -> u64 {
        self.base_version_num
    }

    /// The Version this variant fixes, numbered one past the variant's own
    ///
    /// This is the one step that moves a chain forward: every other way of making an object
    /// leaves the number where it was.
    #[must_use]
    pub fn new_version(&self) -> Version {
        Version::new_bare_version(*self.hash().digest(), self.base_version_num.wrapping_add(1))
    }

    /// A merge Variant based on the same Version as `self`, joining in `join`
    ///
    /// Who made it and what was done do not move either: a merge is one more way of making a
    /// variant of the same version, by the same creator and with the same message.
    #[must_use]
    pub fn new_merge_variant(&self, storage_hash: Blake3Hash, join: &Self) -> Self {
        Self {
            storage_hash,
            base_version: self.base_version,
            join: Some(*join.hash().digest()),
            creator: self.creator,
            message: self.message,
            base_version_num: self.base_version_num,
        }
    }
}

impl VCSWrite for Variant {
    fn kind(&self) -> VCSIndexKind {
        VCSIndexKind::Variant
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + 4 * 32 + 1 + 32);

        bytes.push(VCSIndexKind::Variant.id());
        bytes.extend_from_slice(&self.storage_hash);
        bytes.extend_from_slice(&self.base_version);
        match &self.join {
            Some(join) => {
                bytes.push(1);
                bytes.extend_from_slice(join);
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(&self.creator);
        bytes.extend_from_slice(&self.message);

        bytes
    }

    fn hash(&self) -> Hash {
        Hash::new(self.compute_blake3())
    }
}

impl ComputeBlake3 for Variant {
    fn compute_blake3(&self) -> Blake3Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SALT_VRT.as_bytes());
        hasher.update(&self.storage_hash);
        hasher.update(&self.base_version);
        if let Some(join) = &self.join {
            hasher.update(join);
        }
        hasher.update(&self.creator);
        hasher.update(&self.message);
        hasher.finalize().into()
    }
}

#[lazyffi]
impl Variant {
    /// The kind of this variant
    ///
    /// Whether it is progressive or a merge is read from whether it joins in another
    /// variant (`join`).
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_vcs::{Variant, VariantType};
    ///
    /// // A progressive variant: nothing joined in.
    /// let progressive =
    ///     Variant::new_bare_variant([0u8; 32], [1u8; 32], None, [3u8; 32], [4u8; 32], 0);
    /// assert_eq!(progressive.type_of(), VariantType::Progressive);
    ///
    /// // A merge variant: another variant joined in.
    /// let merge =
    ///     Variant::new_bare_variant([0u8; 32], [1u8; 32], Some([2u8; 32]), [3u8; 32], [4u8; 32], 0);
    /// assert_eq!(merge.type_of(), VariantType::Merge);
    /// ```
    #[lazyffi(export = get_rola_variant_type)]
    #[must_use]
    pub fn type_of(&self) -> VariantType {
        self.into()
    }
}

/// The kind of a Variant
///
/// Tells a progressive variant and a merge variant apart.
#[lazyffi(export = RolaVariantType)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantType {
    /// Progressive: based on one Version and pointing at one stored entry
    Progressive,

    /// Merge: based on one Version and joining in an existing variant
    Merge,
}

impl From<&Variant> for VariantType {
    fn from(variant: &Variant) -> Self {
        if variant.join.is_some() {
            Self::Merge
        } else {
            Self::Progressive
        }
    }
}
