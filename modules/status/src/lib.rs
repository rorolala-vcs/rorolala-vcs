#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod base;
mod ffi;
mod merge;
mod ownership;
mod status;

// The base of a version is read through the index, which is the FFI surface's own business alone; what
// crosses is the standing itself, so the reader beside it is named here rather than re-exported.
pub use base::{Base, Relation, Standing, compare, wrong_base};
pub use ffi::*;
pub use merge::*;
pub use ownership::*;
pub use status::*;
