//! The version chain, as the drawings of it read.
//!
//! A [`Chain`] is the top of a version chain and every variant under it, the variants that form the
//! spine, one per level, and those merged into them. It says nothing about where anything is drawn:
//! the terminal's drawing is `rola`'s business, and the Desktop's Version Graph reads the same shape.
//!
//! The root version is not part of a chain: the chain stops at the first version, so every level is
//! zero or more. The top may be a version with a number, or, when a single variant is looked back
//! from, the version that variant produced, whose number is unknown, or the version being edited,
//! which the index does not have at all. What each is drawn as is [`Top`]'s business.

/// What the top of a chain is, as it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top {
    /// The highest version's number, drawn `V{n}`.
    Numbered,
    /// A version whose number is unknown, drawn `?`.
    ///
    /// It is the top a Variant is looked back from: the variant is the one the highest version was
    /// produced by, but which version that is takes a search of the whole index to find, and a
    /// drawing does not know it.
    Unknown,
    /// A version being edited, drawn `??`.
    ///
    /// It is the top a file with changes that were never recorded is looked back from: the version
    /// the work is on its way to is not one the index has, and neither is the variant that would
    /// produce it, so both are placeholders over the version being edited.
    Editing,
}

/// A chain of versions, top to bottom.
pub struct Chain {
    /// The number of the highest version drawn.
    top: u64,
    /// What the highest version is, as it is drawn.
    top_kind: Top,
    /// Every variant drawn, spine and merged alike.
    nodes: Vec<Node>,
}

impl Chain {
    /// A chain whose highest version is at `top`, over `nodes`.
    #[must_use]
    pub const fn new(top: u64, top_kind: Top, nodes: Vec<Node>) -> Self {
        Self {
            top,
            top_kind,
            nodes,
        }
    }

    /// The number of the highest version drawn.
    #[must_use]
    pub const fn top(&self) -> u64 {
        self.top
    }

    /// What the highest version is, as it is drawn.
    #[must_use]
    pub const fn top_kind(&self) -> Top {
        self.top_kind
    }

    /// Every variant drawn.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
}

/// One variant of a [`Chain`].
pub struct Node {
    /// The label it is drawn by: a variant's hash, shortened to its leading characters.
    label: String,
    /// The number of the version it is based on, which is the level it sits at.
    level: u64,
    /// Whether it is the spine variant of its level: the one the version above points at.
    spine: bool,
    /// The variant it merges in, as an index into the chain's nodes.
    join: Option<usize>,
}

impl Node {
    /// A node drawn by `label` at `level`.
    #[must_use]
    pub const fn new(label: String, level: u64, spine: bool, join: Option<usize>) -> Self {
        Self {
            label,
            level,
            spine,
            join,
        }
    }

    /// The label it is drawn by.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The level it sits at: the number of the version it is based on.
    #[must_use]
    pub const fn level(&self) -> u64 {
        self.level
    }

    /// Whether it is the spine variant of its level.
    #[must_use]
    pub const fn is_spine(&self) -> bool {
        self.spine
    }

    /// The variant it merges in, as an index into the chain's nodes.
    #[must_use]
    pub const fn join(&self) -> Option<usize> {
        self.join
    }
}
