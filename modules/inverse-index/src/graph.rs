//! The variant graph a view is cut out of: which variants a window holds, and how they meet.
//!
//! The records describe every object at once, which is more than a drawing asks for. What a view is
//! asked for is a window: a focus to stand on, how far back and forward to walk, and a hard cap. What
//! it answers with is the variants in that window, the versions fixing them, the edges between them,
//! and where the window was cut — a [`Frontier`] on each node — so a reader that reaches the edge of
//! what it has can walk further from the node it reached rather than be handed the whole index.
//!
//! A window is a map, not a picture: what it holds is decided by the request, and every answer is
//! the same for the same request. Nothing here reads an object, and nothing fetches one: a version
//! the index does not hold is named as [`Missing`] and left for a reader to ask for its sync.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rorolala_vcs::Hash;

use crate::Snapshot;

/// How many variants back a graph walks from a focus unless it is told otherwise.
const DEFAULT_BACK: u32 = 64;

/// How many variants a graph holds unless it is told otherwise.
const DEFAULT_MAX_NODES: u32 = 400;

/// What a graph is asked for: where to stand, and how far to look.
#[derive(Debug, Clone)]
pub struct GraphRequest {
    /// The versions the window is centred on.
    pub focus: Vec<Hash>,
    /// How many variants back towards the root to walk from each focus.
    pub back: u32,
    /// How many variants forward from each focus to walk.
    pub forward: u32,
    /// Whether to walk into what a variant merges in.
    pub joins: bool,
    /// Whether to take in a variant based on a version in the window, one hop.
    pub branches: bool,
    /// How many variants the window holds at most.
    pub max_nodes: u32,
}

impl Default for GraphRequest {
    fn default() -> Self {
        Self {
            focus: Vec::new(),
            back: DEFAULT_BACK,
            forward: 0,
            joins: true,
            branches: true,
            max_nodes: DEFAULT_MAX_NODES,
        }
    }
}

/// A walk to make from one node rather than from a focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandRequest {
    /// The variant to walk from.
    pub node: Hash,
    /// Which way to walk.
    pub direction: ExpandDirection,
    /// How many variants to walk through.
    pub depth: u32,
    /// How many variants the answer holds at most.
    pub max_nodes: u32,
}

/// Which way an expansion walks from its node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandDirection {
    /// Towards the root: the base chain.
    Parents,
    /// Away from the root: the variants based on the node's versions.
    Children,
    /// Into what the node merges in: the chain the merge came from.
    Joined,
}

/// A window of the variant graph.
#[derive(Debug, Clone, Default)]
pub struct GraphView {
    /// What each focus version resolved to, in the order asked.
    pub anchors: Vec<Anchor>,
    /// The variants in the window, in key order.
    pub nodes: Vec<GraphNode>,
    /// The versions fixing them, in key order.
    pub versions: Vec<GraphVersion>,
    /// The edges between variants in the window, in `(from, to, kind)` order.
    pub edges: Vec<GraphEdge>,
    /// The keys the window names but the index does not hold, in key and kind order.
    pub missing: Vec<Missing>,
    /// Whether the window was cut short by its cap rather than walked out.
    pub truncated: bool,
}

/// A focus version, and the variant it fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    /// The version asked about.
    pub version: Hash,
    /// The variant it fixes, when the index holds it.
    pub variant: Option<Hash>,
}

/// A variant in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphNode {
    /// The variant itself.
    pub hash: Hash,
    /// The version it is based on.
    pub base_version: Hash,
    /// The variant that version fixes, when the index holds it.
    pub base_variant: Option<Hash>,
    /// The variant it merges in, when it is a merge.
    pub join: Option<Hash>,
    /// The content it was made of.
    pub storage: Hash,
    /// Who made it.
    pub creator: Hash,
    /// What it says was done.
    pub message: Hash,
    /// Where the window stops short of what this node names.
    pub frontier: Frontier,
}

/// A version fixing a variant in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphVersion {
    /// The version itself.
    pub hash: Hash,
    /// The variant it fixes.
    ///
    /// The root points at no variant: what is named here for it is the zero hash, and `is_root` is
    /// what says so.
    pub variant: Hash,
    /// Its number in its chain, when it has one.
    pub number: Option<u64>,
    /// Whether it is the root: the version every chain starts from, numbered before the first.
    pub is_root: bool,
}

/// An edge between two variants in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphEdge {
    /// The variant the edge is walked from.
    pub from: Hash,
    /// The variant it names.
    pub to: Hash,
    /// What the edge is.
    pub kind: GraphEdgeKind,
}

/// What an edge between two variants is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphEdgeKind {
    /// The variant is based on the version the target fixes: the chain.
    Base,
    /// The variant merges the target in.
    Join,
}

/// A key a window names but the index does not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Missing {
    /// The key nothing is stored under.
    pub hash: Hash,
    /// What kind of object it would have been.
    pub kind: MissingKind,
}

/// What a key the index does not hold would have been.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MissingKind {
    /// A version, named as a base or a focus.
    Version,
    /// A variant, named as a base, a join or a focus's variant.
    Variant,
}

/// Where a node's walks were not taken in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Frontier {
    /// The base variant, or the version it is based on, is not in the window.
    pub parents: bool,
    /// A variant based on one of the node's versions is not in the window.
    pub children: bool,
    /// A variant the node merges in is not in the window.
    pub joined: bool,
}

impl Snapshot {
    /// The window of the graph `request` asks for.
    ///
    /// The window is walked from each focus: back along the base chain, forward over the variants
    /// based on it, into what the variants in the window merge in, and one hop into the variants
    /// based on the versions in the window — each within the request's limits. A version or variant
    /// the records do not name is answered as missing rather than left unexplained, since a reader
    /// that asked where its focus is can only be told that it is not here.
    #[must_use]
    pub fn graph(&self, request: &GraphRequest) -> GraphView {
        let mut walk = Walk::new(self, request.max_nodes);
        let mut anchors = Vec::with_capacity(request.focus.len());
        let mut starts = Vec::new();

        for version in &request.focus {
            let Some(record) = self.version(*version) else {
                walk.missing.insert(Missing {
                    hash: *version,
                    kind: MissingKind::Version,
                });
                anchors.push(Anchor {
                    version: *version,
                    variant: None,
                });

                continue;
            };

            walk.versions.insert(*version);

            // The root points at no variant: it is the version before the first, not a state a
            // variant was made of, so it is taken in as itself rather than as a missing variant.
            let variant = if record.is_root {
                None
            } else if self.variant(record.variant).is_some() {
                Some(record.variant)
            } else {
                walk.missing.insert(Missing {
                    hash: record.variant,
                    kind: MissingKind::Variant,
                });

                None
            };

            anchors.push(Anchor {
                version: *version,
                variant,
            });

            if let Some(variant) = variant {
                starts.push(variant);
            }
        }

        for start in &starts {
            walk.walk_parents(*start, request.back);
        }
        for start in &starts {
            walk.walk_children(*start, request.forward);
        }
        if request.joins {
            walk.walk_joins(request.back);
        }
        if request.branches {
            walk.take_branches();
        }

        walk.view(anchors)
    }

    /// The window one step of a walk asks for: what the walk from `request.node` reaches.
    ///
    /// This is what a reader asks for when it reaches the edge of what it has been given: the
    /// answer is the part of the graph the node names in that direction, and a reader that already
    /// holds some of it keeps what it has rather than merging the view wholesale.
    #[must_use]
    pub fn expand(&self, request: &ExpandRequest) -> GraphView {
        let mut walk = Walk::new(self, request.max_nodes);

        if self.variant(request.node).is_none() {
            walk.missing.insert(Missing {
                hash: request.node,
                kind: MissingKind::Variant,
            });

            return walk.view(Vec::new());
        }

        match request.direction {
            ExpandDirection::Parents => walk.walk_parents(request.node, request.depth),
            ExpandDirection::Children => walk.walk_children(request.node, request.depth),
            ExpandDirection::Joined => {
                walk.take_variant(request.node);
                walk.walk_joins(request.depth);
            }
        }

        walk.view(Vec::new())
    }
}

/// A window while it is being walked, before it is laid out in the order a view is read in.
struct Walk<'a> {
    /// The records every step reads.
    snapshot: &'a Snapshot,
    /// How many variants the window holds at most.
    max_nodes: u32,
    /// The variants taken in.
    nodes: BTreeSet<Hash>,
    /// The versions taken in.
    versions: BTreeSet<Hash>,
    /// The keys named but not held.
    missing: BTreeSet<Missing>,
    /// The depth the base chain was walked to from each variant, so it is not walked again.
    parents_walked: BTreeMap<Hash, u32>,
    /// The depth the children were walked to from each variant.
    children_walked: BTreeMap<Hash, u32>,
    /// Whether a step was refused by the cap.
    truncated: bool,
}

impl<'a> Walk<'a> {
    /// A window over `snapshot`, holding `max_nodes` variants at most.
    const fn new(snapshot: &'a Snapshot, max_nodes: u32) -> Self {
        Self {
            snapshot,
            max_nodes,
            nodes: BTreeSet::new(),
            versions: BTreeSet::new(),
            missing: BTreeSet::new(),
            parents_walked: BTreeMap::new(),
            children_walked: BTreeMap::new(),
            truncated: false,
        }
    }

    /// Takes a variant in, with the versions fixing it and what it names that is not here.
    ///
    /// Answers whether the variant is in the window: a variant the records do not name is noted as
    /// missing, and one the cap leaves no room for stops the walk short rather than being dropped.
    fn take_variant(&mut self, hash: Hash) -> bool {
        if self.nodes.contains(&hash) {
            return true;
        }

        let Some(record) = self.snapshot.variant(hash) else {
            self.missing.insert(Missing {
                hash,
                kind: MissingKind::Variant,
            });

            return false;
        };

        if self.nodes.len() >= usize::try_from(self.max_nodes).unwrap_or(usize::MAX) {
            self.truncated = true;

            return false;
        }

        self.nodes.insert(hash);

        for version in self.snapshot.versions_pinning(hash) {
            self.versions.insert(*version);
        }

        // What the base version resolves to is part of the node, and a base that resolves to
        // nothing is the root or a version the index does not hold — the one is drawn as the start
        // of the chain, the other as a key to fetch, and neither is a variant in the window.
        match self.snapshot.version(record.base_version) {
            Some(base) => {
                self.versions.insert(record.base_version);

                if !base.is_root && self.snapshot.variant(base.variant).is_none() {
                    self.missing.insert(Missing {
                        hash: base.variant,
                        kind: MissingKind::Variant,
                    });
                }
            }
            None => {
                self.missing.insert(Missing {
                    hash: record.base_version,
                    kind: MissingKind::Version,
                });
            }
        }

        if let Some(join) = record.join
            && self.snapshot.variant(join).is_none()
        {
            self.missing.insert(Missing {
                hash: join,
                kind: MissingKind::Variant,
            });
        }

        true
    }

    /// The variant `hash` is based on, when the records name both the base version and its variant.
    fn base_variant(&self, hash: Hash) -> Option<Hash> {
        let record = self.snapshot.variant(hash)?;
        let base = self.snapshot.version(record.base_version)?;

        if base.is_root || self.snapshot.variant(base.variant).is_none() {
            return None;
        }

        Some(base.variant)
    }

    /// Walks the base chain back from `start`, taking in `depth` variants before it.
    fn walk_parents(&mut self, start: Hash, depth: u32) {
        let mut queue = VecDeque::from([(start, depth)]);

        while let Some((hash, remaining)) = queue.pop_front() {
            if self
                .parents_walked
                .get(&hash)
                .is_some_and(|walked| *walked >= remaining)
            {
                continue;
            }
            if !self.take_variant(hash) {
                continue;
            }

            self.parents_walked.insert(hash, remaining);

            if remaining == 0 {
                continue;
            }
            if let Some(base) = self.base_variant(hash) {
                queue.push_back((base, remaining - 1));
            }
        }
    }

    /// Walks forward from `start`, taking in `depth` variants the chain goes on to.
    fn walk_children(&mut self, start: Hash, depth: u32) {
        let mut queue = VecDeque::from([(start, depth)]);

        while let Some((hash, remaining)) = queue.pop_front() {
            if self
                .children_walked
                .get(&hash)
                .is_some_and(|walked| *walked >= remaining)
            {
                continue;
            }
            if !self.take_variant(hash) {
                continue;
            }

            self.children_walked.insert(hash, remaining);

            if remaining == 0 {
                continue;
            }
            for child in self.children_of(hash) {
                queue.push_back((child, remaining - 1));
            }
        }
    }

    /// The variants based on a version fixing `variant`, in key order.
    fn children_of(&self, variant: Hash) -> Vec<Hash> {
        let mut children = Vec::new();

        for version in self.snapshot.versions_pinning(variant) {
            children.extend_from_slice(self.snapshot.variants_based_on(*version));
        }

        children.sort_unstable();
        children.dedup();

        children
    }

    /// Takes in the chain each merge the window holds comes from, until no merge is left.
    fn walk_joins(&mut self, depth: u32) {
        let mut taken = BTreeSet::new();

        loop {
            let joins: Vec<Hash> = self
                .nodes
                .iter()
                .filter_map(|hash| self.snapshot.variant(*hash).and_then(|record| record.join))
                .filter(|join| !taken.contains(join))
                .collect();

            if joins.is_empty() {
                return;
            }

            for join in joins {
                taken.insert(join);
                self.walk_parents(join, depth);
            }
        }
    }

    /// Takes in one hop of the variants based on the versions already in the window.
    fn take_branches(&mut self) {
        let versions: Vec<Hash> = self.versions.iter().copied().collect();

        for version in versions {
            let branches = self.snapshot.variants_based_on(version).to_vec();

            for branch in branches {
                self.take_variant(branch);
            }
        }
    }

    /// Lays the walk out as a view.
    fn view(self, anchors: Vec<Anchor>) -> GraphView {
        let Self {
            snapshot,
            nodes,
            versions,
            missing,
            truncated,
            ..
        } = self;

        let mut edges = BTreeSet::new();
        let mut laid = Vec::with_capacity(nodes.len());

        for hash in &nodes {
            let Some(record) = snapshot.variant(*hash) else {
                continue;
            };

            let base = snapshot.version(record.base_version).and_then(|base| {
                (!base.is_root && snapshot.variant(base.variant).is_some()).then_some(base.variant)
            });

            if let Some(base) = base
                && nodes.contains(&base)
            {
                edges.insert((*hash, base, GraphEdgeKind::Base));
            }
            if let Some(join) = record.join
                && nodes.contains(&join)
            {
                edges.insert((*hash, join, GraphEdgeKind::Join));
            }

            let parents = base.is_none_or(|base| !nodes.contains(&base));
            let joined = record.join.is_some_and(|join| !nodes.contains(&join));
            let children = snapshot.versions_pinning(*hash).iter().any(|version| {
                snapshot
                    .variants_based_on(*version)
                    .iter()
                    .any(|child| !nodes.contains(child))
            });

            laid.push(GraphNode {
                hash: *hash,
                base_version: record.base_version,
                base_variant: base,
                join: record.join,
                storage: record.storage,
                creator: record.creator,
                message: record.message,
                frontier: Frontier {
                    parents,
                    children,
                    joined,
                },
            });
        }

        let versions = versions
            .into_iter()
            .filter_map(|hash| {
                snapshot.version(hash).map(|record| GraphVersion {
                    hash,
                    variant: record.variant,
                    number: record.number,
                    is_root: record.is_root,
                })
            })
            .collect();

        GraphView {
            anchors,
            nodes: laid,
            versions,
            edges: edges
                .into_iter()
                .map(|(from, to, kind)| GraphEdge { from, to, kind })
                .collect(),
            missing: missing.into_iter().collect(),
            truncated,
        }
    }
}

#[cfg(test)]
mod tests {
    use rorolala_vcs::{Hash, ROOT_VERSION};

    use super::{ExpandDirection, ExpandRequest, GraphEdgeKind, GraphRequest, MissingKind};
    use crate::{Edge, Missing, Snapshot, Table};

    /// A chain of three variants, with a branch from the second version and a merge onto the third.
    ///
    /// The root version is `root`; the first variant is based on it and fixed by `first`; the second
    /// variant is based on `first` and fixed by `second`; the third is based on `second` and fixed by
    /// `third`. `branch` is based on `first` as well, so it is a sibling of the second variant, and
    /// `merge` is based on `third` and merges `foreign` — a variant of another chain — in.
    struct Chain {
        /// The root version.
        root: Hash,
        /// The first variant, and the version fixing it.
        first_variant: Hash,
        first: Hash,
        /// The second variant, and the version fixing it.
        second_variant: Hash,
        second: Hash,
        /// The third variant, and the version fixing it.
        third_variant: Hash,
        third: Hash,
        /// The variant based on the first version as well.
        branch: Hash,
        /// A variant based on the root alone, merged in from another chain.
        foreign: Hash,
        /// The variant based on the third version, merging the foreign one in.
        merge: Hash,
        /// The version fixing the merge.
        merge_version: Hash,
    }

    /// Notes the records of one variant.
    fn variant(table: &mut Table, hash: Hash, base: Hash, join: Option<Hash>) {
        table.add(Hash::new([0x10; 32]), hash, Edge::Storage);
        table.add(base, hash, Edge::Base);
        table.add(Hash::new([0x11; 32]), hash, Edge::Creator);
        table.add(Hash::new([0x12; 32]), hash, Edge::Message);

        if let Some(join) = join {
            table.add(join, hash, Edge::Join);
        }
    }

    /// The records of the chain, with numbers filled as a rebuild would fill them.
    fn chain() -> (Table, Chain) {
        let chain = Chain {
            root: Hash::new([0x01; 32]),
            first_variant: Hash::new([0x02; 32]),
            first: Hash::new([0x03; 32]),
            second_variant: Hash::new([0x04; 32]),
            second: Hash::new([0x05; 32]),
            third_variant: Hash::new([0x06; 32]),
            third: Hash::new([0x07; 32]),
            branch: Hash::new([0x08; 32]),
            foreign: Hash::new([0x09; 32]),
            merge: Hash::new([0x0a; 32]),
            merge_version: Hash::new([0x0b; 32]),
        };

        let mut table = Table::new(vec![]);
        variant(&mut table, chain.first_variant, chain.root, None);
        variant(&mut table, chain.second_variant, chain.first, None);
        variant(&mut table, chain.third_variant, chain.second, None);
        variant(&mut table, chain.branch, chain.first, None);
        variant(&mut table, chain.foreign, chain.root, None);
        variant(&mut table, chain.merge, chain.third, Some(chain.foreign));

        table.add(chain.first_variant, chain.first, Edge::Variant);
        table.add(chain.second_variant, chain.second, Edge::Variant);
        table.add(chain.third_variant, chain.third, Edge::Variant);
        table.add(chain.merge, chain.merge_version, Edge::Variant);
        table.add(Hash::new([0x00; 32]), chain.root, Edge::Variant);
        table.set_number(chain.root, ROOT_VERSION);
        table.set_number(chain.first, 0);
        table.set_number(chain.second, 1);
        table.set_number(chain.third, 2);
        table.set_number(chain.merge_version, 3);

        (table, chain)
    }

    /// The snapshot of the chain.
    fn snapshot() -> (Snapshot, Chain) {
        let (table, chain) = chain();
        let snapshot =
            Snapshot::from_table(&table).expect("the records are the ones this build writes");

        (snapshot, chain)
    }

    #[test]
    fn a_window_holds_the_chain_behind_its_focus() {
        let (snapshot, chain) = snapshot();
        let view = snapshot.graph(&GraphRequest {
            focus: vec![chain.second],
            branches: false,
            joins: false,
            ..GraphRequest::default()
        });

        assert_eq!(view.anchors.len(), 1);
        assert_eq!(view.anchors[0].version, chain.second);
        assert_eq!(view.anchors[0].variant, Some(chain.second_variant));

        // The chain from the second version back: the second variant, the first and the root.
        let mut nodes: Vec<Hash> = view.nodes.iter().map(|node| node.hash).collect();
        nodes.sort_unstable();
        assert_eq!(nodes, [chain.first_variant, chain.second_variant]);
        assert_eq!(
            view.edges
                .iter()
                .map(|edge| (edge.from, edge.to, edge.kind))
                .collect::<Vec<_>>(),
            vec![(
                chain.second_variant,
                chain.first_variant,
                GraphEdgeKind::Base
            )]
        );

        // Both the versions fixing the variants in the window and the root are in the answer.
        let versions: Vec<Hash> = view.versions.iter().map(|version| version.hash).collect();
        assert!(versions.contains(&chain.root));
        assert!(versions.contains(&chain.first));
        assert!(versions.contains(&chain.second));
        assert_eq!(
            view.versions
                .iter()
                .find(|version| version.hash == chain.root)
                .map(|version| (version.is_root, version.number)),
            Some((true, None))
        );
        assert_eq!(
            view.versions
                .iter()
                .find(|version| version.hash == chain.second)
                .map(|version| version.number),
            Some(Some(1))
        );

        // The first variant is where the chain starts in the window, and the second has a version
        // the chain goes on from.
        let first = view
            .nodes
            .iter()
            .find(|node| node.hash == chain.first_variant)
            .expect("the first variant is in the window");
        assert!(first.frontier.parents);
        let second = view
            .nodes
            .iter()
            .find(|node| node.hash == chain.second_variant)
            .expect("the second variant is in the window");
        assert!(!second.frontier.parents);
        assert!(second.frontier.children);
        assert!(!view.truncated);
    }

    #[test]
    fn a_branch_is_taken_in_one_hop() {
        let (snapshot, chain) = snapshot();
        let view = snapshot.graph(&GraphRequest {
            focus: vec![chain.second],
            joins: false,
            ..GraphRequest::default()
        });

        // The branch is based on the first version, which is in the window, so it is one hop in.
        assert!(
            view.nodes.iter().any(|node| node.hash == chain.branch),
            "the branch based on a version in the window is not in it",
        );

        // The third variant is forward of the second and is one hop as well.
        assert!(
            view.nodes
                .iter()
                .any(|node| node.hash == chain.third_variant)
        );
    }

    #[test]
    fn a_merge_brings_in_the_chain_it_merges() {
        let (snapshot, chain) = snapshot();
        let view = snapshot.graph(&GraphRequest {
            focus: vec![chain.merge_version],
            branches: false,
            ..GraphRequest::default()
        });

        let nodes: Vec<Hash> = view.nodes.iter().map(|node| node.hash).collect();
        assert!(nodes.contains(&chain.merge));
        assert!(nodes.contains(&chain.foreign));
        assert_eq!(
            view.edges
                .iter()
                .filter(|edge| edge.kind == GraphEdgeKind::Join)
                .map(|edge| (edge.from, edge.to))
                .collect::<Vec<_>>(),
            vec![(chain.merge, chain.foreign)]
        );

        let merge = view
            .nodes
            .iter()
            .find(|node| node.hash == chain.merge)
            .expect("the merge is in the window");
        assert!(!merge.frontier.joined);

        // A window that asked for no joins leaves the other chain out, and says the merge is there.
        let unjoined = snapshot.graph(&GraphRequest {
            focus: vec![chain.merge_version],
            joins: false,
            branches: false,
            ..GraphRequest::default()
        });
        assert!(!unjoined.nodes.iter().any(|node| node.hash == chain.foreign));
        let merge = unjoined
            .nodes
            .iter()
            .find(|node| node.hash == chain.merge)
            .expect("the merge is in the window");
        assert!(merge.frontier.joined);
    }

    #[test]
    fn a_version_the_index_does_not_hold_is_named() {
        let (snapshot, _) = snapshot();
        let missing = Hash::new([0xee; 32]);
        let view = snapshot.graph(&GraphRequest {
            focus: vec![missing],
            ..GraphRequest::default()
        });

        assert_eq!(view.anchors.len(), 1);
        assert_eq!(view.anchors[0].variant, None);
        assert_eq!(view.nodes, Vec::new());
        assert_eq!(
            view.missing,
            vec![Missing {
                hash: missing,
                kind: MissingKind::Version,
            }]
        );
    }

    #[test]
    fn a_window_stops_at_its_cap_and_says_so() {
        let (snapshot, chain) = snapshot();
        let view = snapshot.graph(&GraphRequest {
            focus: vec![chain.third],
            back: 64,
            forward: 0,
            joins: false,
            branches: false,
            max_nodes: 1,
        });

        assert!(view.truncated);
        assert_eq!(view.nodes.len(), 1);

        let node = &view.nodes[0];
        assert_eq!(node.hash, chain.third_variant);
        assert!(node.frontier.parents);
    }

    #[test]
    fn a_walk_expands_one_way_from_its_node() {
        let (snapshot, chain) = snapshot();

        let parents = snapshot.expand(&ExpandRequest {
            node: chain.third_variant,
            direction: ExpandDirection::Parents,
            depth: 64,
            max_nodes: 400,
        });
        assert!(
            parents
                .nodes
                .iter()
                .any(|node| node.hash == chain.first_variant)
        );
        assert!(
            parents
                .nodes
                .iter()
                .any(|node| node.hash == chain.third_variant),
            "a walk takes the node it walks from in as well",
        );
        assert!(
            !parents.nodes.iter().any(|node| node.hash == chain.merge),
            "a parents walk takes in nothing forward of the node",
        );

        let children = snapshot.expand(&ExpandRequest {
            node: chain.second_variant,
            direction: ExpandDirection::Children,
            depth: 1,
            max_nodes: 400,
        });
        assert!(
            children
                .nodes
                .iter()
                .any(|node| node.hash == chain.third_variant)
        );
        assert!(
            !children
                .nodes
                .iter()
                .any(|node| node.hash == chain.first_variant),
            "a children walk does not take in what came before",
        );

        let joined = snapshot.expand(&ExpandRequest {
            node: chain.merge,
            direction: ExpandDirection::Joined,
            depth: 64,
            max_nodes: 400,
        });
        assert!(joined.nodes.iter().any(|node| node.hash == chain.foreign));
    }
}
