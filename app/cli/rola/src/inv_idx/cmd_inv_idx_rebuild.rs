//! The `rola inv-idx rebuild` command: build the inverse index from the objects.
//!
//! A rebuild reads every object the index holds and writes what it found, so afterwards the records
//! describe exactly the index they were built from. It prints nothing of its own: what it produced
//! is a file, and what it is worth reading is the report a `--json` run is answered with.

use librorolala::inverse_index::InverseIndex;
use mingling::{
    Grouped, LazyRes, StructuralData, Suggest,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, renderer, routeify, suggest,
    },
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::inv_idx::{ErrorInvIdxNoIndex, ErrorInvIdxRead, runtime};

#[help(buffer)]
pub fn help_inv_idx_rebuild(_: EntryInvIdxRebuild, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx_rebuild.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInvIdxRebuild)]
pub fn desc_inv_idx_rebuild() -> Description {
    t!("inv_idx_rebuild.description").to_string().into()
}

/// Builds the inverse index from the objects the index holds
///
/// Every object is read once and what each points at is written down, so the records afterwards
/// describe exactly the index they were built from — a reader that arrives after another object is
/// written sees records that no longer describe it and falls back to the objects.
///
/// The run has to be somewhere an index can be found — inside a Vault or a Workspace.
///
/// # Errors
///
/// Renders [`ErrorInvIdxNoIndex`] when the run is nowhere an index is, and [`ErrorInvIdxRead`] when
/// the index could not be read or the inverse index could not be written.
#[command(node = "inv-idx.rebuild")]
pub fn inv_idx_rebuild() -> StateInvIdxRebuild {
    StateInvIdxRebuild
}

/// The state a rebuild starts in.
#[derive(Grouped)]
pub struct StateInvIdxRebuild;

#[chain(routeify)]
pub fn handle_inv_idx_rebuild(
    _state: StateInvIdxRebuild,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorInvIdxNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let inverse = InverseIndex::at(index.clone());
    match runtime.block_on(inverse.rebuild()) {
        Ok(report) => ResultInvIdxRebuild {
            objects: report.objects,
            variants: report.variants,
            versions: report.versions,
            creators: report.creators,
            messages: report.messages,
            entries: report.entries,
            dependents: report.dependents,
            fingerprint: report.fingerprint,
        }
        .into(),
        Err(error) => ErrorInvIdxRead {
            cause: error.reason(),
        }
        .into(),
    }
}

/// Result: what the rebuild found and wrote.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultInvIdxRebuild {
    /// How many objects the index holds.
    objects: u64,
    /// How many of them are variants.
    variants: u64,
    /// How many are versions.
    versions: u64,
    /// How many are creators.
    creators: u64,
    /// How many are messages.
    messages: u64,
    /// How many records the inverse index has.
    entries: u64,
    /// How many dependents are noted across the records.
    dependents: u64,
    /// The digest of what was written, as hex.
    fingerprint: String,
}

#[renderer(buffer)]
pub fn render_result_inv_idx_rebuild(_result: ResultInvIdxRebuild) {
    // A rebuild produces a file, not a line: nothing is said on stdout, and a `--json` run is
    // answered with the report through the structured renderer.
}

/// Completes what `rola inv-idx rebuild` can be given next.
///
/// The command names nothing, so there is nothing to offer.
#[completion(EntryInvIdxRebuild)]
pub fn complete_inv_idx_rebuild() -> Suggest {
    suggest!()
}
