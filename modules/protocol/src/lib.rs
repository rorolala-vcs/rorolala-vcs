#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// Deserializing builds each value out of plain fields; the `unsafe` methods the lint sees are the
// FFI conversions `lazyffi` generates beside the types, not anything the data goes through.
#![allow(clippy::unsafe_derive_deserialize)]
// The `pub(crate)` items are the plumbing of this crate's own modules, and `pub use data::*;` below
// would carry a `pub` one out to a caller: `OnlyVault::holding` and its like are what let a module
// here build a side-tagged value, and an outside caller must not be able to claim the tag itself.
// The lint reads the private modules as making `pub` and `pub(crate)` the same visibility; the
// re-exports are what makes them differ.
#![allow(clippy::redundant_pub_crate)]

#[macro_use]
mod macros;

mod address;
pub use address::*;

mod action;
pub use action::*;

mod both;
pub use both::*;

mod context;
pub use context::*;

mod data;
pub use data::*;

mod encode;
pub use encode::*;

mod error;
pub use error::*;

mod sync;
pub use sync::*;

mod sync_mut;
pub use sync_mut::*;
