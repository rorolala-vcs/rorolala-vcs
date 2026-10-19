//! The chain an index object sits at the top of, as a drawing reads it.
//!
//! The graph itself is [`crate::draw`]'s business; what this module does is walk the index and say
//! what is there — one level a version, each with the variant its version points at and the
//! variants merged in beside it, and the variant itself at the top when the object named one. Both
//! `rola vcs-index lookback` and the Desktop's Version Graph read the chain this way, so the walk is
//! kept here rather than in each of them.

use std::collections::{HashMap, HashSet};

use rorolala_storage::Key;
use serde::Serialize;

use crate::{
    Chain, ComputeBlake3, Node, Top, VCSIndex, VCSIndexObject, VCSIndexReadingError, Variant,
    Version,
};

/// One variant, as much of it as a drawing needs: the hash it is named by, the variant it merges
/// in when it is a merge, and the words it carries.
#[derive(Debug, Clone, Serialize)]
pub struct VariantView {
    /// The variant's hash, as hex.
    pub hash: String,
    /// The hash of the variant it merges in, as hex, when it is a merge.
    pub join: Option<String>,
    /// Who made it, as text; empty when the index does not hold it.
    pub creator: String,
    /// What it says, as text; empty when the index does not hold it.
    pub message: String,
}

/// What the top of a chain is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopKind {
    /// The highest version's number is known.
    Version,
    /// The highest version's number is unknown: the chain was started from a variant.
    Unknown,
    /// The highest version is being edited: changes to it were never recorded.
    Editing,
}

/// One level of the chain: a version's number, the variant the version points at, and the variants
/// merged into the chain at this level, left to right.
#[derive(Debug, Clone, Serialize)]
pub struct LevelView {
    /// The number of the version this level stands for.
    pub number: u64,
    /// The spine variant: the one the version points at, or the variant the chain was started at.
    pub variant: VariantView,
    /// The variants merged in at this level, left to right, the one merged in latest furthest out.
    pub merged: Vec<VariantView>,
}

/// Where a Vault's own Layout stands in the chain being drawn: the version it records for the entry
/// and the Vault's name.
#[derive(Debug, Clone, Serialize)]
pub struct Origin {
    /// The number the version has in this chain.
    pub number: u64,
    /// The Vault the version is recorded in.
    pub vault: String,
}

/// The chain, one level a version, the highest version's first.
#[derive(Debug, Clone)]
pub struct Lookback {
    /// The levels, the highest version's first.
    pub levels: Vec<LevelView>,
    /// What the top of the chain is.
    pub top: TopKind,
    /// The version a Vault records for the entry, when one was named and this chain holds it.
    pub origin: Option<Origin>,
    /// The variant that made the oldest version, when the chain has one.
    ///
    /// It is the one based on the root, which is drawn at no level and so is not among the levels;
    /// its words are the oldest version's own, which the drawing would otherwise leave out.
    pub first: Option<VariantView>,
    /// The full hash of each version in the chain, by the version's number.
    ///
    /// The drawing is by variant, but a caller that shows where a version itself stands — the
    /// Desktop's Version Graph does — needs the hash the index names that version by.
    pub version_hashes: HashMap<u64, String>,
}

/// One unjoined variant with the version it is based on, as a flat listing names it.
#[derive(Debug, Clone)]
pub struct BasedVariant {
    /// The number of the version the variant is based on.
    pub number: u64,
    /// The hash of the version the variant is based on, as hex.
    pub version: String,
    /// The variant itself.
    pub variant: VariantView,
}

impl Lookback {
    /// Every unjoined variant based on a version of this chain, the highest base version first.
    ///
    /// The index is scanned rather than walked: a variant the chain draws nowhere is reached from no
    /// link in the chain. What the chain draws is its spine and the variants merged in beside it, so a
    /// variant is unjoined when it is neither — and it belongs to this file when the version it is
    /// based on is one the chain holds.
    ///
    /// # Errors
    ///
    /// Returns [`LookbackError::Read`] when the index's objects cannot be read.
    pub fn unjoined_variants(
        &self,
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
    ) -> Result<Vec<BasedVariant>, LookbackError> {
        // The versions by the hex a variant names them with, since a variant carries the digest rather
        // than the number the chain worked out.
        let versions: HashMap<&str, (u64, &str)> = self
            .version_hashes
            .iter()
            .map(|(number, hash)| (hash.as_str(), (*number, hash.as_str())))
            .collect();

        // What the chain draws: every spine variant, every variant merged in beside one, and the
        // variant that made the oldest version, which hangs from the root rather than a level.
        let drawn: HashSet<&str> = self
            .levels
            .iter()
            .flat_map(|level| std::iter::once(&level.variant).chain(level.merged.iter()))
            .chain(self.first.iter())
            .map(|variant| variant.hash.as_str())
            .collect();

        let objects = runtime
            .block_on(index.read_objects())
            .map_err(LookbackError::Read)?;
        let mut found = Vec::new();

        for (key, object) in objects {
            let VCSIndexObject::Variant(variant) = object else {
                continue;
            };

            let hash = key.hex();

            if drawn.contains(hash.as_str()) {
                continue;
            }

            let base = Key::new(*variant.base_version()).hex();
            let Some((number, version)) = versions.get(base.as_str()) else {
                continue;
            };

            found.push(BasedVariant {
                number: *number,
                version: (*version).to_owned(),
                variant: view(index, runtime, &variant),
            });
        }

        found.sort_by(|left, right| {
            right
                .number
                .cmp(&left.number)
                .then_with(|| left.variant.hash.cmp(&right.variant.hash))
        });

        Ok(found)
    }
}

/// Why a chain could not be read.
#[derive(Debug)]
pub enum LookbackError {
    /// What was named is not the top of a version chain.
    NotAChain,
    /// The index does not hold the version or variant the chain needs.
    Broken,
    /// The index itself could not be read.
    Read(VCSIndexReadingError),
}

/// The chain a lookback draws, as [`crate::draw`] takes it.
///
/// `has_first` says whether the oldest version's variant is held apart — see
/// [`Lookback::first`] — because a chain that recorded nothing has no such variant and its highest
/// number is one lower.
#[must_use]
pub fn chain_of(levels: &[LevelView], top_kind: TopKind, has_first: bool) -> Chain {
    // Where each variant is drawn, so a join can be turned into the node it reaches.
    let mut at: HashMap<&str, usize> = HashMap::new();
    let mut count = 0;
    for level in levels {
        at.insert(level.variant.hash.as_str(), count);
        count += 1;
        for merged in &level.merged {
            at.insert(merged.hash.as_str(), count);
            count += 1;
        }
    }

    let mut nodes = Vec::with_capacity(count);
    for level in levels {
        nodes.push(node_of(&level.variant, level.number, true, &at));
        for merged in &level.merged {
            nodes.push(node_of(merged, level.number, false, &at));
        }
    }

    // The number the highest recorded version carries: one version is number 0, and every version
    // over the oldest — a level of the drawing, or the head a chain was started from — adds one.
    let recorded = u64::try_from(levels.len())
        .unwrap_or(u64::MAX)
        .saturating_add(u64::from(has_first))
        .saturating_sub(1);

    // The version a chain is looked back from a variant is one the index does not number, and one
    // being edited is a version it does not have at all: both are a level of their own over the
    // highest one drawn, so `V{n}` numbers what was recorded and the placeholder stands above it.
    let number = match top_kind {
        TopKind::Editing => recorded.saturating_add(1),
        _ => recorded,
    };
    let top = match top_kind {
        TopKind::Version => Top::Numbered,
        TopKind::Unknown => Top::Unknown,
        TopKind::Editing => {
            // The variant the work is on its way to is not one the index has either, so the level
            // it will be made by is drawn as a placeholder of its own.
            nodes.push(Node::new(
                "??".to_owned(),
                number.saturating_sub(1),
                true,
                None,
            ));
            Top::Editing
        }
    };

    Chain::new(number, top, nodes)
}

/// One node of the drawing: its label the leading characters of the variant's hash, its join the
/// node that hash names.
fn node_of(view: &VariantView, level: u64, spine: bool, at: &HashMap<&str, usize>) -> Node {
    let join = view.join.as_deref().and_then(|hash| at.get(hash).copied());

    Node::new(short_hash(&view.hash).to_owned(), level, spine, join)
}

/// The 7 hex characters a variant is drawn by.
///
/// It is the one shortening the drawing and the lines beside it use, so a hash named in a note is
/// the same head a label is drawn by.
#[must_use]
pub fn short_hash(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

/// The chain the index object `key` names sits at the top of.
///
/// A Version is the top of the chain. A Variant is the top itself, over the version it is based on;
/// anything else starts no chain. `origin` is the version a Vault's own Layout records for the
/// entry being drawn, with the Vault's name, so that where the Vault stands in the chain can be
/// marked; a chain drawn by name alone names no Vault and marks nothing.
///
/// # Errors
///
/// Returns why the object is not the top of a chain, or why the chain could not be read.
pub fn read_chain(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    key: Key,
    origin: Option<(&str, [u8; 32])>,
) -> Result<Lookback, LookbackError> {
    let (start, head) = match runtime.block_on(index.read(key)) {
        Ok(VCSIndexObject::Version(version)) => (version, None),
        Ok(VCSIndexObject::Variant(variant)) => {
            match runtime.block_on(index.read(Key::new(*variant.base_version()))) {
                Ok(VCSIndexObject::Version(version)) => (version, Some(variant)),
                _ => return Err(LookbackError::NotAChain),
            }
        }
        Ok(_) => return Err(LookbackError::NotAChain),
        Err(error) => return Err(LookbackError::Read(error)),
    };

    gather(index, runtime, start, head.as_ref(), origin)
}

/// Walks the chain back from `start`, gathering every variant the graph draws.
///
/// The versions come first, one to a level, each with the variant it points at; then the variants
/// merged in, reached by following each variant's `join` and placed at the level of the version
/// they are based on. The variant in `head`, when one is given, is the version `start`'s own
/// variant: it is the variant of the version above `start`, whose number nothing here knows.
fn gather(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    start: Version,
    head: Option<&Variant>,
    origin: Option<(&str, [u8; 32])>,
) -> Result<Lookback, LookbackError> {
    let mut versions = Vec::new();
    let mut variants = Vec::new();
    let mut current = start;

    while !current.is_root() {
        let variant = read_variant(index, runtime, current.variant())?;
        let base = read_version(index, runtime, variant.base_version())?;
        variants.push(variant);
        versions.push(current);
        current = base;
    }

    // The number of the highest version, one less than the number of versions down to the first.
    let count = u64::try_from(versions.len()).unwrap_or(u64::MAX);
    let top = count.saturating_sub(1);
    let levels = usize::try_from(top).unwrap_or(usize::MAX);

    // The number each version carries, so a merged variant can be placed by its base version.
    let mut number_of: HashMap<[u8; 32], u64> = HashMap::new();
    for (at, version) in versions.iter().enumerate() {
        let at = u64::try_from(at).unwrap_or(0);
        number_of.insert(version.compute_blake3(), top.saturating_sub(at));
    }

    // Where the Vault being tracked stands: the version it records, when this chain holds it too.
    // A Vault ahead of the work, or one never told of the entry, records a version that is not here,
    // and there is nothing to mark.
    let origin = origin.and_then(|(vault, version)| {
        number_of.get(&version).map(|number| Origin {
            number: *number,
            vault: vault.to_owned(),
        })
    });

    // The words of the oldest version come from the variant that made it, which is based on the root
    // — a version that is never drawn — so it hangs from no level. It is kept apart here so that
    // version is drawn saying what it was, like every other.
    let first = variants.last().map(|variant| view(index, runtime, variant));

    // The spine of each level, highest first, and the level each variant sits at.
    let mut level_of: HashMap<[u8; 32], u64> = HashMap::new();
    let mut spine = Vec::with_capacity(levels);
    for (at, variant) in variants.into_iter().take(levels).enumerate() {
        let at = u64::try_from(at).unwrap_or(0);
        let level = top.saturating_sub(1).saturating_sub(at);
        level_of.insert(variant.compute_blake3(), level);
        spine.push((variant, level));
    }

    // What to follow the joins of: the spine variants, and the head variant when there is one —
    // it is the variant of the level above the spine.
    let mut queue = spine.clone();
    if let Some(head) = head {
        level_of.insert(head.compute_blake3(), top);
        queue.push((head.clone(), top));
    }

    // The variants merged in, reached by following each variant's `join`, and the level that merges
    // each of them in — the latest merge sets where it stands.
    let mut merged_at: HashMap<u64, Vec<Variant>> = HashMap::new();
    let mut merged_by: HashMap<[u8; 32], u64> = HashMap::new();
    while let Some((variant, level)) = queue.pop() {
        let Some(join) = variant.join() else {
            continue;
        };
        merged_by
            .entry(*join)
            .and_modify(|highest| *highest = (*highest).max(level))
            .or_insert(level);

        // A variant that is already drawn — a spine, the head, or one merged in before — is
        // reached, not gathered a second time.
        if level_of.contains_key(join) {
            continue;
        }
        let Ok(merged) = read_variant(index, runtime, join) else {
            continue;
        };
        // Its place is the level of its base version. A variant based on the root has no version
        // above it to sit under, so it has no level and is left out — the root is never drawn.
        let Some(&at) = number_of.get(merged.base_version()) else {
            continue;
        };
        level_of.insert(merged.compute_blake3(), at);
        merged_at.entry(at).or_default().push(merged.clone());
        queue.push((merged, at));
    }

    // One level a version, the highest first: its spine variant, then the variants merged into it,
    // left to right, the one merged in latest furthest out. The head, when there is one, is the
    // level above them all.
    let mut built = Vec::new();
    if let Some(head) = head {
        let merged = merged_at.remove(&top).unwrap_or_default();
        built.push(level_view(index, runtime, head, top, merged, &merged_by));
    }
    for (variant, level) in &spine {
        let merged = merged_at.remove(level).unwrap_or_default();
        built.push(level_view(
            index, runtime, variant, *level, merged, &merged_by,
        ));
    }

    let mut version_hashes: HashMap<u64, String> = number_of
        .iter()
        .map(|(hash, number)| (*number, Key::new(*hash).hex()))
        .collect();

    // A chain whose start is the root has no version in the walk, but it is still the oldest
    // version drawn: its own hash is the one the root is named by.
    if version_hashes.is_empty() {
        version_hashes.insert(0, Key::new(current.compute_blake3()).hex());
    }

    Ok(Lookback {
        levels: built,
        top: if head.is_some() {
            TopKind::Unknown
        } else {
            TopKind::Version
        },
        origin,
        first,
        version_hashes,
    })
}

/// One level, its merged variants put in draw order: the one merged in latest furthest out.
fn level_view(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    variant: &Variant,
    level: u64,
    mut merged: Vec<Variant>,
    merged_by: &HashMap<[u8; 32], u64>,
) -> LevelView {
    merged.sort_by_cached_key(|merged| {
        (
            merged_by
                .get(&merged.compute_blake3())
                .copied()
                .unwrap_or(0),
            Key::new(merged.compute_blake3()).hex(),
        )
    });

    LevelView {
        number: level,
        variant: view(index, runtime, variant),
        merged: merged
            .iter()
            .map(|merged| view(index, runtime, merged))
            .collect(),
    }
}

/// Reads the variant `hash` names.
fn read_variant(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &[u8; 32],
) -> Result<Variant, LookbackError> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Variant(variant)) => Ok(variant),
        Ok(_) => Err(LookbackError::Broken),
        Err(error) => Err(LookbackError::Read(error)),
    }
}

/// Reads the version `hash` names.
fn read_version(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &[u8; 32],
) -> Result<Version, LookbackError> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Version(version)) => Ok(version),
        Ok(_) => Err(LookbackError::Broken),
        Err(error) => Err(LookbackError::Read(error)),
    }
}

/// The text the object `hash` names, or nothing when the index does not hold it.
///
/// A variant names who made it and what it says by hash, so a drawing that says either has to read
/// the object behind it. One the index does not hold is left empty rather than failing the drawing:
/// what the chain is made of is what is being looked at, and the words beside it are what may be
/// missing — a copy of an index that came over without them still has a chain to draw.
fn read_text(index: &VCSIndex, runtime: &tokio::runtime::Runtime, hash: &[u8; 32]) -> String {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Creator(creator)) => creator.read_to_string().to_owned(),
        Ok(VCSIndexObject::Message(message)) => message.read_to_string().to_owned(),
        _ => String::new(),
    }
}

/// A variant as the graph draws it: the hashes it is named by, and the words that go with it.
fn view(index: &VCSIndex, runtime: &tokio::runtime::Runtime, variant: &Variant) -> VariantView {
    VariantView {
        hash: Key::new(variant.compute_blake3()).hex(),
        join: variant.join().map(|hash| Key::new(*hash).hex()),
        creator: read_text(index, runtime, variant.creator()),
        message: read_text(index, runtime, variant.message()),
    }
}
