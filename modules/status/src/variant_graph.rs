//! The variant graph as the Desktop draws it: the graph itself, the entries that name it, and the
//! texts of the variants in it.
//!
//! The inverse index answers what the graph is, which variants there are, how they are based on one
//! another and what they merge in, and knows nothing about where the work stands. This adds what only
//! the workspace knows: the entries the window holds, from the Layout being worked in and from every
//! fetched copy; the merge a file is in the middle of; and the words of the creators and messages the
//! window's variants name.
//!
//! The request is a window, not a file: it names the entries it is centred on and how far to look, and
//! the answer is that part of the graph with a `frontier` on each node where it stops. A caller that
//! reaches the edge asks again from the node it reached. The whole answer crosses as one handle (the
//! graph, the entries, the merging records and the texts) because it is read as one picture, and one
//! handle is one crossing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use rorolala_inverse_index::{
    GraphEdgeKind, GraphNode, GraphRequest, GraphView, InverseIndex, MissingKind, Snapshot,
};
use rorolala_layout::{Layout, LayoutPath, MutableData};
use rorolala_storage::Key;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{Hash, VCSIndex, VCSIndexObject};
use rorolala_workspace::{Merging, Workspace};
use uuid::Uuid;

use crate::existence::{Existence, Target, local};

/// The side the Layout being worked in is named on.
const LOCAL: &str = "local";

/// The path an entry is at when it has been deprecated: the marker the Vault's own moves put it under.
const DEPRECATED_PREFIX: &str = "@/removed/";

/// The variant graph around the entries `focus` names.
///
/// The window is walked over the inverse index's records, rebuilt first when they do not describe the
/// index, so what is drawn is what the index holds now, no fetch and no network. What only the
/// workspace knows is added to the answer: the entries the window holds, from the Layout being worked
/// in and from every fetched copy, each with the side it is named on; the merges in progress; and the
/// text of each creator and message the window's variants name. A key the index does not hold is
/// answered as missing rather than fetched.
///
/// The graph's window is `window`'s: `focus` names the entries to stand on, each a `side` and a
/// `path`, a side being the Layout being worked in or the name of a Vault whose copy was fetched, and
/// an empty `focus` stands on every entry of the Layout being worked in that records a version. The
/// rest of `window` is the walk's, as the graph reads it.
///
/// # FFI
///
/// `directory` is the directory the Workspace is searched upwards from. `window` and `focus` are the
/// caller's, borrowed for the call and released by the caller, and `focus_len` is how many entries
/// `focus` holds. A `RolaVariantGraph *` the caller releases with `free_rola_variant_graph` answers;
/// a failure is a `RolaVariantGraphError *` the caller releases with `free_rola_variant_graph_error`.
///
/// # Errors
///
/// Returns [`GraphError::NoWorkspace`] when no Workspace holds `directory`,
/// [`GraphError::NoLayout`] when the Workspace works in no Layout, [`GraphError::NoIndex`] when it
/// holds no version control index, and [`GraphError::Read`] when the index or the records beside it
/// cannot be read.
#[lazyffi(export = rola_variant_graph_locate)]
pub fn locate_variant_graph(
    directory: &Path,
    window: &VariantGraphWindow,
    focus: &[VariantGraphFocus],
) -> Result<VariantGraph, GraphError> {
    let workspace = Workspace::locate(directory).ok_or(GraphError::NoWorkspace)?;
    let layout = workspace
        .layout_readonly()
        .map_err(|error| GraphError::Read {
            reason: error.to_string(),
        })?
        .ok_or(GraphError::NoLayout)?;
    let index = VCSIndex::locate(directory).ok_or(GraphError::NoIndex)?;

    let runtime = tokio::runtime::Runtime::new().map_err(|error| GraphError::Read {
        reason: error.to_string(),
    })?;
    let inverse = InverseIndex::at(index.clone());
    let snapshot = snapshot_of(&runtime, &inverse)?;
    let sides = Sides::open(&workspace, layout);
    let entries = sides.entries();
    let focused = sides.focused(focus, &entries);
    let versions: Vec<Hash> = focused
        .iter()
        .filter_map(|focused| focused.version)
        .collect();
    let view = snapshot.graph(&window.graph(versions));
    let (creators, messages) = texts_of(&runtime, &index, &view);
    let merging = merging_of(&workspace, &sides.local);
    let focus = focus_of(&view, focused);

    // A node's content is the content the variant's `storage` field names, so what the graph has
    // already read is what the local check asks the store about: one lookup for the whole window,
    // not one per node.
    let targets: Vec<Target> = view
        .nodes
        .iter()
        .map(|node| Target::Storage(node.storage))
        .collect();
    let set = workspace.storage_set().map_err(|error| GraphError::Read {
        reason: error.to_string(),
    })?;
    let held = runtime.block_on(local(&sides.local, &index, Some(&set), &targets));

    // Only the entries the window can name are carried: a workspace may hold thousands, and the ones
    // the window does not draw are the listing's business rather than the graph's.
    let named: BTreeSet<Hash> = view.versions.iter().map(|version| version.hash).collect();
    let entries: Vec<&Entry> = entries
        .iter()
        .filter(|entry| {
            entry
                .version()
                .is_some_and(|version| named.contains(&version))
        })
        .collect();

    Ok(VariantGraph {
        focus,
        nodes: view
            .nodes
            .iter()
            .zip(&held)
            .map(|(node, existence)| node_of(node, existence))
            .collect(),
        versions: view
            .versions
            .iter()
            .map(|version| VariantGraphVersion {
                hash: version.hash.hex(),
                variant: version.variant.hex(),
                number_present: version.number.is_some(),
                number: version.number.unwrap_or_default(),
                is_root: version.is_root,
            })
            .collect(),
        edges: view
            .edges
            .iter()
            .map(|edge| VariantGraphEdge {
                from: edge.from.hex(),
                to: edge.to.hex(),
                kind: edge_kind(edge.kind).to_owned(),
            })
            .collect(),
        missing: view
            .missing
            .iter()
            .map(|missing| VariantGraphMissing {
                hash: missing.hash.hex(),
                kind: missing_kind(missing.kind).to_owned(),
            })
            .collect(),
        truncated: view.truncated,
        entries: entries.iter().map(|entry| entry_of(entry)).collect(),
        merging,
        creators,
        messages,
    })
}

/// What a variant graph is asked for besides its entries.
///
/// # FFI
///
/// A `RolaVariantGraphWindow` the caller fills. Every field is the walk's, as the graph reads it; a
/// caller that leaves them zero draws nothing rather than taking a default, so it writes the window
/// it wants.
#[lazyffi(export = RolaVariantGraphWindow, input)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VariantGraphWindow {
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

impl VariantGraphWindow {
    /// The window the graph is walked with, standing on `focus`.
    const fn graph(&self, focus: Vec<Hash>) -> GraphRequest {
        GraphRequest {
            focus,
            back: self.back,
            forward: self.forward,
            joins: self.joins,
            branches: self.branches,
            max_nodes: self.max_nodes,
        }
    }
}

/// The side an entry a graph is centred on is named on.
///
/// # FFI
///
/// A `RolaVariantGraphSide` the caller fills: `tag` selects the variant and the payload companion it
/// names holds the Vault's name. The name is borrowed for the call and released by the caller.
#[lazyffi(export = RolaVariantGraphSide)]
#[derive(Debug, Clone)]
pub enum VariantGraphSide {
    /// The Layout the Workspace works in.
    Local,
    /// The copy a fetch left of a Vault's Layout, named by the Vault it is of.
    Vault {
        /// The Vault whose copy the entry is read from.
        name: String,
    },
}

impl VariantGraphSide {
    /// The name the side is written by: `local` or the Vault's own.
    fn name(&self) -> &str {
        match self {
            Self::Local => LOCAL,
            Self::Vault { name } => name,
        }
    }
}

/// One entry a graph is centred on: the side it is named on, and the path it is at.
///
/// # FFI
///
/// A `RolaVariantGraphFocus` the caller fills, whose strings and Vault name are borrowed for the
/// call and released by the caller. `focus` and `focus_len` on `rola_variant_graph_locate` are an
/// array of these.
#[lazyffi(export = RolaVariantGraphFocus, input)]
#[derive(Debug, Clone)]
pub struct VariantGraphFocus {
    /// The side the entry is named on.
    pub side: VariantGraphSide,
    /// The path the entry is at, as the Layout names it.
    pub path: String,
}

/// The records describing the index, made to describe it first when they do not.
fn snapshot_of(
    runtime: &tokio::runtime::Runtime,
    inverse: &InverseIndex,
) -> Result<Snapshot, GraphError> {
    if let Ok(Some(snapshot)) = runtime.block_on(inverse.snapshot()) {
        return Ok(snapshot);
    }

    // The records are derived data: what does not describe the index is brought up to date rather than
    // read around, and a rebuild of nothing new reads nothing.
    runtime
        .block_on(inverse.rebuild())
        .map_err(|error| GraphError::Read {
            reason: error.to_string(),
        })?;

    runtime
        .block_on(inverse.snapshot())
        .map_err(|error| GraphError::Read {
            reason: error.to_string(),
        })?
        .ok_or_else(|| GraphError::Read {
            reason: "the records do not describe the index after a rebuild".to_owned(),
        })
}

/// The Layouts a variant graph is read against: the one being worked in, and a copy per fetched Vault.
struct Sides {
    /// The Layout the Workspace works in.
    local: Layout,
    /// The fetched copies, by the Vault each is of, in name order.
    copies: Vec<(String, Layout)>,
}

impl Sides {
    /// The sides open for `workspace`, whose working Layout is `local`.
    fn open(workspace: &Workspace, local: Layout) -> Self {
        let mut copies = Vec::new();

        for (vault, name) in workspace.readonly_layout_copies() {
            // Only a Vault's own Layout is fetched; a copy of anything else is not one the graph has a
            // side for, so it is left out rather than named by a side nothing else uses.
            if name != VAULT_LAYOUT_NAME {
                continue;
            }
            let Some(dir) = workspace.readonly_layout_copy(&vault, VAULT_LAYOUT_NAME) else {
                continue;
            };
            let Ok(layout) = Layout::open_existing(dir) else {
                continue;
            };

            copies.push((vault, layout));
        }

        copies.sort_by(|left, right| left.0.cmp(&right.0));

        Self { local, copies }
    }

    /// The Layout the side `side` is read from, when it is one of these sides.
    fn named(&self, side: &str) -> Option<&Layout> {
        if side == LOCAL {
            return Some(&self.local);
        }

        self.copies
            .iter()
            .find(|(vault, _)| vault == side)
            .map(|(_, layout)| layout)
    }

    /// Every entry of every side: the work's own first, then a Vault's, each in path order.
    fn entries(&self) -> Vec<Entry> {
        let mut entries = side_entries(LOCAL, &self.local);

        for (vault, layout) in &self.copies {
            entries.extend(side_entries(vault, layout));
        }

        entries
    }

    /// What each entry a request stands on resolves to.
    fn focused(&self, requested: &[VariantGraphFocus], entries: &[Entry]) -> Vec<Focused> {
        if requested.is_empty() {
            return entries
                .iter()
                .filter(|entry| entry.side == LOCAL && entry.version().is_some())
                .map(|entry| Focused {
                    side: entry.side.clone(),
                    path: entry.path.as_str().to_owned(),
                    version: entry.version(),
                })
                .collect();
        }

        requested
            .iter()
            .map(|focus| self.focused_one(focus))
            .collect()
    }

    /// What one entry a request stands on resolves to.
    ///
    /// A side or a path this reading does not hold is answered as an entry that records no version:
    /// the work may have moved on since the request was written, and a window that cannot be centred
    /// is told so rather than answered from somewhere else.
    fn focused_one(&self, focus: &VariantGraphFocus) -> Focused {
        let version = self
            .named(focus.side.name())
            .and_then(|layout| {
                LayoutPath::from_stored(&focus.path)
                    .ok()
                    .and_then(|path| layout.data_of(&path))
            })
            .and_then(|data| (data.version() != [0; 32]).then(|| Key::new(data.version())));

        Focused {
            side: focus.side.name().to_owned(),
            path: focus.path.clone(),
            version,
        }
    }
}

/// Every entry of one side, in path order.
fn side_entries(side: &str, layout: &Layout) -> Vec<Entry> {
    let mut entries: Vec<Entry> = layout
        .entries()
        .into_iter()
        .filter_map(|(id, data)| {
            let path = layout.path_of(id)?;

            Some(Entry {
                side: side.to_owned(),
                path,
                id,
                data,
            })
        })
        .collect();

    entries.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));

    entries
}

/// One entry of a Layout, before it is answered as one of the graph's own.
struct Entry {
    /// The side it is named on.
    side: String,
    /// The path it is at.
    path: LayoutPath,
    /// The `Uuid` it is held by.
    id: Uuid,
    /// What the Layout keeps for it.
    data: MutableData,
}

impl Entry {
    /// The version it records, when it records one.
    fn version(&self) -> Option<Hash> {
        (self.data.version() != [0; 32]).then(|| Key::new(self.data.version()))
    }
}

/// What an entry a request stands on resolved to, before the graph answers.
struct Focused {
    /// The side it is named on.
    side: String,
    /// The path it is at.
    path: String,
    /// The version it records, when it records one.
    version: Option<Hash>,
}

/// The focus entries with the variants the graph resolved them to.
fn focus_of(view: &GraphView, focused: Vec<Focused>) -> Vec<VariantGraphFocusResolution> {
    let anchors: BTreeMap<Hash, Option<Hash>> = view
        .anchors
        .iter()
        .map(|anchor| (anchor.version, anchor.variant))
        .collect();

    focused
        .into_iter()
        .map(|focused| VariantGraphFocusResolution {
            side: focused.side,
            path: focused.path,
            variant: focused
                .version
                .and_then(|version| anchors.get(&version).copied().flatten())
                .map_or_else(String::new, |variant| variant.hex()),
            version: focused
                .version
                .map_or_else(String::new, |version| version.hex()),
        })
        .collect()
}

/// One variant, with what the store says about its content.
fn node_of(node: &GraphNode, existence: &Existence) -> VariantGraphNode {
    VariantGraphNode {
        hash: node.hash.hex(),
        base_version: node.base_version.hex(),
        base_variant: node
            .base_variant
            .map_or_else(String::new, |variant| variant.hex()),
        join: node.join.map_or_else(String::new, |join| join.hex()),
        storage: node.storage.hex(),
        offline: existence.held,
        creator: node.creator.hex(),
        message: node.message.hex(),
        frontier_parents: node.frontier.parents,
        frontier_children: node.frontier.children,
        frontier_joined: node.frontier.joined,
    }
}

/// One entry of one side, as the answer carries it.
fn entry_of(entry: &Entry) -> VariantGraphEntry {
    let path = entry.path.as_str();

    VariantGraphEntry {
        side: entry.side.clone(),
        path: path.to_owned(),
        uuid: entry.id.to_string(),
        version: entry
            .version()
            .map_or_else(String::new, |version| version.hex()),
        owner: entry.data.owner().unwrap_or_default().to_owned(),
        deprecated: path.starts_with(DEPRECATED_PREFIX),
    }
}

/// The merges in progress.
fn merging_of(workspace: &Workspace, layout: &Layout) -> Vec<VariantGraphMerging> {
    let merging = Merging::read(workspace.get_root());

    merging
        .iter()
        .map(|pending| VariantGraphMerging {
            variant: Key::new(*pending.variant()).hex(),
            target: pending.target().to_string(),
            path: layout
                .path_of(pending.target())
                .map_or_else(String::new, |path| path.as_str().to_owned()),
            file: merging
                .variant_path(layout, pending)
                .map_or_else(String::new, |path| path.as_str().to_owned()),
        })
        .collect()
}

/// The text of every creator and message the window's variants name.
///
/// A key the index does not hold is left out rather than answered as an empty text: what is not there
/// has no words, and a caller drawing the hash instead is drawing what is known. Nothing is fetched.
fn texts_of(
    runtime: &tokio::runtime::Runtime,
    index: &VCSIndex,
    view: &GraphView,
) -> (Vec<VariantGraphText>, Vec<VariantGraphText>) {
    let mut creators = Vec::new();
    let mut messages = Vec::new();
    let wanted: BTreeSet<Hash> = view
        .nodes
        .iter()
        .flat_map(|node| [node.creator, node.message])
        .collect();

    for hash in wanted {
        match runtime.block_on(index.read(hash)) {
            Ok(VCSIndexObject::Creator(creator)) => creators.push(VariantGraphText {
                hash: hash.hex(),
                text: creator.read_to_string().to_owned(),
            }),
            Ok(VCSIndexObject::Message(message)) => messages.push(VariantGraphText {
                hash: hash.hex(),
                text: message.read_to_string().to_owned(),
            }),
            _ => {}
        }
    }

    (creators, messages)
}

/// What an edge between two variants is.
const fn edge_kind(kind: GraphEdgeKind) -> &'static str {
    match kind {
        GraphEdgeKind::Base => "base",
        GraphEdgeKind::Join => "join",
    }
}

/// What kind a key the index does not hold would be.
const fn missing_kind(kind: MissingKind) -> &'static str {
    match kind {
        MissingKind::Version => "version",
        MissingKind::Variant => "variant",
    }
}

/// The variant graph, the entries that name it, and the texts in it.
///
/// # FFI
///
/// A `RolaVariantGraph *` the caller releases with `free_rola_variant_graph`. `_focus_count`
/// and `_focus_at`, `_node_count` and `_node_at`, `_version_count` and `_version_at`,
/// `_edge_count` and `_edge_at`, `_missing_count` and `_missing_at`, `_entry_count` and
/// `_entry_at`, `_merging_count` and `_merging_at`, `_creator_count` and `_creator_at`, and
/// `_message_count` and `_message_at` answer the lists. Each item is owned by the caller and released
/// with the `free_` its type names.
#[lazyffi(export = RolaVariantGraph)]
#[derive(Debug, Default)]
pub struct VariantGraph {
    /// What each entry the request stood on resolved to, in the order asked.
    focus: Vec<VariantGraphFocusResolution>,
    /// The variants in the window, in key order.
    nodes: Vec<VariantGraphNode>,
    /// The versions fixing them, in key order.
    versions: Vec<VariantGraphVersion>,
    /// The edges between variants in the window.
    edges: Vec<VariantGraphEdge>,
    /// The keys the window names but the index does not hold.
    missing: Vec<VariantGraphMissing>,
    /// Whether the window was cut short by its cap rather than walked out.
    truncated: bool,
    /// The entries whose version the window holds, the work's own first.
    entries: Vec<VariantGraphEntry>,
    /// The merges in progress.
    merging: Vec<VariantGraphMerging>,
    /// The text of each creator the window's variants name.
    creators: Vec<VariantGraphText>,
    /// The text of each message the window's variants name.
    messages: Vec<VariantGraphText>,
}

#[lazyffi(export = rola_)]
impl VariantGraph {
    /// How many entries the graph was centred on.
    #[must_use]
    pub const fn focus_count(&self) -> usize {
        self.focus.len()
    }

    /// What the entry at `index` resolved to, or null past the end.
    #[must_use]
    pub fn focus_at(&self, index: usize) -> Option<VariantGraphFocusResolution> {
        self.focus.get(index).cloned()
    }

    /// How many variants the window holds.
    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The variant at `index`, or null past the end.
    #[must_use]
    pub fn node_at(&self, index: usize) -> Option<VariantGraphNode> {
        self.nodes.get(index).cloned()
    }

    /// How many versions fix the variants in the window.
    #[must_use]
    pub const fn version_count(&self) -> usize {
        self.versions.len()
    }

    /// The version at `index`, or null past the end.
    #[must_use]
    pub fn version_at(&self, index: usize) -> Option<VariantGraphVersion> {
        self.versions.get(index).cloned()
    }

    /// How many edges the window holds.
    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// The edge at `index`, or null past the end.
    #[must_use]
    pub fn edge_at(&self, index: usize) -> Option<VariantGraphEdge> {
        self.edges.get(index).cloned()
    }

    /// How many keys the window names that the index does not hold.
    #[must_use]
    pub const fn missing_count(&self) -> usize {
        self.missing.len()
    }

    /// The missing key at `index`, or null past the end.
    #[must_use]
    pub fn missing_at(&self, index: usize) -> Option<VariantGraphMissing> {
        self.missing.get(index).cloned()
    }

    /// Whether the window was cut short by its cap rather than walked out.
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// How many entries the window names.
    #[must_use]
    pub const fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// The entry at `index`, or null past the end.
    #[must_use]
    pub fn entry_at(&self, index: usize) -> Option<VariantGraphEntry> {
        self.entries.get(index).cloned()
    }

    /// How many merges are in progress.
    #[must_use]
    pub const fn merging_count(&self) -> usize {
        self.merging.len()
    }

    /// The merge at `index`, or null past the end.
    #[must_use]
    pub fn merging_at(&self, index: usize) -> Option<VariantGraphMerging> {
        self.merging.get(index).cloned()
    }

    /// How many creators the window's variants name.
    #[must_use]
    pub const fn creator_count(&self) -> usize {
        self.creators.len()
    }

    /// The creator's text at `index`, or null past the end.
    #[must_use]
    pub fn creator_at(&self, index: usize) -> Option<VariantGraphText> {
        self.creators.get(index).cloned()
    }

    /// How many messages the window's variants name.
    #[must_use]
    pub const fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// The message's text at `index`, or null past the end.
    #[must_use]
    pub fn message_at(&self, index: usize) -> Option<VariantGraphText> {
        self.messages.get(index).cloned()
    }
}

/// What an entry the request stood on resolved to.
///
/// # FFI
///
/// A `RolaVariantGraphFocusResolution *` the caller releases with
/// `free_rola_variant_graph_focus_resolution`. `_side`, `_path`, `_variant` and `_version` each
/// return a `char *` the caller releases with `free_string`; `_variant` and `_version` are empty
/// when the graph resolved none.
#[lazyffi(export = RolaVariantGraphFocusResolution)]
#[derive(Debug, Clone)]
pub struct VariantGraphFocusResolution {
    /// The side it is named on.
    side: String,
    /// The path it is at.
    path: String,
    /// The variant the version it records fixes, or an empty string.
    variant: String,
    /// The version it records, or an empty string.
    version: String,
}

#[lazyffi(export = rola_)]
impl VariantGraphFocusResolution {
    /// The side it is named on: `local` or the Vault's own name.
    #[must_use]
    pub fn side(&self) -> String {
        self.side.clone()
    }

    /// The path it is at.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The variant the version it records fixes, or an empty string.
    #[must_use]
    pub fn variant(&self) -> String {
        self.variant.clone()
    }

    /// The version it records, or an empty string.
    #[must_use]
    pub fn version(&self) -> String {
        self.version.clone()
    }
}

/// One variant in the window.
///
/// # FFI
///
/// A `RolaVariantGraphNode *` the caller releases with `free_rola_variant_graph_node`. `_hash`,
/// `_base_version`, `_base_variant`, `_join`, `_storage`, `_creator` and `_message` each return a
/// `char *` the caller releases with `free_string`; `_base_variant` and `_join` are empty when the
/// index holds no such variant. `_frontier_parents`, `_frontier_children` and `_frontier_joined` say
/// where the window stops short of what this node names.
#[lazyffi(export = RolaVariantGraphNode)]
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is one frontier of one node, and a caller reads each on its own"
)]
pub struct VariantGraphNode {
    /// The variant's own key, as hex.
    hash: String,
    /// The version it is based on, as hex.
    base_version: String,
    /// The variant that version fixes, or an empty string when the index does not hold it.
    base_variant: String,
    /// The variant it merges in, or an empty string when it is no merge.
    join: String,
    /// The content it was made of, as the store's key in hex.
    storage: String,
    /// Whether every object the content needs is held here.
    offline: bool,
    /// Who made it, as the key of the Creator index object.
    creator: String,
    /// What it says was done, as the key of the Message index object.
    message: String,
    /// The base variant, or the version it is based on, is not in the window.
    frontier_parents: bool,
    /// A variant based on one of the node's versions is not in the window.
    frontier_children: bool,
    /// A variant the node merges in is not in the window.
    frontier_joined: bool,
}

#[lazyffi(export = rola_)]
impl VariantGraphNode {
    /// The variant's own key, as hex.
    #[must_use]
    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    /// The version it is based on, as hex.
    #[must_use]
    pub fn base_version(&self) -> String {
        self.base_version.clone()
    }

    /// The variant that version fixes, or an empty string when the index does not hold it.
    #[must_use]
    pub fn base_variant(&self) -> String {
        self.base_variant.clone()
    }

    /// The variant it merges in, or an empty string when it is no merge.
    #[must_use]
    pub fn join(&self) -> String {
        self.join.clone()
    }

    /// The content it was made of, as the store's key in hex.
    #[must_use]
    pub fn storage(&self) -> String {
        self.storage.clone()
    }

    /// Whether every object the content needs is held here.
    #[must_use]
    pub const fn offline(&self) -> bool {
        self.offline
    }

    /// Who made it, as the key of the Creator index object.
    #[must_use]
    pub fn creator(&self) -> String {
        self.creator.clone()
    }

    /// What it says was done, as the key of the Message index object.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    /// The base variant, or the version it is based on, is not in the window.
    #[must_use]
    pub const fn frontier_parents(&self) -> bool {
        self.frontier_parents
    }

    /// A variant based on one of the node's versions is not in the window.
    #[must_use]
    pub const fn frontier_children(&self) -> bool {
        self.frontier_children
    }

    /// A variant the node merges in is not in the window.
    #[must_use]
    pub const fn frontier_joined(&self) -> bool {
        self.frontier_joined
    }
}

/// One version fixing a variant in the window.
///
/// # FFI
///
/// A `RolaVariantGraphVersion *` the caller releases with `free_rola_variant_graph_version`. `_hash`
/// and `_variant` each return a `char *` the caller releases with `free_string`. `_number` says
/// nothing unless `_number_present` is true, which is false for the root and for a cut-off chain.
#[lazyffi(export = RolaVariantGraphVersion)]
#[derive(Debug, Clone)]
pub struct VariantGraphVersion {
    /// The version's own key, as hex.
    hash: String,
    /// The variant it fixes, as hex; the zero hash for the root.
    variant: String,
    /// Whether its number in its chain is known.
    number_present: bool,
    /// Its number in its chain, or 0 when it is not known.
    number: u64,
    /// Whether it is the root, numbered before the first.
    is_root: bool,
}

#[lazyffi(export = rola_)]
impl VariantGraphVersion {
    /// The version's own key, as hex.
    #[must_use]
    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    /// The variant it fixes, as hex; the zero hash for the root.
    #[must_use]
    pub fn variant(&self) -> String {
        self.variant.clone()
    }

    /// Whether its number in its chain is known.
    #[must_use]
    pub const fn number_present(&self) -> bool {
        self.number_present
    }

    /// Its number in its chain, or 0 when it is not known.
    #[must_use]
    pub const fn number(&self) -> u64 {
        self.number
    }

    /// Whether it is the root, numbered before the first.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.is_root
    }
}

/// One edge between two variants in the window.
///
/// # FFI
///
/// A `RolaVariantGraphEdge *` the caller releases with `free_rola_variant_graph_edge`. `_from`,
/// `_to` and `_kind` each return a `char *` the caller releases with `free_string`; `_kind` is
/// `base` or `join`.
#[lazyffi(export = RolaVariantGraphEdge)]
#[derive(Debug, Clone)]
pub struct VariantGraphEdge {
    /// The variant the edge is walked from.
    from: String,
    /// The variant it names.
    to: String,
    /// What the edge is: `base` or `join`.
    kind: String,
}

#[lazyffi(export = rola_)]
impl VariantGraphEdge {
    /// The variant the edge is walked from.
    #[must_use]
    pub fn from(&self) -> String {
        self.from.clone()
    }

    /// The variant it names.
    #[must_use]
    pub fn to(&self) -> String {
        self.to.clone()
    }

    /// What the edge is: `base` or `join`.
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }
}

/// One key the window names but the index does not hold.
///
/// # FFI
///
/// A `RolaVariantGraphMissing *` the caller releases with `free_rola_variant_graph_missing`. `_hash`
/// and `_kind` each return a `char *` the caller releases with `free_string`; `_kind` is `version` or
/// `variant`.
#[lazyffi(export = RolaVariantGraphMissing)]
#[derive(Debug, Clone)]
pub struct VariantGraphMissing {
    /// The key, as hex.
    hash: String,
    /// What it would have been: `version` or `variant`.
    kind: String,
}

#[lazyffi(export = rola_)]
impl VariantGraphMissing {
    /// The key, as hex.
    #[must_use]
    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    /// What it would have been: `version` or `variant`.
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }
}

/// One entry of one side.
///
/// # FFI
///
/// A `RolaVariantGraphEntry *` the caller releases with `free_rola_variant_graph_entry`. `_side`,
/// `_path`, `_uuid`, `_version` and `_owner` each return a `char *` the caller releases with
/// `free_string`; `_version` and `_owner` are empty when the entry records none.
#[lazyffi(export = RolaVariantGraphEntry)]
#[derive(Debug, Clone)]
pub struct VariantGraphEntry {
    /// The side it is named on.
    side: String,
    /// The path it is at.
    path: String,
    /// The `Uuid` it is held by.
    uuid: String,
    /// The version it records, or an empty string.
    version: String,
    /// The account that holds it, or an empty string.
    owner: String,
    /// Whether it was moved under the deprecated marker.
    deprecated: bool,
}

#[lazyffi(export = rola_)]
impl VariantGraphEntry {
    /// The side it is named on.
    #[must_use]
    pub fn side(&self) -> String {
        self.side.clone()
    }

    /// The path it is at.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The `Uuid` it is held by.
    #[must_use]
    pub fn uuid(&self) -> String {
        self.uuid.clone()
    }

    /// The version it records, or an empty string.
    #[must_use]
    pub fn version(&self) -> String {
        self.version.clone()
    }

    /// The account that holds it, or an empty string.
    #[must_use]
    pub fn owner(&self) -> String {
        self.owner.clone()
    }

    /// Whether it was moved under the deprecated marker.
    #[must_use]
    pub const fn deprecated(&self) -> bool {
        self.deprecated
    }
}

/// One merge in progress.
///
/// # FFI
///
/// A `RolaVariantGraphMerging *` the caller releases with `free_rola_variant_graph_merging`. `_variant`,
/// `_target`, `_path` and `_file` each return a `char *` the caller releases with `free_string`;
/// `_path` is empty when the Layout no longer names the file.
#[lazyffi(export = RolaVariantGraphMerging)]
#[derive(Debug, Clone)]
pub struct VariantGraphMerging {
    /// The variant that was brought in, as the hex it is named by.
    variant: String,
    /// The `Uuid` of the file it is to be joined into.
    target: String,
    /// The path that file is at, or an empty string.
    path: String,
    /// Where the variant file lies.
    file: String,
}

#[lazyffi(export = rola_)]
impl VariantGraphMerging {
    /// The variant that was brought in, as the hex it is named by.
    #[must_use]
    pub fn variant(&self) -> String {
        self.variant.clone()
    }

    /// The `Uuid` of the file it is to be joined into.
    #[must_use]
    pub fn target(&self) -> String {
        self.target.clone()
    }

    /// The path that file is at, or an empty string.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// Where the variant file lies.
    #[must_use]
    pub fn file(&self) -> String {
        self.file.clone()
    }
}

/// The text of one creator or message.
///
/// # FFI
///
/// A `RolaVariantGraphText *` the caller releases with `free_rola_variant_graph_text`. `_hash` and
/// `_text` each return a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaVariantGraphText)]
#[derive(Debug, Clone)]
pub struct VariantGraphText {
    /// The hash the variant names it by.
    hash: String,
    /// What it says.
    text: String,
}

#[lazyffi(export = rola_)]
impl VariantGraphText {
    /// The hash the variant names it by.
    #[must_use]
    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    /// What it says.
    #[must_use]
    pub fn text(&self) -> String {
        self.text.clone()
    }
}

/// What reading a variant graph failed with.
#[lazyffi(export = RolaVariantGraphError)]
#[derive(Debug)]
pub enum GraphError {
    /// No Workspace holds the directory the reading was asked of.
    NoWorkspace,
    /// The Workspace works in no Layout.
    NoLayout,
    /// The Workspace holds no version control index.
    NoIndex,
    /// The index, or the records beside it, could not be read.
    Read {
        /// Why it could not.
        reason: String,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWorkspace => formatter.write_str("no Workspace holds the directory"),
            Self::NoLayout => formatter.write_str("the Workspace works in no Layout"),
            Self::NoIndex => formatter.write_str("the Workspace holds no version control index"),
            Self::Read { reason } => write!(formatter, "the graph could not be read: {reason}"),
        }
    }
}

impl std::error::Error for GraphError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_inverse_index::GraphRequest;
    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use rorolala_storage::{Codec, Key};
    use rorolala_utils_location::Locate as _;
    use rorolala_vcs::{
        ComputeBlake3, Creator, Message, VCSIndex, VCSWrite as _, Variant, Version,
    };
    use rorolala_workspace::{Merging, Pending, Workspace};
    use uuid::Uuid;

    use super::{
        GraphError, VariantGraphFocus, VariantGraphSide, VariantGraphWindow, locate_variant_graph,
    };

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-variant-graph-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A Workspace with its index, the root written, and the Layout it works in.
    fn held(label: &str) -> (PathBuf, Workspace, VCSIndex, tokio::runtime::Runtime) {
        let dir = scratch(label);
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        let index = VCSIndex::create(dir.join(".rola/index"));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(index.write(Version::root())).unwrap();

        (dir, workspace, index, runtime)
    }

    /// A version of `base`, with the variant that made it, made by `creator` and saying `message`.
    fn version(
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
        base: &Version,
        seed: u8,
        creator: &Creator,
        message: &Message,
    ) -> (Variant, Version) {
        let variant = base.new_variant(
            [seed; 32],
            creator.compute_blake3(),
            message.compute_blake3(),
        );
        let made = variant.new_version();

        runtime.block_on(index.write(variant.clone())).unwrap();
        runtime.block_on(index.write(made.clone())).unwrap();

        (variant, made)
    }

    /// The two versions of one file, written into the index and named at `art.psd` in the Layout in
    /// `layout`, which records the second for `id`.
    fn chain(
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
        layout: &Layout,
        id: Uuid,
    ) -> (Variant, Version, Variant, Version) {
        let creator = Creator::try_from("me").unwrap();
        let message = Message::try_from("did").unwrap();
        runtime.block_on(index.write(creator.clone())).unwrap();
        runtime.block_on(index.write(message.clone())).unwrap();

        let (first_variant, first) =
            version(index, runtime, &Version::root(), 1, &creator, &message);
        let (second_variant, second) = version(index, runtime, &first, 2, &creator, &message);

        layout
            .create_entry(
                id,
                MutableData::new(
                    Some("alice".to_owned()),
                    second.compute_blake3(),
                    "art".to_owned(),
                ),
            )
            .unwrap();
        layout
            .create_path(&LayoutPath::new("art.psd").unwrap(), id)
            .unwrap();

        (first_variant, first, second_variant, second)
    }

    /// The window a test asks for when it names none: the graph's own defaults.
    fn window() -> VariantGraphWindow {
        let defaults = GraphRequest::default();

        VariantGraphWindow {
            back: defaults.back,
            forward: defaults.forward,
            joins: defaults.joins,
            branches: defaults.branches,
            max_nodes: defaults.max_nodes,
        }
    }

    /// A file's two versions are a chain of two variants, with the root behind them.
    #[test]
    fn a_focus_walks_the_chain_of_its_entry() {
        let (dir, workspace, index, runtime) = held("chain");
        let layout = workspace.layout().unwrap();
        let (first_variant, first, second_variant, second) =
            chain(&index, &runtime, &layout, Uuid::from_u128(1));

        let answer = locate_variant_graph(&dir, &window(), &[]).expect("the graph is read");

        // The focus is the work's own entry, which records the second version and resolves to the
        // variant that version fixes.
        let focus = answer
            .focus_at(0)
            .expect("the work's own entry is the focus");
        assert_eq!(focus.side(), "local");
        assert_eq!(focus.path(), "art.psd");
        assert_eq!(focus.version(), second.hash().hex());
        assert_eq!(focus.variant(), second_variant.hash().hex());

        // The chain behind it: two variants, and the three versions that fix them and the root.
        assert_eq!(answer.node_count(), 2);
        // Nothing was stored, so no node's content is here, and each says so.
        assert!(!answer.node_at(0).unwrap().offline());
        assert_eq!(answer.version_count(), 3);
        assert_eq!(answer.edge_count(), 1);

        let edge = answer.edge_at(0).unwrap();
        assert_eq!(edge.from(), second_variant.hash().hex());
        assert_eq!(edge.to(), first_variant.hash().hex());
        assert_eq!(edge.kind(), "base");

        // The first version is numbered from nothing and the second one past it; the root is named
        // as the root rather than numbered.
        let versions: Vec<_> = (0..answer.version_count())
            .filter_map(|index| answer.version_at(index))
            .collect();
        let first_version = versions
            .iter()
            .find(|version| version.hash() == first.hash().hex())
            .expect("the first version is in the window");
        assert!(first_version.number_present());
        assert_eq!(first_version.number(), 0);
        let root_version = versions
            .iter()
            .find(|version| version.is_root())
            .expect("the root is in the window");
        assert!(!root_version.number_present());

        // The words of the window's variants are read from the index, once each.
        assert_eq!(answer.creator_count(), 1);
        assert_eq!(answer.creator_at(0).unwrap().text(), "me");
        assert_eq!(answer.message_count(), 1);
        assert_eq!(answer.message_at(0).unwrap().text(), "did");

        // Every entry whose version the window holds is named beside the graph, with its side and
        // owner.
        assert_eq!(answer.entry_count(), 1);
        let entry = answer.entry_at(0).unwrap();
        assert_eq!(entry.side(), "local");
        assert_eq!(entry.owner(), "alice");
        assert_eq!(entry.version(), second.hash().hex());
        assert!(!entry.deprecated());

        assert!(!answer.truncated());
        assert_eq!(answer.missing_count(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    /// A focus may stand on an entry of a fetched copy, which is named by the Vault's side.
    #[test]
    fn a_focus_may_stand_on_a_fetched_copy() {
        let (dir, workspace, index, runtime) = held("copy");
        let layout = workspace.layout().unwrap();
        let (_first_variant, _first, second_variant, second) =
            chain(&index, &runtime, &layout, Uuid::from_u128(1));

        // The copy a fetch keeps: the container itself, which is the shape a copy was kept in before
        // generations and the one a fetch still drains a generation out of.
        let copy_dir = workspace.readonly_layouts_dir("origin", "truth");
        fs::create_dir_all(&copy_dir).unwrap();
        let copy = Layout::open(copy_dir).unwrap();
        copy.create_entry(
            Uuid::from_u128(2),
            MutableData::new(None, second.compute_blake3(), "from the vault".to_owned()),
        )
        .unwrap();
        copy.create_path(&LayoutPath::new("art.psd").unwrap(), Uuid::from_u128(2))
            .unwrap();

        let asked = [VariantGraphFocus {
            side: VariantGraphSide::Vault {
                name: "origin".to_owned(),
            },
            path: "art.psd".to_owned(),
        }];
        let answer = locate_variant_graph(&dir, &window(), &asked).expect("the graph is read");

        let focus = answer.focus_at(0).unwrap();
        assert_eq!(focus.side(), "origin");
        assert_eq!(focus.path(), "art.psd");
        assert_eq!(focus.variant(), second_variant.hash().hex());

        // Both sides are named: the work's own entries first, then the Vault's.
        assert_eq!(answer.entry_count(), 2);
        assert_eq!(answer.entry_at(0).unwrap().side(), "local");
        assert_eq!(answer.entry_at(1).unwrap().side(), "origin");
        assert_eq!(answer.entry_at(1).unwrap().owner(), "");

        // A side or a path this reading does not hold is answered as an entry recording no version,
        // and the window is empty rather than centred somewhere else.
        let unknown = [VariantGraphFocus {
            side: VariantGraphSide::Vault {
                name: "nobody".to_owned(),
            },
            path: "art.psd".to_owned(),
        }];
        let answer = locate_variant_graph(&dir, &window(), &unknown).expect("the graph is read");
        let focus = answer.focus_at(0).unwrap();
        assert_eq!(focus.version(), "");
        assert_eq!(focus.variant(), "");
        assert_eq!(answer.node_count(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    /// A merge in progress names its file, the entry it joins into and the variant waiting.
    #[test]
    fn a_merge_in_progress_is_named_beside_the_graph() {
        let (dir, workspace, index, runtime) = held("merging");
        let layout = workspace.layout().unwrap();
        let _ = chain(&index, &runtime, &layout, Uuid::from_u128(1));

        let variant = [0xab; 32];
        let mut merging = Merging::default();
        merging.push(Pending::new(Uuid::from_u128(1), variant, None));
        merging.write(workspace.get_root()).unwrap();

        let answer = locate_variant_graph(&dir, &window(), &[]).expect("the graph is read");

        let named = answer.merging_at(0).expect("the merge is named");
        assert_eq!(named.variant(), Key::new(variant).hex());
        assert_eq!(named.target(), Uuid::from_u128(1).to_string());
        assert_eq!(named.path(), "art.psd");

        let file = named.file();
        let path = Path::new(&file);
        assert!(
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("art."))
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("psd")),
            "the variant file is not named beside its target: {file}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A node whose content is stored is marked as usable offline, which is what the Desktop's
    /// check is drawn from.
    #[test]
    fn a_node_whose_content_is_here_is_marked_offline() {
        let (dir, workspace, index, runtime) = held("offline");
        let layout = workspace.layout().unwrap();
        let store = workspace.get_or_create_rola_storage();
        let content = runtime
            .block_on(store.write_object(b"the art", Codec::Raw))
            .unwrap();

        let creator = Creator::try_from("me").unwrap();
        let message = Message::try_from("did").unwrap();
        runtime.block_on(index.write(creator.clone())).unwrap();
        runtime.block_on(index.write(message.clone())).unwrap();

        let variant = Version::root().new_variant(
            *content.digest(),
            creator.compute_blake3(),
            message.compute_blake3(),
        );
        let version = variant.new_version();
        runtime.block_on(index.write(variant)).unwrap();
        runtime.block_on(index.write(version.clone())).unwrap();

        layout
            .create_entry(
                Uuid::from_u128(1),
                MutableData::new(None, *version.hash().digest(), "art"),
            )
            .unwrap();
        layout
            .create_path(&LayoutPath::new("art.psd").unwrap(), Uuid::from_u128(1))
            .unwrap();

        let answer = locate_variant_graph(&dir, &window(), &[]).expect("the graph is read");

        assert!(answer.node_at(0).unwrap().offline());

        let _ = fs::remove_dir_all(&dir);
    }

    /// A directory no Workspace holds has no graph.
    #[test]
    fn a_directory_no_workspace_holds_has_no_graph() {
        let (dir, _workspace, _index, _runtime) = held("refused");
        let _ = fs::remove_dir_all(&dir);

        let nowhere = scratch("nowhere");
        fs::create_dir_all(&nowhere).unwrap();
        assert!(matches!(
            locate_variant_graph(&nowhere, &window(), &[]),
            Err(GraphError::NoWorkspace)
        ));

        let _ = fs::remove_dir_all(&nowhere);
    }
}
