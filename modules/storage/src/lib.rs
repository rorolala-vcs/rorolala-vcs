#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod algorithm;
pub use algorithm::*;

mod backend;
pub use backend::*;

mod chunk;
pub use chunk::*;

mod codec;
pub use codec::*;

mod config;
pub use config::*;

mod error;
pub use error::*;

mod frame;
pub use frame::*;

mod key;
pub use key::*;

mod locking;
pub use locking::*;

mod manifest;
pub use manifest::*;

mod pack;
pub use pack::*;

// The paths a store lays its objects down at are the store's own business, not part of what a
// caller asks of it — see `internals`.
pub mod internals;

mod presence;
pub use presence::*;

mod zip;
pub use zip::*;

pub mod protocols;

pub mod transfer;

/// Reads the first eight bytes of `bytes` as a big-endian number, and the rest after them.
///
/// Three formats this crate writes — a manifest, a pack index and a transfer's key list — lead with a
/// count and then read a run of fixed-width fields, so the one way to take a number off the front of
/// a byte string is kept here rather than written three times.
pub(crate) fn split_u64(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let (head, rest) = bytes.split_at_checked(size_of::<u64>())?;
    let head: [u8; 8] = head.try_into().ok()?;

    Some((u64::from_be_bytes(head), rest))
}

/// The store Rorolala keeps, wherever one of them is kept.
///
/// It is the one implementation [`protocols`] holds today, kept re-exported here so that what
/// a Workspace or a Vault hands back is one name rather than a path through the module it is
/// implemented in.
pub use protocols::RorolalaStorage;
