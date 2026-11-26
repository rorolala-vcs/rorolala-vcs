//! Naming a version of a chain, by what a person writes on a command line.
//!
//! A chain is read by `modules/vcs` and adjusted by `librorolala`; what this crate owns is the
//! question in front of both: which level a word names. A whole hash, the head of one, a variant's
//! hash, `~N`, or text of a creator or message all answer the same question, so `tweak`, `collapse`
//! and `cut` read one answer rather than three.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod level;
mod target;

pub use level::*;
pub use target::*;
