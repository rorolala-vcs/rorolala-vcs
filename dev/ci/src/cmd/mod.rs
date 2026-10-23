//! The commands that are about a run rather than about the tree: what it recorded, removing that
//! so the next run starts from nothing, and the lock pair a run is wrapped in.
//!
//! They are commands of their own rather than part of the gate, so a run can be recorded by the
//! checks that were run one at a time and read afterwards, and a tree can be locked around them.

pub mod git_lock;
pub mod git_unlock;
pub mod report_clean;
pub mod report_collect;

// See the note at the crate root: a command's generated types have to be in scope there, and this is
// what puts them on the way.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use {git_lock::*, git_unlock::*, report_clean::*, report_collect::*};
