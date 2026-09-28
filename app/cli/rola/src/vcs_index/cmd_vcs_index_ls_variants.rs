//! The `rola vcs-index ls-variants` command: list the variants an index holds.
//!
//! One line a variant: its hash, the base version's hash, and the joined variant's when it is a
//! merge. The base version's number is not worked out here — that costs a trace of the chain for
//! every variant — so it is left to [`vcs_index_read`](crate::vcs_index::cmd_vcs_index_read), which
//! works it out for one.

use librorolala::storage::Key;
use librorolala::vcs::{VCSIndexObject, VCSWrite as _, Variant, Version};
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        buffer, chain, command, help, metadata, r_eprintln, r_print, r_println, renderer, routeify,
    },
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::{err_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::format::ResFormat;
use crate::vcs_index::{ErrorVcsIndexNoIndex, ErrorVcsIndexRead, hex, runtime, variant_tail};

/// How a listing of the variants is drawn when no template is named.
///
/// One line a variant, which is what the command has always printed except that the colour is not
/// in it: a template says nothing about a terminal. `hash` is the variant's own hash, which the
/// index names it by and `--json` does not carry — the two are otherwise the same fields.
const DEFAULT_FORMAT: &str = "{{ variants.hash }} -> {{ variants.variant.base_version }} (store:{{ variants.variant.storage_hash }}){% if variants.variant.join %} (join:{{ variants.variant.join }}){% endif %}";

#[help(buffer)]
pub fn help_vcs_index_ls_variants(_: EntryVcsIndexLsVariants, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_ls_variants.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexLsVariants)]
pub fn desc_vcs_index_ls_variants() -> Description {
    t!("vcs_index_ls_variants.description").to_string().into()
}

/// Lists the variants the index holds
///
/// One per line: the variant's hash, the base version's hash, and the joined variant's hash when
/// it is a merge. The base version's number is not worked out — that costs a trace of the chain
/// for every variant — so it is not printed here; `rola vcs-index read` works it out for one.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is, and
/// [`ErrorVcsIndexRead`] when the index could not be read.
#[command(node = "vcs-index.ls-variants")]
pub fn vcs_index_ls_variants(format: &mut ResFormat) -> StateVcsIndexLsVariants {
    format.default_template(DEFAULT_FORMAT);
    StateVcsIndexLsVariants
}

/// The state a listing of the variants starts in.
#[derive(Grouped)]
pub struct StateVcsIndexLsVariants;

#[chain(routeify)]
pub fn handle_vcs_index_ls_variants(
    _state: StateVcsIndexLsVariants,
    index: &mut LazyRes<ResVCSIndex>,
    format: &mut ResFormat,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let objects = match runtime.block_on(index.read_objects()) {
        Ok(objects) => objects,
        Err(error) => {
            return ErrorVcsIndexRead {
                cause: error.reason(),
            }
            .into();
        }
    };

    let mut variants = Vec::new();
    let mut published = Vec::new();
    for (key, object) in objects {
        let VCSIndexObject::Variant(variant) = object else {
            continue;
        };

        let version = runtime
            .block_on(index.read(Key::new(*variant.base_version())))
            .ok()
            .and_then(|object| object.expect_version().ok());
        published.push(serde_json::json!({
            "hash": key.hex(),
            "variant": &variant,
            "version": &version,
        }));
        variants.push(ItemVariant { variant, version });
    }
    format.set("variants", published);

    ResultVcsIndexLsVariants { variants }.into()
}

/// One variant, with the version it is based on when it can be read.
#[derive(Serialize)]
pub struct ItemVariant {
    /// The variant.
    variant: Variant,
    /// The version it is based on, when one is stored.
    version: Option<Version>,
}

/// Result: the variants the index holds.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexLsVariants {
    /// The variants.
    variants: Vec<ItemVariant>,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_ls_variants(
    result: ResultVcsIndexLsVariants,
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
        for item in &result.variants {
            r_println!(
                "{} -> {}{}",
                item.variant.hash().hex(),
                hex(item.variant.base_version()),
                variant_tail(item.variant.storage_hash(), item.variant.join())
            );
        }
    }
}
