//! The `rola vcs-index lookback` command: draw the chain an object sits at the top of.
//!
//! The drawing is made of two halves: [`gather`] walks the chain and says what is there — one level
//! a version, each with the variant its version points at and the variants merged in beside it, and
//! the variant itself at the top when the hash named one — and the renderer turns that into the
//! grid `rorolala_vcs::draw` lays out. What `--json` prints is [`ResultVcsIndexLookback`], which is
//! the same thing the drawing is made from.
//!
//! A version is what a message belongs to: it points at one variant, and that variant carries who
//! made it and what it says — both by hash, at a [`Creator`](librorolala::vcs::Creator) and a
//! [`Message`](librorolala::vcs::Message) in the index. So what a level is drawn with is read from
//! the variant its version points at, and the drawing is [rola status](crate::cmd_status)'s too:
//! what it draws for a file is this, with [`TopKind::Editing`] over it when the file has changes
//! that were never recorded.

use std::collections::HashMap;

use librorolala::storage::{Blake3Hash, Key};
use librorolala::vcs::{
    Cell, Chain, Node, Role, Top, VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version,
    draw as draw_chain,
};
use mingling::{
    Grouped, LazyRes, ProgramCollect, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
    setup::ProgramSetup,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::{Colorize as _, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::complete::{
    IndexObject, filling_flag, index_hashes, offer, positional, strip_written, typing_flag,
};
use crate::exit_codes::EC_HELP;
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexHash, ErrorVcsIndexNoIndex, ErrorVcsIndexRead, hex,
    parse_hash, runtime,
};

/// How much of a message a drawing shows before it is cut short, when the run names no length.
pub const DEFAULT_MESSAGE_LENGTH: usize = 64;

/// What a message that was cut short ends with.
const ELLIPSIS: &str = "...";

/// Draw the versions alone, without the variants between them.
pub const ARG_COMPACT: PickerArg<'static, Flag> = arg![compact: Flag];

/// Leave out what each version says.
pub const ARG_NO_MESSAGE: PickerArg<'static, Flag> = arg![no_message: Flag];

/// Leave out who made each version.
pub const ARG_NO_CREATOR: PickerArg<'static, Flag> = arg![no_creator: Flag];

/// How much of a message is shown before it is cut short.
pub const ARG_MAX_MESSAGE_LENGTH: PickerArg<'static, Option<usize>> =
    arg![max_message_length: Option<usize>];

/// What a lookback is drawn as, by the run that asked for it.
///
/// A resource rather than state, since it is read where the drawing is made — the renderer — and
/// set where the run's flags are read, which for `rola status` is a command that draws a lookback
/// only when it was given something to look back from.
// The fields are the flags a run names, one each, and they are independent of one another: a drawing
// with no versions between them may still say who made each, and a file may be shown without its
// chain. A bitfield or an enum of shapes would say less than the run asked for.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct ResLookback {
    /// Whether the variants between the versions are left out.
    compact: bool,
    /// Whether what each version says is shown.
    message: bool,
    /// Whether who made each version is shown.
    creator: bool,
    /// How much of a message is shown before it is cut short.
    max_message_length: usize,
    /// Whether the chain itself is drawn.
    graph: bool,
    /// Whether the lines that say where the file comes from and how it stands are drawn.
    hint: bool,
}

impl Default for ResLookback {
    fn default() -> Self {
        Self {
            compact: false,
            message: true,
            creator: true,
            max_message_length: DEFAULT_MESSAGE_LENGTH,
            graph: true,
            hint: true,
        }
    }
}

impl ResLookback {
    /// Whether the variants between the versions are left out.
    #[must_use]
    pub const fn compact(&self) -> bool {
        self.compact
    }

    /// Whether what each version says is shown.
    #[must_use]
    pub const fn message(&self) -> bool {
        self.message
    }

    /// Whether who made each version is shown.
    #[must_use]
    pub const fn creator(&self) -> bool {
        self.creator
    }

    /// How much of a message is shown before it is cut short.
    #[must_use]
    pub const fn max_message_length(&self) -> usize {
        self.max_message_length
    }

    /// Whether the chain itself is drawn.
    #[must_use]
    pub const fn graph(&self) -> bool {
        self.graph
    }

    /// Whether the lines that say where the file comes from and how it stands are drawn.
    #[must_use]
    pub const fn hint(&self) -> bool {
        self.hint
    }

    /// Takes what `rola status` asked for from the two flags only it has.
    ///
    /// A chain drawn by a hash has no file to say anything about, so neither line is ever drawn for
    /// one however these are set.
    pub const fn asked_status(&mut self, no_graph: Flag, no_hint: Flag) {
        self.graph = !matches!(no_graph, Flag::Active);
        self.hint = !matches!(no_hint, Flag::Active);
    }

    /// Takes what the run asked for from the flags it gave.
    ///
    /// It is what a command calls as it is reached, so that the drawing a renderer makes is the one
    /// the run named — whichever of the two commands that draw a lookback it was.
    pub fn asked(
        &mut self,
        compact: Flag,
        no_message: Flag,
        no_creator: Flag,
        max_message_length: Option<usize>,
    ) {
        self.compact = matches!(compact, Flag::Active);
        self.message = !matches!(no_message, Flag::Active);
        self.creator = !matches!(no_creator, Flag::Active);

        if let Some(length) = max_message_length {
            self.max_message_length = length;
        }
    }
}

/// A [`ProgramSetup`] that gives every run a lookback to draw with.
pub struct LookbackSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for LookbackSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        program.with_resource(ResLookback::default());
    }
}

#[help(buffer)]
pub fn help_vcs_index_lookback(_: EntryVcsIndexLookback, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_lookback.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexLookback)]
pub fn desc_vcs_index_lookback() -> Description {
    t!("vcs_index_lookback.description").to_string().into()
}

/// Completes what `rola vcs-index lookback` can be given next.
///
/// The hash is the top of a chain, which is a version or a variant — a variant sits over the version
/// it is based on — so both kinds are offered. `--max-message-length` takes a number, which no list
/// of hashes answers.
#[completion(EntryVcsIndexLookback)]
pub fn complete_vcs_index_lookback(ctx: ShellContext, index: &mut LazyRes<ResVCSIndex>) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_COMPACT: t!("vcs_index_lookback.complete.compact"),
                ARG_NO_MESSAGE: t!("vcs_index_lookback.complete.no_message"),
                ARG_NO_CREATOR: t!("vcs_index_lookback.complete.no_creator"),
                ARG_MAX_MESSAGE_LENGTH: t!("vcs_index_lookback.complete.max_message_length"),
            },
        );
    }

    if filling_flag(&ctx, &ARG_MAX_MESSAGE_LENGTH) || positional(&ctx, "lookback") != 0 {
        return suggest!();
    }

    let held = index.get_ref().as_ref();
    let mut names = index_hashes(held, IndexObject::Version);
    names.extend(index_hashes(held, IndexObject::Variant));

    offer(&ctx, names)
}

/// Draws the chain one index object sits at the top of
///
/// A Version is drawn at the top, and the chain walked back to the first version — the root, a
/// version of its own, is not drawn — reading each version once, so the numbers fall by one a step.
/// A Variant names the same chain from the other side: it is drawn over the version it is based on,
/// under a version whose number nothing here knows, drawn `?`.
///
/// Each version is drawn with who made it and what it says, both read from the variant it points
/// at, since a version is what a message belongs to. `--no-creator` and `--no-message` leave one of
/// them out, `--max-message-length` cuts a message short, and `--compact` draws the versions alone.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexHash`] when the hash does not read, [`ErrorVcsIndexArgument`] when
/// `--max-message-length` does not read, and [`ErrorVcsIndexRead`] when the chain cannot be read.
#[command(node = "vcs-index.lookback", entry = EntryVcsIndexLookback)]
pub fn vcs_index_lookback(args: EntryVcsIndexLookback, lookback: &mut ResLookback) -> Next {
    let picked = args
        .pick(&ARG_COMPACT)
        .pick(&ARG_NO_MESSAGE)
        .pick(&ARG_NO_CREATOR)
        .pick(&ARG_MAX_MESSAGE_LENGTH)
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "HASH".to_owned(),
            }
            .into()
        })
        .to_result();
    let (compact, no_message, no_creator, max_message_length, hash) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    lookback.asked(compact, no_message, no_creator, max_message_length);

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

    match from_object(index, &runtime, key, None) {
        Ok(result) => result.into(),
        Err(cause) => ErrorVcsIndexRead { cause }.into(),
    }
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
pub fn from_object(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    key: Key,
    origin: Option<(&str, Blake3Hash)>,
) -> Result<ResultVcsIndexLookback, String> {
    let (start, head) = match runtime.block_on(index.read(key)) {
        Ok(VCSIndexObject::Version(version)) => (version, None),
        Ok(VCSIndexObject::Variant(variant)) => {
            match runtime.block_on(index.read(Key::new(*variant.base_version()))) {
                Ok(VCSIndexObject::Version(version)) => (version, Some(variant)),
                _ => return Err(not_a_chain()),
            }
        }
        Ok(_) => return Err(not_a_chain()),
        Err(error) => return Err(error.reason()),
    };

    gather(index, runtime, start, head.as_ref(), origin)
}

/// Why a hash that is not the top of a chain is not one.
fn not_a_chain() -> String {
    t!("vcs_index_lookback.err_not_a_chain").trim().to_string()
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
    origin: Option<(&str, Blake3Hash)>,
) -> Result<ResultVcsIndexLookback, String> {
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
    let mut number_of: HashMap<Blake3Hash, u64> = HashMap::new();
    for (at, version) in versions.iter().enumerate() {
        let at = u64::try_from(at).unwrap_or(0);
        number_of.insert(*version.hash().digest(), top.saturating_sub(at));
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
    let mut level_of: HashMap<Blake3Hash, u64> = HashMap::new();
    let mut spine = Vec::with_capacity(levels);
    for (at, variant) in variants.into_iter().take(levels).enumerate() {
        let at = u64::try_from(at).unwrap_or(0);
        let level = top.saturating_sub(1).saturating_sub(at);
        level_of.insert(*variant.hash().digest(), level);
        spine.push((variant, level));
    }

    // What to follow the joins of: the spine variants, and the head variant when there is one —
    // it is the variant of the level above the spine.
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
        built.push(level_view(index, runtime, head, top, merged, &merged_by));
    }
    for (variant, level) in &spine {
        let merged = merged_at.remove(level).unwrap_or_default();
        built.push(level_view(
            index, runtime, variant, *level, merged, &merged_by,
        ));
    }

    Ok(ResultVcsIndexLookback {
        levels: built,
        top: if head.is_some() {
            TopKind::Unknown
        } else {
            TopKind::Version
        },
        origin,
        first,
        standing: None,
    })
}

/// Where a Vault's own Layout stands in the chain being drawn: the version it records for the entry
/// and the Vault's name.
#[derive(Serialize)]
pub struct Origin {
    /// The number the version has in this chain.
    number: u64,
    /// The Vault the version is recorded in.
    vault: String,
}

/// How the file a chain was drawn for stands, as `rola status` reads it.
///
/// It is the data behind the two lines drawn above the chain — where the file is mapped from and how
/// it stands — so a `--json` run reads the same thing a person is shown, and the words are put to it
/// where it is drawn rather than here.
#[derive(Serialize)]
pub struct Standing {
    /// The Vault the entry is tracked in.
    pub vault: String,
    /// The path the Vault's own Layout names it by, when it names one.
    pub source: Option<String>,
    /// Who holds it in the Vault, as a name; nothing when nobody does.
    pub holder: Option<String>,
    /// Whether the run's own account is the holder.
    pub mine: bool,
    /// Whether the tree holds changes the Layout does not name.
    pub modified: bool,
    /// How the version this Layout is at stands beside the Vault's.
    pub relation: StandingRelation,
    /// How many versions apart the two are, when one is below the other.
    pub distance: Option<u64>,
    /// The variant waiting to be joined into this file, when the work is in a merge.
    pub merging: Option<MergeHint>,
}

/// A variant waiting to be joined into the file a chain was drawn for.
#[derive(Serialize)]
pub struct MergeHint {
    /// The variant's hash, as hex.
    pub variant: String,
    /// The path its file lies at.
    pub path: String,
}

/// How the version a Layout is at stands beside the Vault's.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StandingRelation {
    /// Both are at the same version.
    Same,
    /// The Vault's version is on the chain below this one.
    Ahead,
    /// This version is on the chain below the Vault's.
    Behind,
    /// Neither is below the other.
    Apart,
    /// The index does not hold one of them, so how far apart they are is not known here.
    Unknown,
}

/// One level, its merged variants put in draw order: the one merged in latest furthest out.
fn level_view(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
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

/// The text the object `hash` names says, or nothing when the index does not hold it.
///
/// A variant names who made it and what it says by hash, so a drawing that says either has to read
/// the object behind it. One the index does not hold is left empty rather than failing the drawing:
/// what the chain is made of is what is being looked at, and the words beside it are what may be
/// missing — a copy of an index that came over without them still has a chain to draw.
fn read_text(index: &VCSIndex, runtime: &tokio::runtime::Runtime, hash: &Blake3Hash) -> String {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Creator(creator)) => creator.read_to_string().to_owned(),
        Ok(VCSIndexObject::Message(message)) => message.read_to_string().to_owned(),
        _ => String::new(),
    }
}

/// A variant as the graph draws it: the hashes it is named by, and the words that go with it.
fn view(index: &VCSIndex, runtime: &tokio::runtime::Runtime, variant: &Variant) -> VariantView {
    VariantView {
        hash: hex(variant.hash().digest()),
        join: variant.join().map(hex),
        creator: read_text(index, runtime, variant.creator()),
        message: read_text(index, runtime, variant.message()),
    }
}

/// One variant, as much of it as a drawing needs: the hash it is named by, the variant it merges
/// in when it is a merge, and the words it carries.
#[derive(Serialize)]
pub struct VariantView {
    /// The variant's hash, as hex.
    hash: String,
    /// The hash of the variant it merges in, as hex, when it is a merge.
    join: Option<String>,
    /// Who made it, as text; empty when the index does not hold it.
    creator: String,
    /// What it says, as text; empty when the index does not hold it.
    message: String,
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
/// and the variants merged in beside it, and what the top of the chain is — so what a `--json` run
/// prints is the graph itself.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexLookback {
    /// The levels, the highest version's first.
    levels: Vec<LevelView>,
    /// What the top of the chain is.
    top: TopKind,
    /// The version a Vault records for the entry, when one was named and this chain holds it.
    origin: Option<Origin>,
    /// The variant that made the oldest version, when the chain has one.
    ///
    /// It is the one based on the root, which is drawn at no level and so is not among the levels;
    /// its words are the oldest version's own, which the drawing would otherwise leave out.
    first: Option<VariantView>,
    /// How the file the chain was drawn for stands, when it was drawn for one.
    standing: Option<Standing>,
}

impl ResultVcsIndexLookback {
    /// Says the highest version is being edited, over the chain as it was recorded.
    ///
    /// It is what `rola status` says of a file whose changes were never recorded: the work is on its
    /// way to a version the index does not have, and drawing that is drawing one more level over
    /// the chain the file's own version sits at.
    pub const fn editing(&mut self) {
        self.top = TopKind::Editing;
    }

    /// Says how the file the chain was drawn for stands, for the two lines drawn above it.
    pub fn standing(&mut self, standing: Standing) {
        self.standing = Some(standing);
    }
}

/// The 7 hex characters a hash is drawn by in the graph.
fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

#[renderer(buffer)]
pub fn render_result_vcs_index_lookback(result: ResultVcsIndexLookback, lookback: &ResLookback) {
    let hints = if lookback.hint() {
        hints(&result)
    } else {
        Vec::new()
    };

    // The lines above the chain, one blank line apart — and one more between them and the chain
    // itself, so what a file is reads apart from the versions it has.
    for (at, hint) in hints.iter().enumerate() {
        if at > 0 {
            r_println!("");
        }

        r_println!("{hint}");
    }

    if lookback.graph() {
        if !hints.is_empty() {
            r_println!("");
        }

        let chain = chain_of(&result);
        let drawing = draw_chain(&chain);
        let notes = notes(&result, lookback);
        let mut level_of: HashMap<usize, u64> = drawing
            .versions()
            .iter()
            .map(|version| (version.row(), version.level()))
            .collect();

        for (index, row) in drawing.rows().iter().enumerate() {
            // `--compact` draws the versions alone: what is between them is what a reader who wants
            // the chain's shape already has from the numbers.
            let Some(level) = level_of.remove(&index) else {
                if lookback.compact() {
                    continue;
                }

                r_println!("{}", drawn(row));
                continue;
            };

            // The version the Vault records is the one a run reading this is looking for: where the
            // Vault stands is what says whether the work has been told to it. It is drawn as a line
            // of its own colour, so it is found without reading every number.
            if let Some(origin) = result
                .origin
                .as_ref()
                .filter(|origin| origin.number == level)
            {
                let note = notes.get(&level);

                r_println!("{}", marked(row, &origin.vault, note));
                continue;
            }

            match notes.get(&level) {
                Some(note) => r_println!("{} {}", drawn(row), note),
                None => r_println!("{}", drawn(row)),
            }
        }
    }
}

/// The lines drawn above the chain: where the file is mapped from, and how it stands.
///
/// A chain drawn by a hash has no file behind it and says neither. The state is drawn in a colour of
/// its own — white when the two sides stand as they should, bright yellow when they do not — so what
/// is wrong reads before the words do.
fn hints(result: &ResultVcsIndexLookback) -> Vec<String> {
    let Some(standing) = &result.standing else {
        return Vec::new();
    };

    let mut lines = Vec::with_capacity(3);

    if let Some(source) = &standing.source {
        lines.push(
            t!("status.source", vault = standing.vault, path = source)
                .trim()
                .to_owned(),
        );
    }

    lines.push(standing_state(standing));

    // A variant waiting to be joined is drawn last: it is what the next `rola track` would record,
    // so it reads below how the file stands rather than before it.
    if let Some(merging) = &standing.merging {
        lines.push(
            t!(
                "status.merging_into",
                variant = short(&merging.variant),
                path = &merging.path
            )
            .trim()
            .to_owned(),
        );
    }

    lines
}

/// How the file stands, as the words that go beside its chain, in the colour of whether it should.
fn standing_state(standing: &Standing) -> String {
    let said = state_words(standing);

    if anomalous(standing) {
        said.bright_yellow().to_string()
    } else {
        said.white().to_string()
    }
}

/// What the state line says, from who holds the file and how it stands.
fn state_words(standing: &Standing) -> String {
    let holder = holder_words(standing);
    let distance = standing.distance.unwrap_or(0);

    let said = match (standing.mine, standing.relation, standing.modified) {
        (true, StandingRelation::Same, false) => t!("status.state_mine_same"),
        (true, StandingRelation::Same | StandingRelation::Unknown, true) => {
            t!("status.state_mine_changed")
        }
        (true, StandingRelation::Unknown, false) => t!("status.state_mine_only"),
        (true, StandingRelation::Ahead, false) => {
            t!("status.state_mine_ahead", distance = distance)
        }
        (true, StandingRelation::Ahead, true) => {
            t!("status.state_mine_ahead_changed", distance = distance)
        }
        (true, StandingRelation::Behind, false) => {
            t!("status.state_mine_behind", distance = distance)
        }
        (true, StandingRelation::Behind, true) => {
            t!("status.state_mine_behind_changed", distance = distance)
        }
        (true, StandingRelation::Apart, false) => t!("status.state_mine_apart"),
        (true, StandingRelation::Apart, true) => t!("status.state_mine_apart_changed"),
        (false, StandingRelation::Same, false) => {
            t!("status.state_held_same", holder = holder)
        }
        (false, StandingRelation::Same | StandingRelation::Unknown, true) => {
            t!("status.state_held_changed", holder = holder)
        }
        (false, StandingRelation::Unknown, false) => {
            t!("status.state_held_only", holder = holder)
        }
        (false, StandingRelation::Ahead, false) => {
            t!(
                "status.state_held_ahead",
                holder = holder,
                distance = distance
            )
        }
        (false, StandingRelation::Ahead, true) => t!(
            "status.state_held_ahead_changed",
            holder = holder,
            distance = distance
        ),
        (false, StandingRelation::Behind, false) => {
            t!(
                "status.state_held_behind",
                holder = holder,
                distance = distance
            )
        }
        (false, StandingRelation::Behind, true) => t!(
            "status.state_held_behind_changed",
            holder = holder,
            distance = distance
        ),
        (false, StandingRelation::Apart, false) => t!("status.state_held_apart", holder = holder),
        (false, StandingRelation::Apart, true) => {
            t!("status.state_held_apart_changed", holder = holder)
        }
    };

    said.trim().to_owned()
}

/// Who holds the file, as the clause the state line begins with.
fn holder_words(standing: &Standing) -> String {
    standing.holder.as_ref().map_or_else(
        || t!("status.holder_nobody").trim().to_owned(),
        |holder| t!("status.holder_held", holder = holder).trim().to_owned(),
    )
}

/// Whether the state is one the two sides should not be in.
///
/// Holding something the Vault is ahead on, or that has diverged from it, is not how it should be;
/// neither is not holding something this side is ahead on, apart on, or has changed.
fn anomalous(standing: &Standing) -> bool {
    if standing.mine {
        return matches!(
            standing.relation,
            StandingRelation::Behind | StandingRelation::Apart
        );
    }

    standing.modified
        || matches!(
            standing.relation,
            StandingRelation::Ahead | StandingRelation::Apart
        )
}

/// One row as the line that says the Vault records the version it draws.
///
/// The whole line is drawn as one thing — the number, the Vault's name and the version's words —
/// since what it says is where the Vault stands rather than what any part of it is.
fn marked(row: &[Cell], vault: &str, note: Option<&String>) -> String {
    let text = plain(row);
    let said = note.map_or_else(
        || format!("{text} ({vault})"),
        |note| format!("{text} ({vault}) {note}"),
    );

    said.bold().bright_cyan().to_string()
}

/// One row of a drawing as its characters alone, with nothing said about colour.
fn plain(row: &[Cell]) -> String {
    let mut line = String::new();

    for cell in row {
        match cell {
            Cell::Blank => line.push(' '),
            Cell::Glyph(_, glyph) => line.push(*glyph),
        }
    }

    line.trim_end().to_owned()
}

/// One row of a drawing, its colours and nothing else.
fn drawn(row: &[Cell]) -> String {
    let mut line = String::new();

    for cell in row {
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

    line.trim_end().to_owned()
}

/// The words that go beside each version the drawing has, by the version's number.
///
/// A version's words are its variant's, and its variant is the one drawn at the level below it, so
/// a level's words stand beside the version one number up from it. The number the top placeholder
/// stands at has words of its own when the version there is being edited.
fn notes(result: &ResultVcsIndexLookback, lookback: &ResLookback) -> HashMap<u64, String> {
    let mut notes = HashMap::new();

    for level in &result.levels {
        let said = said(&level.variant, lookback);
        if !said.is_empty() {
            notes.insert(level.number.saturating_add(1), said);
        }
    }

    // The oldest version's words come from the variant based on the root, which is drawn at no
    // level: they belong beside version 0, which no level names.
    if let Some(first) = &result.first {
        let said = said(first, lookback);
        if !said.is_empty() {
            notes.insert(0, said);
        }
    }

    if result.top == TopKind::Editing && lookback.message() {
        notes.insert(
            recorded_top(result).saturating_add(1),
            t!("vcs_index_lookback.being_edited").trim().to_owned(),
        );
    }

    notes
}

/// The number the highest version the chain recorded carries.
///
/// One version is number 0, and every version over the oldest — a level of the drawing, or the head
/// a chain was started from — adds one. The levels alone do not count them, since the oldest
/// version's variant is based on the root and hangs from no level; a chain that recorded nothing is
/// number 0 all the same, which is where a placeholder stands when there is nothing under it.
fn recorded_top(result: &ResultVcsIndexLookback) -> u64 {
    u64::try_from(result.levels.len() + usize::from(result.first.is_some()))
        .unwrap_or(u64::MAX)
        .saturating_sub(1)
}

/// What is said beside one variant: who made it, what it says, or both.
fn said(variant: &VariantView, lookback: &ResLookback) -> String {
    let creator = lookback
        .creator()
        .then_some(variant.creator.as_str())
        .filter(|creator| !creator.is_empty());
    let message = lookback
        .message()
        .then(|| truncate(&variant.message, lookback.max_message_length()))
        .filter(|message| !message.is_empty());

    match (creator, message) {
        (Some(creator), Some(message)) => format!("{creator}: {message}"),
        (Some(creator), None) => creator.to_owned(),
        (None, Some(message)) => message,
        (None, None) => String::new(),
    }
}

/// `text` cut to `longest` characters, ending in an ellipsis when it was cut.
///
/// The ellipsis is part of what is shown, so a message of `longest` characters exactly is shown
/// whole and a longer one is cut to `longest - 3` and the three dots that say so. A `longest` too
/// small to hold the ellipsis cuts the text alone: what a run asked for is a length, and an
/// ellipsis that overran it would be saying something else.
fn truncate(text: &str, longest: usize) -> String {
    if text.chars().count() <= longest {
        return text.to_owned();
    }

    if longest <= ELLIPSIS.len() {
        return text.chars().take(longest).collect();
    }

    let kept: String = text.chars().take(longest - ELLIPSIS.len()).collect();

    format!("{kept}{ELLIPSIS}")
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

    // The number the highest recorded version carries: one version is number 0, and every version
    // over the oldest — a level of the drawing, or the head a chain was started from — adds one.
    // The levels alone do not count them, since the oldest version's variant is based on the root
    // and hangs from no level.
    let recorded = recorded_top(result);

    // The version a chain is looked back from a variant is one the index does not number, and one
    // being edited is a version it does not have at all: both are a level of their own over the
    // highest one drawn, so `V{n}` numbers what was recorded and the placeholder stands above it.
    let top = match result.top {
        TopKind::Editing => recorded.saturating_add(1),
        _ => recorded,
    };
    let top_kind = match result.top {
        TopKind::Version => Top::Numbered,
        TopKind::Unknown => Top::Unknown,
        TopKind::Editing => {
            // The variant the work is on its way to is not one the index has either, so the level
            // it will be made by is drawn as a placeholder of its own.
            nodes.push(Node::new(
                "??".to_owned(),
                top.saturating_sub(1),
                true,
                None,
            ));
            Top::Editing
        }
    };

    Chain::new(top, top_kind, nodes)
}

/// One node of the drawing: its label the leading characters of the variant's hash, its join the
/// node that hash names.
fn node_of(view: &VariantView, level: u64, spine: bool, at: &HashMap<&str, usize>) -> Node {
    let join = view.join.as_deref().and_then(|hash| at.get(hash).copied());

    Node::new(short(&view.hash).to_owned(), level, spine, join)
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn a_message_that_fits_is_shown_whole() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abc", 64), "abc");
    }

    #[test]
    fn what_is_cut_ends_in_an_ellipsis_that_fits_the_length() {
        assert_eq!(truncate("abcdefgh", 6), "abc...");
        assert_eq!(truncate("abcdefgh", 4), "a...");
    }

    #[test]
    fn a_length_too_small_for_an_ellipsis_cuts_the_text_alone() {
        assert_eq!(truncate("abcdefgh", 3), "abc");
        assert_eq!(truncate("abcdefgh", 0), "");
    }

    #[test]
    fn characters_are_what_is_counted() {
        assert_eq!(truncate("中文中文中文", 5), "中文...");
    }
}
