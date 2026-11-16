//! How one index object is written for a reader.
//!
//! An object crosses as itself: the text a Creator or a Message is, the hashes a Variant carries,
//! the number and hash of a Version. The kind travels in the data rather than in a line printed, so a
//! `--json` run and the C ABI write the same document, and a reader that parsed one parses the other.
//!
//! The view lives here rather than in the command that first wrote it because the command line and
//! the C ABI are two readers of one shape: a second copy would be free to drift, and a caller would
//! have to notice.

use rorolala_storage::Key;
use serde::Serialize;

use crate::{VCSIndexObject, VCSWrite as _};

/// One index object, as it was read: which kind it is, and what it holds.
///
/// The kind travels in the data rather than in the line printed, so what a `--json` run prints is
/// the object itself, the text of a Creator or a Message, the hashes a Variant carries, the number
/// and hashes of a Version, rather than the string a reader would have been shown.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ReadObject {
    /// A Creator: the name it is.
    Creator {
        /// The name, as text.
        text: String,
    },
    /// A Message: the text it is.
    Message {
        /// The text, as text.
        text: String,
    },
    /// A Variant: the version it is based on, the storage entry it points at, and the variant it
    /// merges in when it is a merge.
    Variant {
        /// The variant's hash.
        hash: Key,
        /// The hash of the version it is based on.
        base_version: Key,
        /// The hash of the storage entry it points at.
        storage: Key,
        /// The hash of the variant it merges in, when it is a merge.
        join: Option<Key>,
    },
    /// A Version: its number, and the variant it points at.
    Version {
        /// The version's number, or nothing when the chain could not be traced.
        number: Option<u64>,
        /// The version's hash.
        hash: Key,
        /// The hash of the variant it points at.
        variant: Key,
    },
}

/// One index object, as the command shows it, with the number a Version was traced to.
///
/// The number is passed in rather than read off the object because which index an object came from
/// decides it: one read here is traced here, and one read from the other end is traced there and
/// carried back in the Version it names. A Creator or a Message has no number, and what is passed
/// for it is not read.
#[must_use]
pub fn view_of(object: VCSIndexObject, number: Option<u64>) -> ReadObject {
    match object {
        VCSIndexObject::Creator(creator) => ReadObject::Creator {
            text: creator.read_to_string().to_owned(),
        },
        VCSIndexObject::Message(message) => ReadObject::Message {
            text: message.read_to_string().to_owned(),
        },
        VCSIndexObject::Variant(variant) => ReadObject::Variant {
            hash: variant.hash(),
            base_version: Key::new(*variant.base_version()),
            storage: Key::new(*variant.storage_hash()),
            join: variant.join().map(|join| Key::new(*join)),
        },
        VCSIndexObject::Version(version) => ReadObject::Version {
            number,
            hash: version.hash(),
            variant: Key::new(*version.variant()),
        },
    }
}
