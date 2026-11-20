#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// Deserializing builds each value out of plain fields; the `unsafe` methods the lint sees are the
// FFI conversions `lazyffi` generates beside the types, not anything the data goes through.
#![allow(clippy::unsafe_derive_deserialize)]

mod account;
mod channel;
mod current;
mod error;
mod key;
mod locate;
mod member;
mod rule;

pub use account::*;
pub use channel::*;
pub use current::*;
pub use error::*;
pub use key::*;
pub use locate::*;
pub use member::*;
pub use rule::*;

/// The directory a user's keys are kept in, under this build's userdata name.
pub use rorolala_utils_location::user_keys_dir;
