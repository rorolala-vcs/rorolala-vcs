#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::redundant_pub_crate)]

#[macro_use]
mod macros;

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
