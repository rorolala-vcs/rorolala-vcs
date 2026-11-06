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
//! reaches the edge asks again from the node it reached. The whole answer crosses as one JSON document
//! (the graph, the entries, the merging records and the texts) because it is read as one picture, and
//! one document is one crossing.

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
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The side the Layout being worked in is named on.
const LOCAL: &str = "local";

/// The path an entry is at when it has been deprecated: the marker the Vault's own moves put it under.
const DEPRECATED_PREFIX: &str = "@/removed/";

/// The variant graph around the entries `request` names, as one JSON document.
///
/// The window is walked over the inverse index's records, rebuilt first when they do not describe the
/// index, so what is drawn is what the index holds now, no fetch and no network. What only the
/// workspace knows is added beside it: the entries the window holds, from the Layout being worked in
/// and from every fetched copy, each with the side it is named on; the merges in progress; and the
/// text of each creator and message the window's variants name. A key the index does not hold is
/// answered as missing rather than fetched.
///
/// The graph's window is the request's: `focus` names the entries to stand on, each a `side` and a
/// `path`, a side being `local` or the name of a Vault whose copy was fetched, and an empty `focus`
/// stands on every entry of the Layout being worked in that records a version. `back`, `forward`,
/// `joins`, `branches` and `max_nodes` are the walk's, as the graph reads them.
///
/// # Errors
///
/// Returns [`GraphError::NoWorkspace`] when no Workspace holds `directory`,
/// [`GraphError::NoLayout`] when the Workspace works in no Layout, [`GraphError::NoIndex`] when it
/// holds no version control index, [`GraphError::BadRequest`] when `request` does not read as one this
/// build knows, and [`GraphError::Read`] when the index or the records beside it cannot be read.
///
/// # FFI
///
/// `directory` and `request` cross as UTF-8 strings. The answer is an owned JSON string the caller
/// releases with `free_string`; a failure is a `RolaVariantGraphError *` the caller releases with
/// `free_rola_variant_graph_error`.
#[lazyffi(export = locate_rola_variant_graph_json)]
pub fn locate_variant_graph_json(directory: &Path, request: &str) -> Result<String, GraphError> {
    let workspace = Workspace::locate(directory).ok_or(GraphError::NoWorkspace)?;
    let layout = workspace
        .layout_if_there()
        .map_err(|error| GraphError::Read {
            reason: error.to_string(),
        })?
        .ok_or(GraphError::NoLayout)?;
    let index = VCSIndex::locate(directory).ok_or(GraphError::NoIndex)?;
    let request: Request =
        serde_json::from_str(request).map_err(|error| GraphError::BadRequest {
            reason: error.to_string(),
        })?;

    let runtime = tokio::runtime::Runtime::new().map_err(|error| GraphError::Read {
        reason: error.to_string(),
    })?;
    let inverse = InverseIndex::at(index.clone());
    let snapshot = snapshot_of(&runtime, &inverse)?;
    let sides = Sides::open(&workspace, layout);
    let entries = sides.entries();
    let focused = sides.focused(&request.focus, &entries);
    let versions: Vec<Hash> = focused
        .iter()
        .filter_map(|focused| focused.version)
        .collect();
    let view = snapshot.graph(&request.graph(versions));
    let texts = texts_of(&runtime, &index, &view);
    let merging = merging_of(&workspace, &sides.local);
    let focus = focus_of(&view, focused);

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

    let graph = GraphJson {
        focus,
        nodes: view.nodes.iter().map(node_json).collect(),
        versions: view
            .versions
            .iter()
            .map(|version| VersionJson {
                hash: version.hash.hex(),
                variant: version.variant.hex(),
                number: version.number,
                is_root: version.is_root,
            })
            .collect(),
        edges: view
            .edges
            .iter()
            .map(|edge| EdgeJson {
                from: edge.from.hex(),
                to: edge.to.hex(),
                kind: edge_kind(edge.kind),
            })
            .collect(),
        missing: view
            .missing
            .iter()
            .map(|missing| MissingJson {
                hash: missing.hash.hex(),
                kind: missing_kind(missing.kind),
            })
            .collect(),
        truncated: view.truncated,
        entries: entries.iter().map(|entry| entry_json(entry)).collect(),
        merging,
        texts,
    };

    serde_json::to_string(&graph).map_err(|error| GraphError::Read {
        reason: error.to_string(),
    })
}

/// What a variant graph is asked for.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct Request {
    /// The entries to stand on; empty stands on the Layout being worked in's own entries.
    focus: Vec<Focus>,
    /// How many variants back towards the root to walk from each focus.
    back: u32,
    /// How many variants forward from each focus to walk.
    forward: u32,
    /// Whether to walk into what a variant merges in.
    joins: bool,
    /// Whether to take in a variant based on a version in the window, one hop.
    branches: bool,
    /// How many variants the window holds at most.
    max_nodes: u32,
}

impl Default for Request {
    fn default() -> Self {
        let window = GraphRequest::default();

        Self {
            focus: Vec::new(),
            back: window.back,
            forward: window.forward,
            joins: window.joins,
            branches: window.branches,
            max_nodes: window.max_nodes,
        }
    }
}

impl Request {
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

/// One entry a request stands on: the side it is named on, and the path it is at.
#[derive(Debug, Clone, Deserialize)]
struct Focus {
    /// The side the entry is named on: `local`, or the name of a Vault whose copy was fetched.
    side: String,
    /// The path the entry is at, as the Layout names it.
    path: String,
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
    fn focused(&self, requested: &[Focus], entries: &[Entry]) -> Vec<Focused> {
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
    fn focused_one(&self, focus: &Focus) -> Focused {
        let version = self
            .named(&focus.side)
            .and_then(|layout| {
                LayoutPath::new(&focus.path)
                    .ok()
                    .and_then(|path| layout.data_of(&path))
            })
            .and_then(|data| (data.version() != [0; 32]).then(|| Key::new(data.version())));

        Focused {
            side: focus.side.clone(),
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

/// One entry of a Layout, before it is written as JSON.
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
fn focus_of(view: &GraphView, focused: Vec<Focused>) -> Vec<FocusJson> {
    let anchors: BTreeMap<Hash, Option<Hash>> = view
        .anchors
        .iter()
        .map(|anchor| (anchor.version, anchor.variant))
        .collect();

    focused
        .into_iter()
        .map(|focused| FocusJson {
            side: focused.side,
            path: focused.path,
            variant: focused
                .version
                .and_then(|version| anchors.get(&version).copied().flatten())
                .map(|variant| variant.hex()),
            version: focused.version.map(|version| version.hex()),
        })
        .collect()
}

/// One variant, as JSON.
fn node_json(node: &GraphNode) -> NodeJson {
    NodeJson {
        hash: node.hash.hex(),
        base_version: node.base_version.hex(),
        base_variant: node.base_variant.map(|variant| variant.hex()),
        join: node.join.map(|join| join.hex()),
        storage: node.storage.hex(),
        creator: node.creator.hex(),
        message: node.message.hex(),
        frontier: FrontierJson {
            parents: node.frontier.parents,
            children: node.frontier.children,
            joined: node.frontier.joined,
        },
    }
}

/// One entry, as JSON.
fn entry_json(entry: &Entry) -> EntryJson {
    let path = entry.path.as_str();

    EntryJson {
        side: entry.side.clone(),
        path: path.to_owned(),
        uuid: entry.id.to_string(),
        version: entry.version().map(|version| version.hex()),
        owner: entry.data.owner().map(str::to_owned),
        deprecated: path.starts_with(DEPRECATED_PREFIX),
    }
}

/// The merges in progress, as JSON.
fn merging_of(workspace: &Workspace, layout: &Layout) -> Vec<MergingJson> {
    let merging = Merging::read(workspace.get_root());

    merging
        .iter()
        .map(|pending| MergingJson {
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
fn texts_of(runtime: &tokio::runtime::Runtime, index: &VCSIndex, view: &GraphView) -> TextsJson {
    let mut creators = BTreeMap::new();
    let mut messages = BTreeMap::new();
    let wanted: BTreeSet<Hash> = view
        .nodes
        .iter()
        .flat_map(|node| [node.creator, node.message])
        .collect();

    for hash in wanted {
        match runtime.block_on(index.read(hash)) {
            Ok(VCSIndexObject::Creator(creator)) => {
                creators.insert(hash.hex(), creator.read_to_string().to_owned());
            }
            Ok(VCSIndexObject::Message(message)) => {
                messages.insert(hash.hex(), message.read_to_string().to_owned());
            }
            _ => {}
        }
    }

    TextsJson { creators, messages }
}

/// What an edge between two variants is, as JSON.
const fn edge_kind(kind: GraphEdgeKind) -> &'static str {
    match kind {
        GraphEdgeKind::Base => "base",
        GraphEdgeKind::Join => "join",
    }
}

/// What kind a key the index does not hold would be, as JSON.
const fn missing_kind(kind: MissingKind) -> &'static str {
    match kind {
        MissingKind::Version => "version",
        MissingKind::Variant => "variant",
    }
}

/// The variant graph, the entries that name it, and the texts in it.
#[derive(Debug, Serialize)]
struct GraphJson {
    /// What each entry the request stood on resolved to, in the order asked.
    focus: Vec<FocusJson>,
    /// The variants in the window, in key order.
    nodes: Vec<NodeJson>,
    /// The versions fixing them, in key order.
    versions: Vec<VersionJson>,
    /// The edges between variants in the window.
    edges: Vec<EdgeJson>,
    /// The keys the window names but the index does not hold.
    missing: Vec<MissingJson>,
    /// Whether the window was cut short by its cap rather than walked out.
    truncated: bool,
    /// The entries whose version the window holds, the work's own first.
    entries: Vec<EntryJson>,
    /// The merges in progress.
    merging: Vec<MergingJson>,
    /// The text of every creator and message the window's variants name.
    texts: TextsJson,
}

/// What an entry the request stood on resolved to.
#[derive(Debug, Serialize)]
struct FocusJson {
    /// The side it is named on.
    side: String,
    /// The path it is at.
    path: String,
    /// The variant the version it records fixes, when the graph resolved one.
    variant: Option<String>,
    /// The version it records, when it records one.
    version: Option<String>,
}

/// One variant in the window.
#[derive(Debug, Serialize)]
struct NodeJson {
    /// The variant's own key, as hex.
    hash: String,
    /// The version it is based on, as hex.
    base_version: String,
    /// The variant that version fixes, when the index holds it.
    base_variant: Option<String>,
    /// The variant it merges in, when it is a merge.
    join: Option<String>,
    /// The content it was made of.
    storage: String,
    /// Who made it.
    creator: String,
    /// What it says was done.
    message: String,
    /// Where the window stops short of what this node names.
    frontier: FrontierJson,
}

/// Where a node's walks were not taken in.
#[derive(Debug, Serialize)]
struct FrontierJson {
    /// The base variant, or the version it is based on, is not in the window.
    parents: bool,
    /// A variant based on one of the node's versions is not in the window.
    children: bool,
    /// A variant the node merges in is not in the window.
    joined: bool,
}

/// One version fixing a variant in the window.
#[derive(Debug, Serialize)]
struct VersionJson {
    /// The version's own key, as hex.
    hash: String,
    /// The variant it fixes, as hex; the zero hash for the root.
    variant: String,
    /// Its number in its chain, when it has one; null for the root and for a cut-off chain.
    number: Option<u64>,
    /// Whether it is the root, numbered before the first.
    is_root: bool,
}

/// One edge between two variants in the window.
#[derive(Debug, Serialize)]
struct EdgeJson {
    /// The variant the edge is walked from.
    from: String,
    /// The variant it names.
    to: String,
    /// What the edge is: `base` or `join`.
    kind: &'static str,
}

/// One key the window names but the index does not hold.
#[derive(Debug, Serialize)]
struct MissingJson {
    /// The key, as hex.
    hash: String,
    /// What it would have been: `version` or `variant`.
    kind: &'static str,
}

/// One entry of one side.
#[derive(Debug, Serialize)]
struct EntryJson {
    /// The side it is named on.
    side: String,
    /// The path it is at.
    path: String,
    /// The `Uuid` it is held by.
    uuid: String,
    /// The version it records, when it records one.
    version: Option<String>,
    /// The account that holds it, when one does.
    owner: Option<String>,
    /// Whether it was moved under the deprecated marker.
    deprecated: bool,
}

/// One merge in progress.
#[derive(Debug, Serialize)]
struct MergingJson {
    /// The variant that was checked in, as the hex it is named by.
    variant: String,
    /// The `Uuid` of the file it is to be joined into.
    target: String,
    /// The path that file is at, when the Layout still names it.
    path: String,
    /// Where the variant file lies.
    file: String,
}

/// The words of what the window's variants name.
#[derive(Debug, Default, Serialize)]
struct TextsJson {
    /// Each creator's name, by the hash the variant names it by.
    creators: BTreeMap<String, String>,
    /// Each message, by the hash the variant names it by.
    messages: BTreeMap<String, String>,
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
    /// The request does not read as one this build knows.
    BadRequest {
        /// Why it does not.
        reason: String,
    },
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
            Self::BadRequest { reason } => {
                write!(formatter, "the request does not read: {reason}")
            }
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

    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use rorolala_storage::Key;
    use rorolala_utils_location::Locate as _;
    use rorolala_vcs::{
        ComputeBlake3, Creator, Message, VCSIndex, VCSWrite as _, Variant, Version,
    };
    use rorolala_workspace::{Merging, Pending, Workspace};
    use serde_json::Value;
    use uuid::Uuid;

    use super::{GraphError, locate_variant_graph_json};

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

    /// A file's two versions are a chain of two variants, with the root behind them.
    #[test]
    fn a_focus_walks_the_chain_of_its_entry() {
        let (dir, workspace, index, runtime) = held("chain");
        let layout = workspace.layout().unwrap();
        let (first_variant, first, second_variant, second) =
            chain(&index, &runtime, &layout, Uuid::from_u128(1));

        let answer = locate_variant_graph_json(&dir, "{}").expect("the graph is read");
        let json: Value = serde_json::from_str(&answer).expect("the answer is JSON");

        // The focus is the work's own entry, which records the second version and resolves to the
        // variant that version fixes.
        let focus = &json["focus"][0];
        assert_eq!(focus["side"].as_str(), Some("local"));
        assert_eq!(focus["path"].as_str(), Some("art.psd"));
        assert_eq!(
            focus["version"].as_str(),
            Some(second.hash().hex().as_str())
        );
        assert_eq!(
            focus["variant"].as_str(),
            Some(second_variant.hash().hex().as_str())
        );

        // The chain behind it: two variants, and the three versions that fix them and the root.
        assert_eq!(json["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(json["versions"].as_array().unwrap().len(), 3);
        assert_eq!(json["edges"].as_array().unwrap().len(), 1);
        assert_eq!(
            json["edges"][0]["from"].as_str(),
            Some(second_variant.hash().hex().as_str())
        );
        assert_eq!(
            json["edges"][0]["to"].as_str(),
            Some(first_variant.hash().hex().as_str())
        );
        assert_eq!(json["edges"][0]["kind"].as_str(), Some("base"));

        // The first version is numbered from nothing and the second one past it; the root is named
        // as the root rather than numbered.
        let versions = json["versions"].as_array().unwrap();
        let first_version = versions
            .iter()
            .find(|version| version["hash"].as_str() == Some(first.hash().hex().as_str()))
            .expect("the first version is in the window");
        assert_eq!(first_version["number"].as_u64(), Some(0));
        let root_version = versions
            .iter()
            .find(|version| version["is_root"] == Value::Bool(true))
            .expect("the root is in the window");
        assert_eq!(root_version["number"], Value::Null);

        // The words of the window's variants are read from the index, once each.
        assert_eq!(
            json["texts"]["creators"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .and_then(Value::as_str),
            Some("me")
        );
        assert_eq!(
            json["texts"]["messages"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .and_then(Value::as_str),
            Some("did")
        );

        // Every entry whose version the window holds is named beside the graph, with its side and owner.
        assert_eq!(json["entries"].as_array().unwrap().len(), 1);
        assert_eq!(json["entries"][0]["side"].as_str(), Some("local"));
        assert_eq!(json["entries"][0]["owner"].as_str(), Some("alice"));
        assert_eq!(
            json["entries"][0]["version"].as_str(),
            Some(second.hash().hex().as_str())
        );
        assert_eq!(json["entries"][0]["deprecated"], Value::Bool(false));

        assert_eq!(json["truncated"], Value::Bool(false));
        assert_eq!(json["missing"].as_array().unwrap().len(), 0);

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

        let answer =
            locate_variant_graph_json(&dir, r#"{"focus":[{"side":"origin","path":"art.psd"}]}"#)
                .expect("the graph is read");
        let json: Value = serde_json::from_str(&answer).expect("the answer is JSON");

        assert_eq!(json["focus"][0]["side"].as_str(), Some("origin"));
        assert_eq!(json["focus"][0]["path"].as_str(), Some("art.psd"));
        assert_eq!(
            json["focus"][0]["variant"].as_str(),
            Some(second_variant.hash().hex().as_str())
        );

        // Both sides are named: the work's own entries first, then the Vault's.
        let entries = json["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["side"].as_str(), Some("local"));
        assert_eq!(entries[1]["side"].as_str(), Some("origin"));
        assert_eq!(entries[1]["owner"], Value::Null);

        // A side or a path this reading does not hold is answered as an entry recording no version,
        // and the window is empty rather than centred somewhere else.
        let unknown =
            locate_variant_graph_json(&dir, r#"{"focus":[{"side":"nobody","path":"art.psd"}]}"#)
                .expect("the graph is read");
        let json: Value = serde_json::from_str(&unknown).expect("the answer is JSON");
        assert_eq!(json["focus"][0]["version"], Value::Null);
        assert_eq!(json["focus"][0]["variant"], Value::Null);
        assert_eq!(json["nodes"].as_array().unwrap().len(), 0);

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

        let answer = locate_variant_graph_json(&dir, "{}").expect("the graph is read");
        let json: Value = serde_json::from_str(&answer).expect("the answer is JSON");

        let named = &json["merging"][0];
        assert_eq!(
            named["variant"].as_str(),
            Some(Key::new(variant).hex().as_str())
        );
        assert_eq!(
            named["target"].as_str(),
            Some(Uuid::from_u128(1).to_string().as_str())
        );
        assert_eq!(named["path"].as_str(), Some("art.psd"));
        assert!(
            named["file"].as_str().is_some_and(|file| {
                let path = Path::new(file);

                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("art."))
                    && path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("psd"))
            }),
            "the variant file is not named beside its target: {named:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A request that does not read is refused, and a directory no Workspace holds has no graph.
    #[test]
    fn what_cannot_be_read_is_refused() {
        let (dir, _workspace, _index, _runtime) = held("refused");
        assert!(matches!(
            locate_variant_graph_json(&dir, "{"),
            Err(GraphError::BadRequest { .. })
        ));

        let nowhere = scratch("nowhere");
        fs::create_dir_all(&nowhere).unwrap();
        assert!(matches!(
            locate_variant_graph_json(&nowhere, "{}"),
            Err(GraphError::NoWorkspace)
        ));

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&nowhere);
    }
}
