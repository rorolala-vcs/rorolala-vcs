use rorolala_storage::Blake3Hash;
use rorolala_utils_lazyffi::lazyffi;

use crate::error::TextError;
use crate::index::{VCSIndexKind, VCSWrite};
use crate::{ComputeBlake3, Hash, SALT_MSG};

/// The longest a text object may be, in bytes
const LONGEST: usize = 256;

/// A message, as an index object
///
/// A Version says what was done by carrying the hash of one of these. Rola's messages are far
/// fewer than git's: one short piece of text stands for a change, rather than a subject line and
/// a body, so what is held is the text and nothing else. It is a newtype around the text, so
/// what goes in and out is a string.
#[lazyffi(export = RolaMessage)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message(String);

impl Message {
    /// The message, as text.
    #[must_use]
    pub fn read_to_string(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Message {
    type Error = TextError;

    /// Reads a message from `text`, refusing one that is too long.
    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text.len() > LONGEST {
            return Err(TextError::TooLong);
        }

        Ok(Self(text))
    }
}

impl TryFrom<&str> for Message {
    type Error = TextError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::try_from(text.to_owned())
    }
}

impl TryFrom<Vec<u8>> for Message {
    type Error = TextError;

    /// Reads a message from the bytes it was stored as, refusing bytes that are not UTF-8.
    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(String::from_utf8(bytes).map_err(|_| TextError::NotUtf8)?)
    }
}

impl From<Message> for String {
    fn from(message: Message) -> Self {
        message.0
    }
}

impl AsRef<str> for Message {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ComputeBlake3 for Message {
    fn compute_blake3(&self) -> Blake3Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SALT_MSG.as_bytes());
        hasher.update(self.0.as_bytes());
        hasher.finalize().into()
    }
}

impl VCSWrite for Message {
    fn kind(&self) -> VCSIndexKind {
        VCSIndexKind::Message
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + self.0.len());

        bytes.push(VCSIndexKind::Message.id());
        bytes.extend_from_slice(self.0.as_bytes());

        bytes
    }

    fn hash(&self) -> Hash {
        Hash::new(self.compute_blake3())
    }
}
