//! The `rola inv-idx list` command: list the variants and versions the records name.
//!
//! This is the listing of the inverse index read the other way: a variant a row with what it is made
//! of, a version a row with the variant it fixes and its number, and the rows drawn through the
//! template `--format` names or written as `--json` objects. It is read-only: records that do not
//! describe the index are not rebuilt by a listing, the objects are read instead, which is the same
//! answer at the price of a read per object.
//!
//! The rows are the variants first and then the versions, each in key order, since that is the order
//! the records hold them in; `--kind` keeps one kind and `--limit` stops the listing short.

use librorolala::inverse_index::{InverseIndex, Snapshot};
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg},
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::{err_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::complete::{filling_flag, offer, strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::format::ResFormat;
use crate::inv_idx::{ErrorInvIdxKind, ErrorInvIdxNoIndex, ErrorInvIdxRead, runtime};

/// How a listing of the records is drawn when no template is named.
///
/// One row a line, and a row is its kind and its key: what `--json` writes under `objects`.
pub const DEFAULT_FORMAT_LIST: &str = "{{ objects.kind }} {{ objects.hash }}";

/// Which rows to list: `variant`, `version` or `all`.
const ARG_KIND: PickerArg<'static, Option<String>> = arg![kind: Option<String>];

/// How many rows to list at most.
const ARG_LIMIT: PickerArg<'static, Option<usize>> = arg![limit: Option<usize>];

#[help(buffer)]
pub fn help_inv_idx_list(_: EntryInvIdxList, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx_list.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInvIdxList)]
pub fn desc_inv_idx_list() -> Description {
    t!("inv_idx_list.description").to_string().into()
}

/// Completes what `rola inv-idx list` can be given next.
///
/// A row's kind is one of three words, so `--kind` offers them; there is nothing else the listing
/// names, and a number has no list to offer.
#[completion(EntryInvIdxList)]
pub fn complete_inv_idx_list(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_KIND: t!("inv_idx_list.complete.kind"),
                ARG_LIMIT: t!("inv_idx_list.complete.limit"),
            },
        );
    }

    if filling_flag(&ctx, &ARG_KIND) {
        return offer(&ctx, KIND_WORDS);
    }

    suggest!()
}

/// Lists the variants and versions the inverse index holds
///
/// A variant is a row with its key and what it is made of: the version it is based on, the variant
/// that version fixes, the variant it merges in, and the content, the creator and the message it
/// names. A version is a row with its key, the variant it fixes, its number and whether it is the
/// root; the root is numbered before the first, so what is drawn for it is `is_root` and no number.
///
/// The rows are the variants and then the versions, each in key order. `--kind` keeps one kind, and
/// `--limit` stops after that many rows.
///
/// The records are read when they describe the index; when they do not, an object was written and
/// no rebuild has run since, the objects are read instead, so the run stays read-only and is still
/// answered with what a rebuild would make the records say.
///
/// # Errors
///
/// Renders [`ErrorInvIdxNoIndex`] when the run is nowhere an index is, [`ErrorInvIdxKind`] when
/// `--kind` names no kind this command lists, and [`ErrorInvIdxRead`] when the records or the
/// objects could not be read.
#[command(node = "inv-idx.list", entry = EntryInvIdxList)]
pub fn inv_idx_list(args: EntryInvIdxList, format: &mut ResFormat) -> Next {
    // UNWRAP: Picking cannot fail: a flag that is absent is `None`, and the words no argument takes
    // are collected as unmatched rather than refused by the pick.
    let (kind, limit, unmatched) = args
        .pick(ARG_KIND)
        .pick(ARG_LIMIT)
        .pick(&UNMATCHED)
        .unwrap();

    if let Some(next) = crate::unmatched::refused(&unmatched, "inv-idx.list") {
        return next;
    }

    let kinds = match kind.as_deref() {
        None => Kinds::All,
        Some(text) => match Kinds::parse(text) {
            Some(kinds) => kinds,
            None => {
                return ErrorInvIdxKind {
                    kind: text.to_owned(),
                }
                .into();
            }
        },
    };

    format.default_template(DEFAULT_FORMAT_LIST);

    StateInvIdxList { kinds, limit }.into()
}

/// The state a listing starts in: which rows, and how many.
#[derive(Grouped, Clone, Copy)]
pub struct StateInvIdxList {
    /// Which kinds of row to list.
    kinds: Kinds,
    /// How many rows to list at most.
    limit: Option<usize>,
}

#[chain(routeify)]
pub fn handle_inv_idx_list(
    state: StateInvIdxList,
    index: &mut LazyRes<ResVCSIndex>,
    format: &mut ResFormat,
) -> Next {
    let StateInvIdxList { kinds, limit } = state;
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorInvIdxNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let inverse = InverseIndex::at(index.clone());
    let snapshot = match runtime.block_on(inverse.snapshot()) {
        Ok(Some(snapshot)) => snapshot,
        // The records do not describe the index, and a listing may not write them: the answer is
        // read off the objects instead, which is what a rebuild would make the records say.
        Ok(None) => match runtime.block_on(inverse.scan()) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return ErrorInvIdxRead {
                    cause: error.reason(),
                }
                .into();
            }
        },
        Err(error) => {
            return ErrorInvIdxRead {
                cause: error.reason(),
            }
            .into();
        }
    };

    let mut objects = rows(&snapshot, kinds);
    if let Some(limit) = limit {
        objects.truncate(limit);
    }

    // UNWRAP: a row is plain fields and hex keys, which serialize to JSON without failing.
    let drawn: Vec<serde_json::Value> = objects
        .iter()
        .map(|row| serde_json::to_value(row).expect("a row of plain fields is JSON"))
        .collect();
    format.set("objects", drawn);

    ResultInvIdxList { objects }.into()
}

/// Result: the variants and versions the records name.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultInvIdxList {
    /// The rows, a variant or a version each.
    objects: Vec<InvIdxRow>,
}

/// One row of a listing: a variant or a version, and what it names.
///
/// A field that belongs to the other kind is `null`, so `--json` writes the same names for every row
/// and a template can draw any of them without knowing which kind a row is.
#[derive(Debug, Clone, Serialize)]
pub struct InvIdxRow {
    /// The kind of object: `variant` or `version`.
    kind: &'static str,
    /// The object's own key, as hex.
    hash: String,
    /// The variant a version fixes.
    variant: Option<String>,
    /// A version's number in its chain, when it has one.
    number: Option<u64>,
    /// Whether a version is the root, the one numbered before the first.
    is_root: bool,
    /// The version a variant is based on.
    base_version: Option<String>,
    /// The variant that version fixes, when the index holds it.
    base_variant: Option<String>,
    /// The variant a merge merges in.
    join: Option<String>,
    /// The content a variant was made of.
    storage: Option<String>,
    /// Who made a variant.
    creator: Option<String>,
    /// What a variant says was done.
    message: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_inv_idx_list(
    result: ResultInvIdxList,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!(
                    "{}",
                    err_line!(t!("format.err_format", reason = error).trim())
                );
                ec.exit_code = EC_ERR_FORMAT;
            }
        }
    } else {
        for row in &result.objects {
            r_println!("{} {}", row.kind, row.hash);
        }
    }
}

/// The rows `snapshot` gives, in the kinds asked for.
fn rows(snapshot: &Snapshot, kinds: Kinds) -> Vec<InvIdxRow> {
    let mut rows = Vec::new();

    if kinds != Kinds::Version {
        for (hash, record) in snapshot.variants() {
            let base_variant = snapshot.version(record.base_version).and_then(|base| {
                (!base.is_root && snapshot.variant(base.variant).is_some()).then_some(base.variant)
            });

            rows.push(InvIdxRow {
                kind: "variant",
                hash: hash.hex(),
                variant: None,
                number: None,
                is_root: false,
                base_version: Some(record.base_version.hex()),
                base_variant: base_variant.map(|hash| hash.hex()),
                join: record.join.map(|hash| hash.hex()),
                storage: Some(record.storage.hex()),
                creator: Some(record.creator.hex()),
                message: Some(record.message.hex()),
            });
        }
    }

    if kinds != Kinds::Variant {
        for (hash, record) in snapshot.versions() {
            rows.push(InvIdxRow {
                kind: "version",
                hash: hash.hex(),
                variant: Some(record.variant.hex()),
                number: record.number,
                is_root: record.is_root,
                base_version: None,
                base_variant: None,
                join: None,
                storage: None,
                creator: None,
                message: None,
            });
        }
    }

    rows
}

/// Which rows a listing holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kinds {
    /// Every row.
    All,
    /// The variants only.
    Variant,
    /// The versions only.
    Version,
}

/// The words `--kind` takes.
const KIND_WORDS: [&str; 3] = ["all", "variant", "version"];

impl Kinds {
    /// The kinds `text` names, when it names one this command lists.
    fn parse(text: &str) -> Option<Self> {
        match text {
            "all" => Some(Self::All),
            "variant" => Some(Self::Variant),
            "version" => Some(Self::Version),
            _ => None,
        }
    }
}
