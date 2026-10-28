//! The version chain of one file, as the Desktop's Version Graph reads it.
//!
//! The chain is walked by [`read_chain`], the same walk `rola status <file>` uses, but what crosses
//! the ABI is the structure rather than the command line's drawing: one row of versions, the
//! variants that produced them, and the joins between variants. The Desktop lays that structure
//! out as geometry of its own.

use std::collections::HashMap;
use std::path::Path;

use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::{Layout, LayoutPath};
use rorolala_storage::Key;
use rorolala_tree_analyze::tree_diff;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{Lookback, VCSIndex, VariantView, chain_of, read_chain};
use rorolala_workspace::Workspace;
use uuid::Uuid;

/// How alike two text files have to be to count as the same file moved, when nothing is said.
const DEFAULT_ALIKE: f32 = 0.6;

/// One version the graph can draw.
#[derive(Debug, Clone)]
struct VersionNode {
    /// The version's number, zero for the oldest.
    level: u64,
    /// The full hash the index names the version by, as hex.
    hash: String,
    /// Who made the variant that produced the version.
    creator: String,
    /// What the variant that produced the version says.
    message: String,
}

/// One variant the graph can draw.
#[derive(Debug, Clone)]
struct VariantNode {
    /// The label it is drawn by: its hash's leading characters.
    label: String,
    /// The full hash the index names the variant by, as hex; empty for a placeholder the
    /// drawing invents rather than reads.
    hash: String,
    /// The version it is based on.
    level: u64,
    /// Whether it is the variant the version above points at.
    spine: bool,
    /// The index of the variant it merges into, in this list.
    join: Option<usize>,
    /// Who made it.
    creator: String,
    /// What it says.
    message: String,
}

/// The chain one file has, as the Desktop's Version Graph reads it.
///
/// # FFI
///
/// Opaque: C takes a pointer from `locate_rola_version_graph` and gives it back with
/// `free_rola_version_graph`. Every version and variant is read by its own accessor; the strings are
/// owned by the caller and released with `free_string`.
#[lazyffi(export = RolaVersionGraph)]
#[derive(Debug, Clone)]
pub struct VersionGraph {
    /// The versions, the highest first.
    versions: Vec<VersionNode>,
    /// The variants in the chain's order: level by level, the spine before the variants merged in
    /// beside it.
    variants: Vec<VariantNode>,
    /// The version the tracked Vault records, and the Vault's name, when the chain holds it.
    origin: Option<(u64, String)>,
    /// The Vault's chain, when the index here holds the versions it is made of.
    ///
    /// It is the chain from the version the tracked Vault records back to the root, the highest
    /// first, and its numbers are the same as the local chain's where the two share a version. It
    /// is empty when the Vault's version is not held here — a fetch brings its Layout and never its
    /// versions — and then only [`Self::upstream_head`] is known.
    upstream: Vec<VersionNode>,
    /// The version the tracked Vault records, as the index names it by, or an empty string when the
    /// Layout tracks no Vault or the Vault's Layout does not name the entry.
    upstream_head: String,
    /// The Vault whose version [`Self::upstream_head`] is, or an empty string when none is tracked.
    upstream_vault: String,
}

/// Reads the chain of the file `target` as the Version Graph shows it.
///
/// # FFI
///
/// A `RolaVersionGraph` the caller releases with `free_rola_version_graph`, or null when the file
/// has no version history: no Workspace holds the directory, the Workspace works in no Layout, the
/// Layout does not name `target`, nothing was ever recorded for it, or the chain cannot be read.
#[must_use]
#[lazyffi(export = locate_rola_version_graph)]
pub fn locate_version_graph(directory: &Path, target: &Path) -> Option<VersionGraph> {
    let workspace = Workspace::locate(directory)?;
    let layout = working_layout(&workspace)?;
    let index = VCSIndex::locate(directory)?;
    let found = found(&workspace, &layout, target)?;
    let tracked = vault_origin(&workspace, found.id);

    let runtime = tokio::runtime::Runtime::new().ok()?;
    let chain = read_chain(
        &index,
        &runtime,
        Key::new(found.version),
        tracked
            .as_ref()
            .map(|(vault, version)| (vault.as_str(), *version)),
    )
    .ok()?;

    // The Vault's own chain is read only when its versions are here: a fetch brings the Vault's
    // Layout and never its index, so a Vault ahead of this side normally has nothing to walk. What
    // is known either way is the version it records, which is what the Desktop marks when there is
    // no chain to draw beside the work's own.
    let (upstream, upstream_head, upstream_vault) = match tracked {
        Some((vault, version)) => (
            read_chain(&index, &runtime, Key::new(version), None)
                .ok()
                .map_or_else(Vec::new, |lookback| versions_of(&lookback)),
            Key::new(version).hex(),
            vault,
        ),
        None => (Vec::new(), String::new(), String::new()),
    };

    Some(VersionGraph::of(
        &chain,
        upstream,
        upstream_head,
        upstream_vault,
    ))
}

/// The version `target` is recorded at, and the `Uuid` its path names.
struct Found {
    /// The `Uuid` the path names.
    id: Uuid,
    /// The version the Layout records for it.
    version: [u8; 32],
}

impl VersionGraph {
    /// Reads the chain into the structure the Desktop draws.
    fn of(
        lookback: &Lookback,
        upstream: Vec<VersionNode>,
        upstream_head: String,
        upstream_vault: String,
    ) -> Self {
        let versions = versions_of(lookback);

        // The nodes are in the same order as the level views: one spine and then the variants
        // merged in beside it, level by level, the highest first.
        let chain = chain_of(&lookback.levels, lookback.top, lookback.first.is_some());
        let nodes = chain.nodes();
        let mut views: Vec<&VariantView> = Vec::with_capacity(nodes.len());

        for level in &lookback.levels {
            views.push(&level.variant);

            for merged in &level.merged {
                views.push(merged);
            }
        }

        let variants = nodes
            .iter()
            .enumerate()
            .map(|(at, node)| {
                let (hash, creator, message) = views.get(at).map_or_else(
                    || (String::new(), String::new(), String::new()),
                    |view| {
                        (
                            view.hash.clone(),
                            view.creator.clone(),
                            view.message.clone(),
                        )
                    },
                );

                VariantNode {
                    label: node.label().to_owned(),
                    hash,
                    level: node.level(),
                    spine: node.is_spine(),
                    join: node.join(),
                    creator,
                    message,
                }
            })
            .collect();

        let origin = lookback
            .origin
            .as_ref()
            .map(|origin| (origin.number, origin.vault.clone()));

        Self {
            versions,
            variants,
            origin,
            upstream,
            upstream_head,
            upstream_vault,
        }
    }
}

/// The versions a lookback names, the highest first, with the words of the variant that made each.
fn versions_of(lookback: &Lookback) -> Vec<VersionNode> {
    let levels = lookback.levels.len();
    let has_first = lookback.first.is_some();
    let top = u64::try_from(
        levels
            .saturating_add(usize::from(has_first))
            .saturating_sub(1),
    )
    .unwrap_or(u64::MAX);

    // A version's words are the producing variant's: the spine of the level below it, or the
    // root-based variant for the oldest version.
    let mut words: HashMap<u64, (String, String)> = HashMap::new();

    for level in &lookback.levels {
        words.insert(
            level.number.saturating_add(1),
            (level.variant.creator.clone(), level.variant.message.clone()),
        );
    }

    if let Some(first) = &lookback.first {
        words.insert(0, (first.creator.clone(), first.message.clone()));
    }

    let mut versions = Vec::new();

    for level in (0..=top).rev() {
        let (creator, message) = words.get(&level).cloned().unwrap_or_default();

        versions.push(VersionNode {
            level,
            hash: lookback
                .version_hashes
                .get(&level)
                .cloned()
                .unwrap_or_default(),
            creator,
            message,
        });
    }

    versions
}

#[lazyffi(export = rola_version_graph_)]
impl VersionGraph {
    /// How many versions the chain has.
    #[must_use]
    #[lazyffi(export = rola_version_graph_version_count)]
    pub const fn version_count(&self) -> usize {
        self.versions.len()
    }

    /// The number of the version at `index`, the highest first.
    ///
    /// # FFI
    ///
    /// Zero when there is no such version.
    #[must_use]
    #[lazyffi(export = rola_version_graph_version_level)]
    pub fn version_level(&self, index: usize) -> u64 {
        self.versions.get(index).map_or(0, |version| version.level)
    }

    /// The full hash of the version at `index`, as hex.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such version. The caller owns the string and releases it
    /// with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_version_hash)]
    pub fn version_hash(&self, index: usize) -> String {
        self.versions
            .get(index)
            .map_or_else(String::new, |version| version.hash.clone())
    }

    /// Who made the variant that produced the version at `index`.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such version, or when the index does not hold the maker.
    /// The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_version_creator)]
    pub fn version_creator(&self, index: usize) -> String {
        self.versions
            .get(index)
            .map_or_else(String::new, |version| version.creator.clone())
    }

    /// What the variant that produced the version at `index` says.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such version, or when the index does not hold the message.
    /// The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_version_message)]
    pub fn version_message(&self, index: usize) -> String {
        self.versions
            .get(index)
            .map_or_else(String::new, |version| version.message.clone())
    }

    /// How many variants the chain draws.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_count)]
    pub const fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// The label the variant at `index` is drawn by.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such variant. The caller owns the string and releases it
    /// with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_label)]
    pub fn variant_label(&self, index: usize) -> String {
        self.variants
            .get(index)
            .map_or_else(String::new, |variant| variant.label.clone())
    }

    /// The full hash of the variant at `index`, as hex.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such variant, or when it is a placeholder the drawing has
    /// no hash for. The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_hash)]
    pub fn variant_hash(&self, index: usize) -> String {
        self.variants
            .get(index)
            .map_or_else(String::new, |variant| variant.hash.clone())
    }

    /// The number of the version the variant at `index` is based on.
    ///
    /// # FFI
    ///
    /// Zero when there is no such variant.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_level)]
    pub fn variant_level(&self, index: usize) -> u64 {
        self.variants.get(index).map_or(0, |variant| variant.level)
    }

    /// Whether the variant at `index` is the one the version above points at.
    ///
    /// # FFI
    ///
    /// False when there is no such variant.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_spine)]
    pub fn variant_spine(&self, index: usize) -> bool {
        self.variants
            .get(index)
            .is_some_and(|variant| variant.spine)
    }

    /// The index of the variant the variant at `index` merges into.
    ///
    /// # FFI
    ///
    /// A negative number when it merges none, or when there is no such variant.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_join)]
    pub fn variant_join(&self, index: usize) -> i64 {
        self.variants.get(index).map_or(-1, |variant| {
            variant
                .join
                .map_or(-1, |join| i64::try_from(join).unwrap_or(i64::MAX))
        })
    }

    /// Who made the variant at `index`.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such variant, or when the index does not hold the maker.
    /// The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_creator)]
    pub fn variant_creator(&self, index: usize) -> String {
        self.variants
            .get(index)
            .map_or_else(String::new, |variant| variant.creator.clone())
    }

    /// What the variant at `index` says.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such variant, or when the index does not hold the message.
    /// The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_variant_message)]
    pub fn variant_message(&self, index: usize) -> String {
        self.variants
            .get(index)
            .map_or_else(String::new, |variant| variant.message.clone())
    }

    /// The number of the version the tracked Vault records, when the chain marks one.
    ///
    /// # FFI
    ///
    /// A negative number when there is no such version.
    #[must_use]
    #[lazyffi(export = rola_version_graph_origin_level)]
    pub fn origin_level(&self) -> i64 {
        self.origin
            .as_ref()
            .map_or(-1, |origin| i64::try_from(origin.0).unwrap_or(i64::MAX))
    }

    /// The name of the Vault whose version is marked, when one is.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such Vault. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_origin_vault)]
    pub fn origin_vault(&self) -> String {
        self.origin
            .as_ref()
            .map_or_else(String::new, |origin| origin.1.clone())
    }

    /// How many versions of the tracked Vault's chain the index here holds.
    ///
    /// # FFI
    ///
    /// Zero when the Vault's version is not held here, which is the ordinary state after a fetch:
    /// a fetch brings the Vault's Layout and never its index.
    #[must_use]
    #[lazyffi(export = rola_version_graph_upstream_count)]
    pub const fn upstream_count(&self) -> usize {
        self.upstream.len()
    }

    /// The number of the upstream version at `index`, the highest first.
    ///
    /// # FFI
    ///
    /// Zero when there is no such version.
    #[must_use]
    #[lazyffi(export = rola_version_graph_upstream_level)]
    pub fn upstream_level(&self, index: usize) -> u64 {
        self.upstream.get(index).map_or(0, |version| version.level)
    }

    /// The full hash of the upstream version at `index`, as hex.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such version. The caller owns the string and releases it
    /// with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_upstream_hash)]
    pub fn upstream_hash(&self, index: usize) -> String {
        self.upstream
            .get(index)
            .map_or_else(String::new, |version| version.hash.clone())
    }

    /// Who made the variant that produced the upstream version at `index`.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such version, or when the index does not hold the maker.
    /// The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_upstream_creator)]
    pub fn upstream_creator(&self, index: usize) -> String {
        self.upstream
            .get(index)
            .map_or_else(String::new, |version| version.creator.clone())
    }

    /// What the variant that produced the upstream version at `index` says.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such version, or when the index does not hold the message.
    /// The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_upstream_message)]
    pub fn upstream_message(&self, index: usize) -> String {
        self.upstream
            .get(index)
            .map_or_else(String::new, |version| version.message.clone())
    }

    /// The version the tracked Vault records, as the index names it by.
    ///
    /// # FFI
    ///
    /// An empty string when the Layout tracks no Vault or the Vault's Layout does not name the
    /// entry. The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_upstream_head)]
    pub fn upstream_head(&self) -> String {
        self.upstream_head.clone()
    }

    /// The Vault whose recorded version [`Self::upstream_head`] is.
    ///
    /// # FFI
    ///
    /// An empty string when no Vault is tracked. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_version_graph_upstream_vault)]
    pub fn upstream_vault(&self) -> String {
        self.upstream_vault.clone()
    }
}

/// The Layout the Workspace is working in, when it is working in one.
fn working_layout(workspace: &Workspace) -> Option<Layout> {
    let layouts = workspace.layouts();
    let current = layouts.current().ok().flatten()?;

    layouts.get(&current).ok().flatten()
}

/// The path `target` names in the Layout, read from the Workspace's root.
fn layout_path(workspace: &Workspace, target: &Path) -> Option<LayoutPath> {
    let root = workspace.get_root();
    let absolute = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };

    LayoutPath::from_relative(absolute.strip_prefix(root).ok()?).ok()
}

/// The version the Layout records for `target`, when it records one.
fn found(workspace: &Workspace, layout: &Layout, target: &Path) -> Option<Found> {
    let path = layout_path(workspace, target)?;
    let id = match layout.id_of(&path) {
        Some(id) => id,
        None => renamed(workspace, layout, &path)?,
    };
    let version = layout.entry(id)?.version();

    (version != [0; 32]).then_some(Found { id, version })
}

/// The entry `path` moved from, when the tree reading finds a move.
fn renamed(workspace: &Workspace, layout: &Layout, path: &LayoutPath) -> Option<Uuid> {
    if !workspace.get_root().join(path.to_path_buf()).is_file() {
        return None;
    }

    // The ignore rules govern what the tree gained, and what is asked here is where a path went:
    // a move is found whether or not the place it moved to is ignored, so the empty set is the
    // reading this wants.
    let diff = tree_diff(layout, workspace, DEFAULT_ALIKE, &IgnoreRuleSet::empty()).ok()?;
    let rename = diff.renamed.iter().find(|rename| rename.to == *path)?;

    layout.id_of(&rename.from)
}

/// The version the Vault the Layout tracks records for `id`, when its fetched copy is here.
fn vault_origin(workspace: &Workspace, id: Uuid) -> Option<(String, [u8; 32])> {
    let layouts = workspace.layouts();
    let current = layouts.current().ok().flatten()?;
    let track = layouts.track(&current).ok().flatten()?;
    let dir = workspace.readonly_layout_copy(&track, VAULT_LAYOUT_NAME)?;
    let layout = Layout::open(dir).ok()?;
    let version = layout.entry(id)?.version();

    (version != [0; 32]).then_some((track, version))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::{LayoutPath, MutableData};
    use rorolala_utils_constants::DEFAULT_LAYOUT_NAME;
    use rorolala_utils_location::Locate as _;
    use rorolala_vcs::{ComputeBlake3, Creator, Message, VCSIndex, Version};
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::locate_version_graph;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-version-graph-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A Workspace working in one Layout, with an index and the root written.
    fn held(label: &str) -> (PathBuf, Workspace, VCSIndex, tokio::runtime::Runtime) {
        let dir = scratch(label);
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layouts = workspace.layouts();
        layouts.create(DEFAULT_LAYOUT_NAME).unwrap();
        layouts.set_current(DEFAULT_LAYOUT_NAME).unwrap();

        let index = VCSIndex::create(dir.join(".rola/index"));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(index.write(Version::root())).unwrap();

        (dir, workspace, index, runtime)
    }

    /// The Layout the Workspace works in.
    fn working(workspace: &Workspace) -> rorolala_layout::Layout {
        workspace
            .layouts()
            .get(DEFAULT_LAYOUT_NAME)
            .unwrap()
            .unwrap()
    }

    /// A version of `base`, made by `creator` and saying `message`.
    fn version(
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
        base: &Version,
        seed: u8,
        creator: &Creator,
        message: &Message,
    ) -> Version {
        let variant = base.new_variant(
            [seed; 32],
            creator.compute_blake3(),
            message.compute_blake3(),
        );
        let made = variant.new_version();

        runtime.block_on(index.write(variant)).unwrap();
        runtime.block_on(index.write(made.clone())).unwrap();

        made
    }

    #[test]
    fn a_file_with_a_recorded_version_has_a_chain() {
        let (dir, workspace, index, runtime) = held("chain");
        let creator = Creator::try_from("me").unwrap();
        let message = Message::try_from("did").unwrap();
        runtime.block_on(index.write(creator.clone())).unwrap();
        runtime.block_on(index.write(message.clone())).unwrap();

        let first = version(&index, &runtime, &Version::root(), 1, &creator, &message);
        let second = version(&index, &runtime, &first, 2, &creator, &message);

        let layout = working(&workspace);
        layout
            .create_entry(
                Uuid::from_u128(1),
                MutableData::new(None, second.compute_blake3(), "art".to_owned()),
            )
            .unwrap();
        layout
            .create_path(&LayoutPath::new("art.psd").unwrap(), Uuid::from_u128(1))
            .unwrap();

        let graph = locate_version_graph(&dir, &dir.join("art.psd")).unwrap();

        // The versions are highest first: V1, then V0.
        assert_eq!(graph.versions.len(), 2);
        assert_eq!(graph.versions[0].level, 1);
        assert_eq!(graph.versions[1].level, 0);
        assert_eq!(graph.versions[0].hash.len(), 64);
        assert_eq!(graph.versions[1].hash.len(), 64);

        // A version's words are its producing variant's; V0's are the root-based variant's.
        assert_eq!(graph.versions[0].creator, "me");
        assert_eq!(graph.versions[0].message, "did");
        assert_eq!(graph.versions[1].creator, "me");
        assert_eq!(graph.versions[1].message, "did");

        // Only V1's variant is drawn: V0's is the root-based `first`, outside the levels.
        assert_eq!(graph.variants.len(), 1);
        assert_eq!(graph.variants[0].level, 0);
        assert!(graph.variants[0].spine);
        assert!(graph.variants[0].join.is_none());
        assert_eq!(graph.variants[0].hash.len(), 64);
        assert_eq!(graph.variants[0].creator, "me");
        assert_eq!(graph.variants[0].message, "did");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_the_layout_names_without_a_version_has_none() {
        let (dir, workspace, _index, _runtime) = held("untracked");

        let layout = working(&workspace);
        layout
            .create_entry(
                Uuid::from_u128(2),
                MutableData::new(None, [0; 32], "never recorded".to_owned()),
            )
            .unwrap();
        layout
            .create_path(&LayoutPath::new("new.psd").unwrap(), Uuid::from_u128(2))
            .unwrap();

        assert!(locate_version_graph(&dir, &dir.join("new.psd")).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_the_layout_does_not_name_has_none() {
        let (dir, _workspace, _index, _runtime) = held("absent");

        assert!(locate_version_graph(&dir, &dir.join("absent.psd")).is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
