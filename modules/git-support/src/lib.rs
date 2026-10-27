//! The local half of rola's git support.
//!
//! A project is a directory holding `rola.toml`; beside it sits `rola.lock`, the generated list of
//! paths and the content placed at them. This crate reads both, finds the git work tree the project
//! is inside, writes the project's block of `.git/info/exclude`, and moves materials out of the
//! working tree and back. It answers nothing on its own: the programs are `git-rola`, and the
//! network, the Vault and the store are theirs to reach.

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod config;
mod content;
mod error;
mod exclude;
mod git;
mod project;
mod session;
mod staging;

pub use config::*;
pub use content::*;
pub use error::*;
pub use exclude::*;
pub use git::*;
pub use project::*;
pub use session::*;
pub use staging::*;
