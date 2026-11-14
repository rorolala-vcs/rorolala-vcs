#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod base;
mod existence;
mod ffi;
mod graph;
mod merge;
mod ownership;
mod status;
mod variant_graph;

// The base of a version is read through the index, which is the FFI surface's own business alone; what
// crosses is the standing itself, so the reader beside it is named here rather than re-exported.
pub use base::{Base, Relation, Standing, compare, wrong_base};
pub use existence::*;
pub use ffi::*;
pub use graph::*;
pub use merge::*;
pub use ownership::*;
pub use status::*;
pub use variant_graph::*;
