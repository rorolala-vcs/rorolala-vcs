//! One level of a chain, as the naming reads it.

use rorolala_vcs::{Variant, Version};

/// One version of a chain: the variant that produced it, and what the two say.
#[derive(Debug, Clone)]
pub struct Level {
    /// The version itself.
    pub version: Version,
    /// The variant the version points at, which is what produced it.
    pub variant: Variant,
    /// Who made it, as text.
    pub creator: String,
    /// What it says, as text.
    pub message: String,
}
