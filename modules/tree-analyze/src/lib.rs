#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod cache;
mod error;
mod fingerprint;
mod scan;
mod tree_diff;

pub use cache::Cache;
pub use error::*;
pub use scan::{Found, walk};
pub use tree_diff::*;
