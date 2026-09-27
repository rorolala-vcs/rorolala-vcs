use rorolala_storage::Blake3Hash;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

use crate::index::{VCSIndexKind, VCSWrite};
use crate::variant::Variant;
use crate::{ComputeBlake3, Hash, ROOT_VERSION, SALT_VER};

/// A Version: an editing state that has been fixed
///
/// The core datum of version control in Rola, confirming the direction of the version chain
/// through the variant it points at. Who made the change and what was done belong to the variant,
/// not to the version: fixing one does not change either.
#[allow(clippy::unsafe_derive_deserialize)]
#[allow(clippy::struct_field_names)]
#[lazyffi(export = RolaVersion)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    /// The hash of the variant it points at
    #[serde(with = "crate::hex::hash")]
    variant: Blake3Hash,

    /// The number of this version in its chain
    ///
    /// Derived rather than stored: it is neither written down nor part of the hash, so a version
    /// read back from an index carries [`UNKNOWN_VERSION`](crate::UNKNOWN_VERSION) until
    /// [`VCSIndex::version_num`](crate::VCSIndex::version_num) traces the chain.
    #[serde(skip, default = "crate::unknown_version")]
    version_num: u64,
}

impl PartialEq for Version {
    /// Two versions are the same when the field that is written down is: the number is derived, so
    /// a version that has not been worked out is the same version as one that has.
    fn eq(&self, other: &Self) -> bool {
        self.variant == other.variant
    }
}

impl Eq for Version {}

impl Version {
    /// The root of every chain: a Version pointing at no variant
    ///
    /// Every file's first variant is based on it, so a chain traced back from any version ends
    /// here. Its number is one before the first version — `-1` written as a `u64` — so the first
    /// version a file has is numbered from nothing.
    #[must_use]
    pub const fn root() -> Self {
        Self {
            variant: [0_u8; 32],
            version_num: ROOT_VERSION,
        }
    }

    /// A Version built directly from its parts, checking nothing
    ///
    /// It takes the variant's hash and the number on the caller's word: nothing here reads a
    /// Variant to see that it is there, or that `version` is the number the chain would work out.
    /// It is for a version that came out of bytes, and for the making of one through
    /// [`Variant::new_version`].
    #[must_use]
    pub const fn new_bare_version(variant: Blake3Hash, version: u64) -> Self {
        Self {
            variant,
            version_num: version,
        }
    }

    /// The hash of the variant this version points at.
    #[must_use]
    pub const fn variant(&self) -> &Blake3Hash {
        &self.variant
    }

    /// The number of this version in its chain.
    ///
    /// It is [`UNKNOWN_VERSION`](crate::UNKNOWN_VERSION) when the version was read and has not
    /// been worked out.
    #[must_use]
    pub const fn version_num(&self) -> u64 {
        self.version_num
    }

    /// Whether this is the root: a version pointing at no variant.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.variant == [0_u8; 32]
    }

    /// A progressive Variant based on this version, pointing at `storage_hash`
    ///
    /// `creator` and `message` are the hashes of the [`Creator`](crate::Creator) and
    /// [`Message`](crate::Message) that say who made it and what was done; they are the caller's
    /// to write and to hand over. The number does not move: a variant stands with the version it
    /// is based on, so it is the version made of it, if any, that is numbered past it.
    #[must_use]
    pub fn new_variant(
        &self,
        storage_hash: Blake3Hash,
        creator: Blake3Hash,
        message: Blake3Hash,
    ) -> Variant {
        Variant::new_bare_variant(
            storage_hash,
            *self.hash().digest(),
            None,
            creator,
            message,
            self.version_num,
        )
    }
}

impl ComputeBlake3 for Version {
    fn compute_blake3(&self) -> Blake3Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SALT_VER.as_bytes());
        hasher.update(&self.variant);
        hasher.finalize().into()
    }
}

impl VCSWrite for Version {
    fn kind(&self) -> VCSIndexKind {
        VCSIndexKind::Version
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + 32);

        bytes.push(VCSIndexKind::Version.id());
        bytes.extend_from_slice(&self.variant);

        bytes
    }

    fn hash(&self) -> Hash {
        Hash::new(self.compute_blake3())
    }
}
