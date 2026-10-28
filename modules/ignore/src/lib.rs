#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod defaults;
mod ffi;
mod natural;
mod rules;

pub use defaults::*;
pub use ffi::*;
pub use rules::*;
