//! The `rola vcs-index ls-versions` command: list the versions an index holds.
//!
//! One line a version: its number and hash, and the hash of the variant it points at. The number is
//! worked out by tracing the chain, one version at a time.

use librorolala::storage::Key;
use librorolala::vcs::{VCSIndexObject, VCSWrite as _, Variant, Version};
use mingling::{
    Grouped, LazyRes, StructuralData, Suggest,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, routeify, suggest,
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
use crate::vcs_index::{ErrorVcsIndexNoIndex, ErrorVcsIndexRead, hex, runtime};

/// How a listing of the versions is drawn when no template is named.
///
/// One line a version, which is what the command has always printed. `hash` is the version's own
/// hash, which the index names it by and `--json` does not carry; `?` is where the chain could not
/// be traced and the number is not known.
const DEFAULT_FORMAT: &str = "{% if versions.version_num is none %}?{% else %}{{ versions.version_num }}{% endif %}:{{ versions.hash }} -> {{ versions.version.variant }}";

#[help(buffer)]
pub fn help_vcs_index_ls_versions(_: EntryVcsIndexLsVersions, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_ls_versions.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexLsVersions)]
pub fn desc_vcs_index_ls_versions() -> Description {
    t!("vcs_index_ls_versions.description").to_string().into()
}

/// Lists the versions the index holds
///
/// One per line: the version's number and hash, and the hash of the variant it points at. The
/// number is worked out by tracing the chain, one version at a time.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is, and
/// [`ErrorVcsIndexRead`] when the index could not be read.
#[command(node = "vcs-index.ls-versions")]
pub fn vcs_index_ls_versions(format: &mut ResFormat) -> StateVcsIndexLsVersions {
    format.default_template(DEFAULT_FORMAT);
    StateVcsIndexLsVersions
}

/// The state a listing of the versions starts in.
#[derive(Grouped)]
pub struct StateVcsIndexLsVersions;

#[chain(routeify)]
pub fn handle_vcs_index_ls_versions(
    _state: StateVcsIndexLsVersions,
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

    let mut versions = Vec::new();
    let mut published = Vec::new();
    for (key, object) in objects {
        let VCSIndexObject::Version(version) = object else {
            continue;
        };

        let number = runtime.block_on(index.version_num(&version)).ok();
        let variant = runtime
            .block_on(index.read(Key::new(*version.variant())))
            .ok()
            .and_then(|object| object.expect_variant().ok());
        published.push(serde_json::json!({
            "hash": key.hex(),
            "version_num": number,
            "version": &version,
            "variant": &variant,
        }));
        versions.push(ItemVersion {
            version_num: number,
            version,
            variant,
        });
    }
    format.set("versions", published);

    ResultVcsIndexLsVersions { versions }.into()
}

/// One version, its number, and the variant it points at when it can be read.
#[derive(Serialize)]
pub struct ItemVersion {
    /// The number worked out for the version, or nothing when the chain could not be traced.
    version_num: Option<u64>,
    /// The version.
    version: Version,
    /// The variant it points at, when one is stored.
    variant: Option<Variant>,
}

/// Result: the versions the index holds.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexLsVersions {
    /// The versions.
    versions: Vec<ItemVersion>,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_ls_versions(
    result: ResultVcsIndexLsVersions,
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
        for item in &result.versions {
            r_println!(
                "{}:{} -> {}",
                item.version_num
                    .map_or_else(|| "?".to_owned(), |num| num.to_string()),
                item.version.hash().hex(),
                hex(item.version.variant())
            );
        }
    }
}

/// Completes what `rola vcs-index ls-versions` can be given next.
///
/// The listing names nothing, so there is nothing to offer.
#[completion(EntryVcsIndexLsVersions)]
pub fn complete_vcs_index_ls_versions() -> Suggest {
    suggest!()
}
