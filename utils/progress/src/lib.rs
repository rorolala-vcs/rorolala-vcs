#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod render;
pub use render::*;

mod reporter;
pub use reporter::*;

mod signal;
pub use signal::*;
