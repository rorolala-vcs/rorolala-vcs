//! The roles by which one object points at another.
//!
//! A key is pointed at by different objects for different reasons: a Variant points at the content
//! it was made of, at the Version it is based on, at the Variant it merges in, and at the text that
//! says who made it and what was done; a Version points at the Variant it fixes. What a reader asks
//! for is the dependents *of one role* — which versions pin this variant is a different question
//! from which variants merged it in — so the role is kept beside the source rather than the key
//! alone being enough.

use rorolala_utils_lazyffi::lazyffi;

/// The role by which a source object points at a target key
#[lazyffi(export = RolaInverseEdge)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Edge {
    /// A Variant's storage address: the content the variant was made of.
    Storage = 0,
    /// A Variant's base Version.
    Base = 1,
    /// The Variant a Variant merges in.
    Join = 2,
    /// A Variant's Creator.
    Creator = 3,
    /// A Variant's Message.
    Message = 4,
    /// A Version's Variant.
    Variant = 5,
}

impl Edge {
    /// The byte this role is written down as.
    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// The role written down by `id`, if this build knows one.
    #[must_use]
    pub const fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Storage),
            1 => Some(Self::Base),
            2 => Some(Self::Join),
            3 => Some(Self::Creator),
            4 => Some(Self::Message),
            5 => Some(Self::Variant),
            _ => None,
        }
    }
}
