//! The number of a version, as it crosses into C.
//!
//! A count crossing as a bare integer cannot be told from any other integer an export hands back,
//! and a fallible export cannot hand one back at all — a `Result` carries a boxed payload, and a
//! scalar is not one. So a number has a type of its own, read back through [`value`](VersionNumber::value).

use rorolala_utils_lazyffi::lazyffi;

/// The number of a version in its chain
#[lazyffi(export = RolaVersionNumber)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionNumber(u64);

impl From<u64> for VersionNumber {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[lazyffi(export = rola_version_number_)]
impl VersionNumber {
    /// The number itself.
    #[must_use]
    #[lazyffi(export = version_number_value)]
    pub const fn value(&self) -> u64 {
        self.0
    }
}
