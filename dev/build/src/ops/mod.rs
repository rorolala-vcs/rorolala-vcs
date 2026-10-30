//! The steps that produce this workspace's artifacts, one to a module.
//!
//! A step is a function here and a command beside it: what the command does is run the function,
//! and what the function does is run the tree's own programs — never a second description of them.

pub mod book;
pub mod build;
pub mod cargo_clean;
pub mod chunker;
pub mod clean;
pub mod desktop;
pub mod export;
pub mod icons;
pub mod library;
pub mod programs;
pub mod publish;

// See the note at the crate root: a command's generated types have to be in scope there, and this is
// what puts them on the way. The pass-through has no command of its own — its words are the
// harness's — so it is reached by name rather than by this.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use {
    book::*, build::*, cargo_clean::*, clean::*, desktop::*, export::*, icons::*, library::*,
    programs::*, publish::*,
};
