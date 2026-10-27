//! The `rola.lock` format: one material list and the sources its content comes from.
//!
//! A lock is generated — `rola.toml` is the file a person writes — and is read by the git support
//! to put a project's materials back. This crate is the only place its text is read or written:
//! [`Lock::decode`] and [`Lock::encode`] and nothing else. It holds no files, no network and no
//! git; the caller owns where the text came from and where it goes.

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod codec;
mod error;
mod hash;
mod lock;
mod name;
mod path;

pub use error::*;
pub use hash::*;
pub use lock::*;
pub use name::*;
pub use path::*;

/// The version of the lock format this build reads and writes.
///
/// It is the number a lock's first line carries. A lock naming another version is refused whole
/// rather than read approximately: the format is a generated file, so the answer to one this build
/// does not know is to regenerate it with a build that does.
pub const FORMAT_VERSION: u32 = 1;
