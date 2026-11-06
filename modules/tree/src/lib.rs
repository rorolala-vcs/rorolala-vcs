#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod baseline;
pub mod consts;
mod error;
mod files;
mod merge;
mod rules;
mod scan;
mod staging;
mod status;
mod structure;
mod tree;

pub use baseline::Baseline;
pub use error::TreeError;
pub use merge::{Merged, merge};
pub use rules::TreeRules;
pub use scan::{Scanned, scan};
pub use staging::Staging;
pub use status::{
    Change, ChangeKind, Conflict, TreeStatus, Untracked, compare, conflict_copy, diff,
};
pub use structure::{EntryKind, FileMode, TreeEntry, TreeStructure};
pub use tree::{
    Recorded, Tree, is_control_file, marked, read_description, read_storage, read_structure,
    version_of,
};
