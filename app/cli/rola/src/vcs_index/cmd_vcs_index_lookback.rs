//! The `rola vcs-index lookback` command: draw the chain an object sits at the top of.
//!
//! The drawing is made of two halves: [`gather`] walks the chain and says what is there — one level
//! a version, each with the variant its version points at and the variants merged in beside it, and
//! the variant itself at the top when the hash named one — and the renderer turns that into the
//! grid `rorolala_vcs::draw` lays out. What `--json` prints is [`ResultVcsIndexLookback`], which is
//! the same thing the drawing is made from.

use librorolala::storage::{Blake3Hash, Key};
use librorolala::vcs::{
    Cell, Chain, Node, Role, VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version,
    draw as draw_chain,
};
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::{Colorize as _, trd};
use rust_i18n::t;
use serde::Serialize;
use std::collections::HashMap;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexHash, ErrorVcsIndexNoIndex, ErrorVcsIndexRead, hex,
    parse_hash, runtime,
};

#[help(buffer)]
pub fn help_vcs_index_lookback(_: EntryVcsIndexLookback, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_lookback.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexLookback)]
pub fn desc_vcs_index_lookback() -> Description {
    t!("vcs_index_lookback.description").to_string().into()
}

/// Draws the chain one index object sits at the top of
///
/// A Version is drawn at the top, and the chain walked back to the first version — the root, a
/// version of its own, is not drawn — reading each version once, so the numbers fall by one a step.
/// A Variant names the same chain from the other side: it is drawn as the top itself, over the
/// version it is based on, so what is shown is how that variant came to be.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexHash`] when the hash does not read, and [`ErrorVcsIndexRead`] when the chain
/// cannot be read.
#[command(node = "vcs-index.lookback", entry = EntryVcsIndexLookback)]
pub fn vcs_index_lookback(args: EntryVcsIndexLookback) -> Next {
    let hash = match args
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "HASH".to_owned(),
            }
            .into()
        })
        .to_result()
    {
        Ok(hash) => hash,
        Err(next) => return next,
    };

    StateVcsIndexLookback { hash }.into()
}

/// The state a lookback starts in.
#[derive(Grouped)]
pub struct StateVcsIndexLookback {
    /// The hash of the object to look back from.
    hash: String,
}

#[chain(routeify)]
pub fn handle_vcs_index_lookback(
    state: StateVcsIndexLookback,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(key) = parse_hash(&state.hash) else {
        return ErrorVcsIndexHash { hash: state.hash }.into();
    };
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    // A Version is the top of the chain. A Variant is the top itself, drawn over the version it is
    // based on; anything else starts no chain.
    let (start, head) = match runtime.block_on(index.read(key)) {
        Ok(VCSIndexObject::Version(version)) => (version, None),
        Ok(VCSIndexObject::Variant(variant)) => {
            match runtime.block_on(index.read(Key::new(*variant.base_version()))) {
                Ok(VCSIndexObject::Version(version)) => (version, Some(variant)),
                _ => return not_a_chain(),
            }
        }
        Ok(_) => return not_a_chain(),
        Err(error) => {
            return ErrorVcsIndexRead {
                cause: error.reason(),
            }
            .into();
        }
    };

    match gather(index, &runtime, start, head.as_ref()) {
        Ok((levels, variant_top)) => ResultVcsIndexLookback {
            levels,
            variant_top,
        }
        .into(),
        Err(cause) => ErrorVcsIndexRead { cause }.into(),
    }
}

/// The failure of a hash that is not the top of a chain.
fn not_a_chain() -> Next {
    ErrorVcsIndexRead {
        cause: t!("vcs_index_lookback.err_not_a_chain").trim().to_string(),
    }
    .into()
}

/// Walks the chain back from `start`, gathering every variant the graph draws.
///
/// The versions come first, one to a level, each with the variant it points at; then the variants
/// merged in, reached by following each variant's `join` and placed at the level of the version
/// they are based on. The variant in `head`, when one is given, is the version `start`'s own
/// variant: it is the top of the drawing rather than the version above it.
fn gather(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    start: Version,
    head: Option<&Variant>,
) -> Result<(Vec<LevelView>, bool), String> {
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
    let Ok(count) = u64::try_from(versions.len()) else {
        return Ok((Vec::new(), false));
    };
    let top = count.saturating_sub(1);
    let Ok(levels) = usize::try_from(top) else {
        return Ok((Vec::new(), false));
    };

    // The number each version carries, so a merged variant can be placed by its base version.
    let mut number_of: HashMap<Blake3Hash, u64> = HashMap::new();
    for (at, version) in versions.iter().enumerate() {
        let at = u64::try_from(at).unwrap_or(0);
        number_of.insert(*version.hash().digest(), top - at);
    }

    // The spine of each level, highest first, and the level each variant sits at.
    let mut level_of: HashMap<Blake3Hash, u64> = HashMap::new();
    let mut spine = Vec::with_capacity(levels);
    for (at, variant) in variants.into_iter().take(levels).enumerate() {
        let at = u64::try_from(at).unwrap_or(0);
        let level = top - 1 - at;
        level_of.insert(*variant.hash().digest(), level);
        spine.push((variant, level));
    }

    // What to follow the joins of: the spine variants, and the head variant when there is one —
    // it is the top of the drawing, so it has a level of its own above the spine.
    let mut queue = spine.clone();
    if let Some(head) = head {
        level_of.insert(*head.hash().digest(), top);
        queue.push((head.clone(), top));
    }

    // The variants merged in, reached by following each variant's `join`, and the level that merges
    // each of them in — the latest merge sets where it stands.
    let mut merged_at: HashMap<u64, Vec<Variant>> = HashMap::new();
    let mut merged_by: HashMap<Blake3Hash, u64> = HashMap::new();
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
        level_of.insert(*merged.hash().digest(), at);
        merged_at.entry(at).or_default().push(merged.clone());
        queue.push((merged, at));
    }

    // One level a version, the highest first: its spine variant, then the variants merged into it,
    // left to right, the one merged in latest furthest out. The head, when there is one, is the
    // level above them all.
    let mut built = Vec::new();
    if let Some(head) = head {
        let merged = merged_at.remove(&top).unwrap_or_default();
        built.push(level_view(head, top, merged, &merged_by));
    }
    for (variant, level) in &spine {
        let merged = merged_at.remove(level).unwrap_or_default();
        built.push(level_view(variant, *level, merged, &merged_by));
    }

    Ok((built, head.is_some()))
}

/// One level, its merged variants put in draw order: the one merged in latest furthest out.
fn level_view(
    variant: &Variant,
    level: u64,
    mut merged: Vec<Variant>,
    merged_by: &HashMap<Blake3Hash, u64>,
) -> LevelView {
    merged.sort_by_cached_key(|merged| {
        (
            merged_by.get(merged.hash().digest()).copied().unwrap_or(0),
            hex(merged.hash().digest()),
        )
    });

    LevelView {
        number: level,
        variant: view(variant),
        merged: merged.iter().map(view).collect(),
    }
}

/// Reads the variant `hash` names.
fn read_variant(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &Blake3Hash,
) -> Result<Variant, String> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Variant(variant)) => Ok(variant),
        Ok(_) => Err(t!("vcs_index_lookback.err_broken").trim().to_string()),
        Err(error) => Err(error.reason()),
    }
}

/// Reads the version `hash` names.
fn read_version(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &Blake3Hash,
) -> Result<Version, String> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Version(version)) => Ok(version),
        Ok(_) => Err(t!("vcs_index_lookback.err_broken").trim().to_string()),
        Err(error) => Err(error.reason()),
    }
}

/// A variant as the graph draws it: the hash it is named by and the variant it merges in.
fn view(variant: &Variant) -> VariantView {
    VariantView {
        hash: hex(variant.hash().digest()),
        join: variant.join().map(hex),
    }
}

/// One variant, as much of it as a drawing needs: the hash it is named by, and the variant it
/// merges in when it is a merge. Who made it and what it says are not drawn, so they are not here.
#[derive(Serialize)]
pub struct VariantView {
    /// The variant's hash, as hex.
    hash: String,
    /// The hash of the variant it merges in, as hex, when it is a merge.
    join: Option<String>,
}

/// One level of the chain: a version's number, the variant the version points at, and the variants
/// merged into the chain at this level, left to right.
#[derive(Serialize)]
pub struct LevelView {
    /// The number of the version this level stands for.
    number: u64,
    /// The spine variant: the one the version points at, or the variant the chain was started at.
    variant: VariantView,
    /// The variants merged in at this level, left to right, the one merged in latest furthest out.
    merged: Vec<VariantView>,
}

/// Result: the chain, one level a version, the highest version's first.
///
/// It is what a drawing needs and no more — a level's number, the variant its version points at,
/// and the variants merged in beside it, and whether the top is a variant of its own rather than a
/// version's number — so what a `--json` run prints is the graph itself.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexLookback {
    /// The levels, the highest version's first.
    levels: Vec<LevelView>,
    /// Whether the top row is a variant rather than the version above it.
    variant_top: bool,
}

/// The 7 hex characters a hash is drawn by in the graph.
fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

#[renderer(buffer)]
pub fn render_result_vcs_index_lookback(result: ResultVcsIndexLookback) {
    for row in draw_chain(&chain_of(&result)) {
        let mut line = String::new();

        for cell in &row {
            match cell {
                Cell::Blank => line.push(' '),
                Cell::Glyph(Role::Version, glyph) => {
                    line.push_str(&glyph.to_string().bright_magenta().to_string());
                }
                Cell::Glyph(Role::Variant, glyph) => {
                    line.push_str(&glyph.to_string().bright_cyan().to_string());
                }
                Cell::Glyph(Role::Edge, glyph) => {
                    line.push_str(&glyph.to_string().bold().white().to_string());
                }
            }
        }

        r_println!("{}", line.trim_end());
    }
}

/// The chain the drawing is made from: the levels turned into the nodes the layout reaches for.
fn chain_of(result: &ResultVcsIndexLookback) -> Chain {
    // Where each variant is drawn, so a join can be turned into the node it reaches.
    let mut at: HashMap<&str, usize> = HashMap::new();
    let mut count = 0;
    for level in &result.levels {
        at.insert(level.variant.hash.as_str(), count);
        count += 1;
        for merged in &level.merged {
            at.insert(merged.hash.as_str(), count);
            count += 1;
        }
    }

    let mut nodes = Vec::with_capacity(count);
    for level in &result.levels {
        nodes.push(node_of(&level.variant, level.number, true, &at));
        for merged in &level.merged {
            nodes.push(node_of(merged, level.number, false, &at));
        }
    }

    // A version at the top is one past the highest level; a variant at the top is the highest
    // level itself.
    let top = result
        .levels
        .iter()
        .map(|level| level.number)
        .max()
        .map_or(0, |number| {
            if result.variant_top {
                number
            } else {
                number + 1
            }
        });

    if result.variant_top {
        Chain::variant_top(top, nodes)
    } else {
        Chain::new(top, nodes)
    }
}

/// One node of the drawing: its label the leading characters of the variant's hash, its join the
/// node that hash names.
fn node_of(view: &VariantView, level: u64, spine: bool, at: &HashMap<&str, usize>) -> Node {
    let join = view.join.as_deref().and_then(|hash| at.get(hash).copied());

    Node::new(short(&view.hash).to_owned(), level, spine, join)
}
