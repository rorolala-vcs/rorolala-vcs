//! A call site that names a translation key.

use std::path::PathBuf;

/// One call naming a key.
pub struct Call {
    /// The file the call is written in.
    pub file: PathBuf,
    /// The line the call starts on.
    pub line: usize,
    /// The key it names.
    pub key: String,
}

/// The calls under a directory, and how many named a key the scan cannot read.
#[derive(Default)]
pub struct Calls {
    /// The calls that name a literal key.
    pub calls: Vec<Call>,
    /// How many calls named a key in some other way, which a static scan cannot follow.
    pub unchecked: usize,
}
