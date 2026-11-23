//! How one index object is written for a reader.
//!
//! An object crosses as itself: the text a Creator or a Message is, the hashes a Variant carries,
//! the number and hash of a Version. The kind travels in the data rather than in a line printed, so a
//! `--json` run and the C ABI read the same fields, and a caller that read one reads the other.
//!
//! The view lives here rather than in the command that first wrote it because the command line and
//! the C ABI are two readers of one shape: a second copy would be free to drift, and a caller would
//! have to notice.

use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
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

/// One index object, as the C ABI hands it over: which kind it is, and what it holds.
///
/// The numbers that may be absent are said as a presence flag beside them, because a scalar has no
/// null to spend at the boundary: a Version whose chain could not be traced has
/// `number_present` false and a `number` that says nothing. The hashes are hex text, and a Variant
/// that merges none has an empty `join`.
///
/// # FFI
///
/// A tagged union: `RolaVCSIndexObjectViewTag` says which case it is, and
/// `RolaVCSIndexObjectViewPayload` holds its fields. Every `char *` is owned by the caller and
/// released with `free_string`, one by one; the value itself, when it came back as a result
/// payload, is released with `free_rola_vcs_index_object_view`.
#[lazyffi(export = RolaVCSIndexObjectView)]
#[derive(Debug, Clone)]
pub enum ReadObjectView {
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
    /// A Variant: the hashes it carries.
    Variant {
        /// The variant's hash, as hex.
        hash: String,
        /// The hash of the version it is based on, as hex.
        base_version: String,
        /// The hash of the storage entry it points at, as hex.
        storage: String,
        /// The hash of the variant it merges in, as hex, or an empty string when it merges none.
        join: String,
    },
    /// A Version: its hash, the variant it points at, and the number traced for it.
    Version {
        /// Whether a number was traced for the version.
        number_present: bool,
        /// The version's number, or 0 when none was traced.
        number: u64,
        /// The version's hash, as hex.
        hash: String,
        /// The hash of the variant it points at, as hex.
        variant: String,
    },
}

impl ReadObjectView {
    /// The view of `object`, with the number traced for a Version already in it.
    pub(crate) fn of(view: ReadObject) -> Self {
        match view {
            ReadObject::Creator { text } => Self::Creator { text },
            ReadObject::Message { text } => Self::Message { text },
            ReadObject::Variant {
                hash,
                base_version,
                storage,
                join,
            } => Self::Variant {
                hash: hash.hex(),
                base_version: base_version.hex(),
                storage: storage.hex(),
                join: join.map_or_else(String::new, |join| join.hex()),
            },
            ReadObject::Version {
                number,
                hash,
                variant,
            } => Self::Version {
                number_present: number.is_some(),
                number: number.unwrap_or_default(),
                hash: hash.hex(),
                variant: variant.hex(),
            },
        }
    }
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
