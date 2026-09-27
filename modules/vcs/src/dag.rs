//! The chain a lookback walks, in the shape the graph draws it.
//!
//! A chain is the top of a version chain and every variant under it, the variants that form the
//! spine — one per level — and those merged into them. It says nothing about where anything is
//! drawn; that is [`layout`](crate::draw)'s work. The root version is not part of a chain: the
//! chain stops at the first version, so every level is zero or more.
//!
//! The top may be a version or, when a single variant is looked back from, that variant itself: a
//! variant is drawn through the version it is based on, so looking back from one shows the same
//! chain with the variant's own row at the top and the version above it left off — see
//! [`Chain::variant_top`].

/// A chain of versions, top to bottom.
pub struct Chain {
    /// The number of the highest version drawn.
    top: u64,
    /// Whether the top row is a variant rather than a version's number.
    variant_top: bool,
    /// Every variant drawn, spine and merged alike.
    nodes: Vec<Node>,
}

impl Chain {
    /// A chain whose highest version is numbered `top`, over `nodes`.
    #[must_use]
    pub const fn new(top: u64, nodes: Vec<Node>) -> Self {
        Self {
            top,
            variant_top: false,
            nodes,
        }
    }

    /// A chain that starts at a variant: `top` is the number of the version the variant is based
    /// on, and that variant is the top row rather than the version above it.
    #[must_use]
    pub const fn variant_top(top: u64, nodes: Vec<Node>) -> Self {
        Self {
            top,
            variant_top: true,
            nodes,
        }
    }

    /// The number of the highest version drawn.
    #[must_use]
    pub const fn top(&self) -> u64 {
        self.top
    }

    /// Whether the top row is a variant rather than a version's number.
    #[must_use]
    pub const fn has_variant_top(&self) -> bool {
        self.variant_top
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
