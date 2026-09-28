#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use rorolala_storage::Blake3Hash;
use rorolala_utils_lazyffi::lazyffi;

/// The content address an index object is named by
///
/// An index object is stored under the same kind of key a store's content is, so the two are
/// one type rather than two that would have to be kept in step.
pub use rorolala_storage::Key as Hash;

mod creator;
mod error;
mod graph;
mod hex;
mod index;
mod message;
mod variant;
mod version;

pub use creator::*;
pub use error::*;
pub use graph::*;
pub use index::*;
pub use message::*;
pub use variant::*;
pub use version::*;

/// Hash salt for Variant
#[lazyffi(export = ROLA_SALT_VRT)]
pub const SALT_VRT: &str = "VRT";

/// Hash salt for Version
#[lazyffi(export = ROLA_SALT_VER)]
pub const SALT_VER: &str = "VER";

/// Hash salt for Layout
#[lazyffi(export = ROLA_SALT_LYT)]
pub const SALT_LYT: &str = "LYT";

/// Hash salt for Creator
#[lazyffi(export = ROLA_SALT_CRT)]
pub const SALT_CRT: &str = "CRT";

/// Hash salt for Message
#[lazyffi(export = ROLA_SALT_MSG)]
pub const SALT_MSG: &str = "MSG";

/// A trait for types that can compute their BLAKE3 hash.
pub trait ComputeBlake3 {
    /// Computes the BLAKE3 hash of `self`.
    fn compute_blake3(&self) -> Blake3Hash;
}

/// The number the root of a version chain carries: one before the first version
///
/// It is `-1` written as a `u64`, so the first version a file has is numbered from nothing:
/// making a version of the root's variant steps past it, wrapping to zero.
pub const ROOT_VERSION: u64 = u64::MAX;

/// The number an object carries when it has only been read, not worked out
///
/// A number is derived rather than stored — it is neither written down nor put into the hash —
/// so a read cannot know it. This stands in until
/// [`VCSIndex::version_num`](crate::VCSIndex::version_num) traces the chain.
pub const UNKNOWN_VERSION: u64 = u64::MAX - 1;

/// The number a derived field is filled with when it is read back without one.
///
/// This is what `#[serde(skip)]` uses for the fields that are not stored, so a structured read
/// gives them [`UNKNOWN_VERSION`] rather than a zero that would look like a version.
#[must_use]
pub const fn unknown_version() -> u64 {
    UNKNOWN_VERSION
}
