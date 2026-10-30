#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod colorize;
mod rendering;
mod style;
#[cfg(test)]
mod support;
mod table;
mod theme;

pub use colorize::*;
pub use rendering::TextRendering;
pub use style::*;
pub use table::*;
pub use theme::*;
