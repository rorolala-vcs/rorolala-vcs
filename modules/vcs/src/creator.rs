use rorolala_storage::Blake3Hash;
use rorolala_utils_lazyffi::lazyffi;

use crate::error::TextError;
use crate::index::{VCSIndexKind, VCSWrite};
use crate::{ComputeBlake3, Hash, SALT_CRT};

/// The longest a text object may be, in bytes
const LONGEST: usize = 256;

/// The name of a member, as an index object
///
/// A Version says who made it by carrying the hash of one of these, so what a version holds is a
/// name and nothing else: there is no account behind it, and nothing here checks that a name
/// names anyone. It is a newtype around the text, so what goes in and out is a string.
#[lazyffi(export = RolaCreator)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creator(String);

impl Creator {
    /// The name, as text.
    #[must_use]
    pub fn read_to_string(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Creator {
    type Error = TextError;

    /// Reads a name from `name`, refusing one that is too long.
    fn try_from(name: String) -> Result<Self, Self::Error> {
        if name.len() > LONGEST {
            return Err(TextError::TooLong);
        }

        Ok(Self(name))
    }
}

impl TryFrom<&str> for Creator {
    type Error = TextError;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Self::try_from(name.to_owned())
    }
}

impl TryFrom<Vec<u8>> for Creator {
    type Error = TextError;

    /// Reads a name from the bytes it was stored as, refusing bytes that are not UTF-8.
    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(String::from_utf8(bytes).map_err(|_| TextError::NotUtf8)?)
    }
}

impl From<Creator> for String {
    fn from(creator: Creator) -> Self {
        creator.0
    }
}

impl AsRef<str> for Creator {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ComputeBlake3 for Creator {
    fn compute_blake3(&self) -> Blake3Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SALT_CRT.as_bytes());
        hasher.update(self.0.as_bytes());
        hasher.finalize().into()
    }
}

impl VCSWrite for Creator {
    fn kind(&self) -> VCSIndexKind {
        VCSIndexKind::Creator
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + self.0.len());

        bytes.push(VCSIndexKind::Creator.id());
        bytes.extend_from_slice(self.0.as_bytes());

        bytes
    }

    fn hash(&self) -> Hash {
        Hash::new(self.compute_blake3())
    }
}
