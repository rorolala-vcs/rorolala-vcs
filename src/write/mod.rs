//! Writing a Workspace, a Vault, a store and an index: the operations the command line and the C
//! ABI share.
//!
//! The modules own one object at a time. What this module owns is the order: what is read before
//! what is written, which failure stops the run, and how a caller stops it. A command and a C
//! caller enter the same function, so the order cannot drift between them.

pub mod align;
pub mod bring;
pub mod cancel;
pub mod checkout;
pub mod collapse;
pub mod collect;
pub mod cut;
pub mod error;
pub mod fetch_with;
pub mod index;
pub mod labels;
pub mod layout;
pub mod layout_remote;
pub mod ownership;
pub mod pack;
pub mod path;
pub mod progress;
pub mod request;
pub mod retrack;
pub mod storage;
pub mod sync;
pub mod sync_apply;
pub mod track;
pub mod tree;
pub mod tree_init;
pub mod tree_remove;
pub mod tree_reset;
pub mod tweak;
pub mod workspace;

pub use cancel::*;
pub use checkout::*;
pub use collapse::*;
pub use cut::*;
pub use error::*;
pub use index::*;
pub use labels::*;
pub use layout::*;
pub use layout_remote::*;
pub use pack::*;
pub use progress::*;
pub use retrack::*;
pub use storage::*;
pub use track::*;
pub use tree::*;
pub use tree_init::*;
pub use tree_remove::*;
pub use tree_reset::*;
pub use workspace::*;
