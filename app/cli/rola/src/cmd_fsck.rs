//! The `rola fsck` command: whether the content store still holds what it names.
//!
//! It checks the store a run works on and nothing else: the Layout, the version control index and
//! the inverse index are other crates' to answer for. What it reports is what it found, and it
//! changes nothing: a pack, a manifest or an object that is wrong is left where it is, and
//! `pack --garbage-collect` or `rebuild` is where repairing starts.

use librorolala::storage::{FsckOptions, FsckReport};
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::ResStorageSet;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_FSCK, EC_ERR_FSCK_FAILED, EC_ERR_FSCK_NO_STORAGE, EC_HELP};
use crate::failure::failure;

/// Read every object and check it against its key, rather than only looking at the layout.
const ARG_FULL: PickerArg<'static, Flag> = arg![full: Flag];

#[help(buffer)]
pub fn help_fsck(_: EntryFsck, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fsck.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFsck)]
pub fn desc_fsck() -> Description {
    t!("fsck.cmd_fsck_description").to_string().into()
}

/// Completes what `rola fsck` can be given next.
///
/// The command names nothing, so the only thing to offer is the flag that makes the check read
/// content.
#[completion(EntryFsck)]
pub fn complete_fsck(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_FULL: t!("fsck.complete.full"),
            },
        );
    }

    suggest!()
}

/// Checks that every store the run can reach still holds what it names.
///
/// A structural check is what a run gets without asking for more: it names the packs whose index
/// does not read, the packs whose index reads but whose file is gone, the manifests that do not
/// read, and the contents whose manifests name chunks that are gone. No content is read, so an
/// object whose bytes changed is not found by it.
///
/// `--full` reads every key the store names and checks the content against its key, which is what
/// a read would do. It costs one read of everything the store holds.
///
/// The run has to be somewhere a store can be found, inside one, or inside a Vault or Workspace
/// that keeps one. Each store the run can read is checked on its own, and the findings are named
/// under the store they belong to.
///
/// Nothing is repaired. A store that does not hold together ends the run with exit code 246, and a
/// store that could not be checked at all ends it with 248, because an answer that is not known is
/// worse than one that is wrong.
///
/// # Errors
///
/// Renders [`ErrorFsckNoStorage`] when the run is nowhere a store is, and [`ErrorFsckFailed`] when
/// the runtime the check runs on cannot be made.
#[command(node = "fsck", entry = EntryFsck)]
pub fn fsck(args: EntryFsck) -> Next {
    // UNWRAP: picking flags cannot fail: a flag that is absent is `Inactive`, not an error.
    let (full, unmatched) = args.pick(ARG_FULL).pick(&UNMATCHED).unwrap();

    if let Some(next) = crate::unmatched::refused(&unmatched, "fsck") {
        return next;
    }

    StateFsck {
        full: matches!(full, Flag::Active),
    }
    .into()
}

/// The state a check starts in.
#[derive(Clone, Copy, Grouped)]
pub struct StateFsck {
    /// Whether every object is to be read and checked against its key.
    full: bool,
}

#[chain(routeify)]
pub fn handle_fsck(state: StateFsck, storage: &mut LazyRes<ResStorageSet>) -> Next {
    let Some(set) = storage.get_ref().storage() else {
        return ErrorFsckNoStorage.into();
    };

    // The store is asynchronous and a command is not, so the two meet here, see
    // `cmd_storage_ls_storaged`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorFsckFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let options = FsckOptions { full: state.full };

    let stores = runtime.block_on(async {
        let mut stores = Vec::new();

        for store in set.stores() {
            let root = store.get_root().to_string_lossy().into_owned();

            stores.push(match store.fsck(options).await {
                Ok(report) => StoreFsck {
                    root,
                    report,
                    unreadable: None,
                },
                // One store that cannot be read does not stop the others: what this run knows so
                // far is still worth saying, and the store that failed is named as unanswered.
                Err(error) => StoreFsck {
                    root,
                    report: FsckReport::default(),
                    unreadable: Some(error.to_string()),
                },
            });
        }

        stores
    });

    ResultFsck { stores }.into()
}

/// Result: every store the run can reach was checked.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultFsck {
    /// One check per store, in the order the run reads them.
    pub(crate) stores: Vec<StoreFsck>,
}

/// What was found in one store, and whether the check itself could be carried out.
#[derive(Serialize)]
pub struct StoreFsck {
    /// The root of the store, as the configuration names it.
    pub(crate) root: String,
    /// The findings, by what they are about.
    #[serde(flatten)]
    pub(crate) report: FsckReport,
    /// Why the store could not be checked at all, when it could not.
    pub(crate) unreadable: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_fsck(result: ResultFsck, ec: &mut ResExitCode) {
    // The root path is said once per store only when there is more than one: a run inside a single
    // store that began every line with the path it stands in would be repeating itself.
    let many = result.stores.len() > 1;
    let mut problems = 0;
    let mut failed = false;

    for store in &result.stores {
        if many {
            r_println!("{}", store.root);
        }

        if let Some(reason) = &store.unreadable {
            failed = true;
            r_eprintln!(
                "{}",
                err_line!(
                    t!(
                        "fsck.result_store_unreadable",
                        root = store.root,
                        reason = reason
                    )
                    .trim()
                )
            );
            continue;
        }

        for index in &store.report.broken_packs {
            r_println!("{}", t!("fsck.result_broken_pack", index = index).trim());
        }
        for index in &store.report.missing_packs {
            r_println!("{}", t!("fsck.result_missing_pack", index = index).trim());
        }
        for key in &store.report.broken_manifests {
            r_println!("{}", t!("fsck.result_broken_manifest", key = key).trim());
        }
        for missing in &store.report.missing_chunks {
            for chunk in &missing.chunks {
                r_println!(
                    "{}",
                    t!(
                        "fsck.result_missing_chunk",
                        key = missing.manifest,
                        chunk = chunk
                    )
                    .trim()
                );
            }
        }
        for key in &store.report.missing {
            r_println!("{}", t!("fsck.result_missing_object", key = key).trim());
        }
        for key in &store.report.corrupt {
            r_println!("{}", t!("fsck.result_corrupt_object", key = key).trim());
        }
        for key in &store.report.malformed {
            r_println!("{}", t!("fsck.result_malformed_object", key = key).trim());
        }

        problems += store.report.problems();

        r_println!(
            "{}",
            t!(
                "fsck.result_checked",
                packs = store.report.checked_packs,
                manifests = store.report.checked_manifests,
                objects = store.report.checked_objects
            )
            .trim()
        );

        if store.report.is_clean() {
            r_println!("{}", t!("fsck.result_clean").trim());
        }
    }

    if problems > 0 {
        r_println!("{}", t!("fsck.result_found", count = problems).trim());
    }

    // A store that could not be answered for is what a caller has to branch on first: the findings
    // it did hear are true, but the run cannot promise there are no others.
    ec.exit_code = if failed {
        EC_ERR_FSCK_FAILED
    } else if problems > 0 {
        EC_ERR_FSCK
    } else {
        ec.exit_code
    };
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorFsckNoStorage;

impl Failure for ErrorFsckNoStorage {
    fn name(&self) -> &'static str {
        "error_fsck_no_storage"
    }

    fn reason(&self) -> String {
        t!("fsck.err_no_storage").trim().to_string()
    }
}

failure!(ErrorFsckNoStorage);

#[renderer(buffer)]
pub fn render_error_fsck_no_storage(error: ErrorFsckNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("fsck.err_no_storage_help").trim()));
    ec.exit_code = EC_ERR_FSCK_NO_STORAGE;
}

/// Error: the check could not be started at all.
#[derive(Grouped)]
pub struct ErrorFsckFailed {
    /// Why the run could not be made.
    cause: String,
}

impl Failure for ErrorFsckFailed {
    fn name(&self) -> &'static str {
        "error_fsck_failed"
    }

    fn reason(&self) -> String {
        t!("fsck.err_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorFsckFailed);

#[renderer(buffer)]
pub fn render_error_fsck_failed(error: ErrorFsckFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("fsck.err_failed_help").trim()));
    ec.exit_code = EC_ERR_FSCK_FAILED;
}
