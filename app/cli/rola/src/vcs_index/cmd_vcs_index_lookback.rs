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
}

impl Default for ResLookback {
    fn default() -> Self {
        Self {
            compact: false,
            message: true,
            creator: true,
            max_message_length: DEFAULT_MESSAGE_LENGTH,
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
}

/// The 7 hex characters a hash is drawn by in the graph.
fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

#[renderer(buffer)]
pub fn render_result_vcs_index_lookback(result: ResultVcsIndexLookback, lookback: &ResLookback) {
    let chain = chain_of(&result);
    let drawing = draw_chain(&chain);
    let notes = notes(&result, lookback);
    let mut level_of: HashMap<usize, u64> = drawing
        .versions()
        .iter()
        .map(|version| (version.row(), version.level()))
        .collect();

    for (index, row) in drawing.rows().iter().enumerate() {
        // `--compact` draws the versions alone: what is between them is what a reader who wants the
        // chain's shape already has from the numbers.
        let Some(level) = level_of.remove(&index) else {
            if lookback.compact() {
                continue;
            }

            r_println!("{}", drawn(row));
            continue;
        };

        // The version the Vault records is the one a run reading this is looking for: where the
        // Vault stands is what says whether the work has been told to it. It is drawn as a line of
        // its own colour, so it is found without reading every number.
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

    if result.top == TopKind::Editing && lookback.message() {
        let at = result
            .levels
            .iter()
            .map(|level| level.number)
            .max()
            .map_or(0, |highest| highest.saturating_add(2));

        notes.insert(at, t!("vcs_index_lookback.being_edited").trim().to_owned());
    }

    notes
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

    let highest = result.levels.iter().map(|level| level.number).max();

    // The version a chain is looked back from a variant is one the index does not number, and one
    // being edited is a version it does not have at all: both are a level of their own over the
    // highest one drawn, so `V{n}` numbers what was recorded and the placeholder stands above it.
    let top = match (result.top, highest) {
        (TopKind::Editing, Some(highest)) => highest.saturating_add(2),
        (_, Some(highest)) => highest.saturating_add(1),
        (_, None) => 0,
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
