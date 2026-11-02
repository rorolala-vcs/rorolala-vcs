//! The `rola inv-idx rebuild` command: bring the inverse index up to date with the objects.
//!
//! A rebuild reads the objects the records do not describe and writes what it found, so afterwards
//! the records describe exactly the index they were built from; what it read is what the index had
//! and the records did not, or every object when the records cannot be added to. It prints nothing
//! of its own: what it produced is a file, and what it is worth reading is the report a `--json`
//! run is answered with.

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

/// Brings the inverse index up to date with the objects the index holds
///
/// The records are added to: only the objects written since the last build are read, and a run
/// whose records already describe the index reads nothing. What cannot be added to — the file is
/// not there, does not read, or describes objects the index no longer holds — is built afresh from
/// every object.
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
            mode: report.mode.name().to_owned(),
            objects: report.objects,
            objects_read: report.objects_read,
            variants: report.variants,
            versions: report.versions,
            versions_numbered: report.versions_numbered,
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
    /// What the rebuild had to do: `fresh`, `incremental` or `full`.
    mode: String,
    /// How many objects the index holds, as the build left it.
    objects: u64,
    /// How many objects this rebuild read.
    objects_read: u64,
    /// How many of the objects read are variants.
    variants: u64,
    /// How many are versions.
    versions: u64,
    /// How many versions this rebuild numbered itself.
    versions_numbered: u64,
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
