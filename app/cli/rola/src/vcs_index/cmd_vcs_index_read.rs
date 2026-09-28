//! The `rola vcs-index read` command: read one index object.
//!
//! A Creator or a Message is printed as its text; a Variant the way `ls-variants` prints it, and a
//! Version the way `ls-versions` prints it. What the hash names decides which. What is held is told
//! as fields rather than as the line a reader is shown, so a `--json` run is answered with the
//! object itself and not with the text it would have been printed as.

use librorolala::storage::Key;
use librorolala::vcs::{VCSIndexObject, VCSWrite as _};
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_print, r_println, renderer,
        routeify,
    },
    metadata::Description,
    picker::EntryPicker,
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
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexHash, ErrorVcsIndexNoIndex, ErrorVcsIndexRead, parse_hash,
    runtime, variant_tail,
};

/// How one object is drawn when no template is named.
///
/// The one line the command has always printed, whichever kind the object turned out to be —
/// except that the colour is not in it: a template says nothing about a terminal. A hash is written
/// as `blake3:<hex>` in the data, so the name in front is trimmed off where a line has no room for
/// it, which is how the line was always written.
pub const DEFAULT_FORMAT: &str = r#"{% if object.kind == "creator" %}{{ object.text }}{% elif object.kind == "message" %}{{ object.text }}{% elif object.kind == "variant" %}{{ object.hash | replace("blake3:", "") }} -> {{ object.base_version | replace("blake3:", "") }} (store:{{ object.storage | replace("blake3:", "") }}){% if object.join %} (join:{{ object.join | replace("blake3:", "") }}){% endif %}{% else %}{% if object.number is none %}?{% else %}{{ object.number }}{% endif %}:{{ object.hash | replace("blake3:", "") }} -> {{ object.variant | replace("blake3:", "") }}{% endif %}"#;

#[help(buffer)]
pub fn help_vcs_index_read(_: EntryVcsIndexRead, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_read.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexRead)]
pub fn desc_vcs_index_read() -> Description {
    t!("vcs_index_read.description").to_string().into()
}

/// Reads one index object
///
/// A Creator or a Message is printed as its text. A Variant is printed the way `ls-variants`
/// prints it, and a Version the way `ls-versions` prints it.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexHash`] when the hash does not read, and [`ErrorVcsIndexRead`] when the object
/// could not be read.
#[command(node = "vcs-index.read", entry = EntryVcsIndexRead)]
pub fn vcs_index_read(args: EntryVcsIndexRead, format: &mut ResFormat) -> Next {
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

    format.default_template(DEFAULT_FORMAT);
    StateVcsIndexRead { hash }.into()
}

/// The state a read starts in: the hash it names.
#[derive(Grouped)]
pub struct StateVcsIndexRead {
    /// The hash of the object to read.
    hash: String,
}

#[chain(routeify)]
pub fn handle_vcs_index_read(
    state: StateVcsIndexRead,
    index: &mut LazyRes<ResVCSIndex>,
    format: &mut ResFormat,
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

    let object = match runtime.block_on(index.read(key)) {
        Ok(object) => object,
        Err(error) => {
            return ErrorVcsIndexRead {
                cause: error.reason(),
            }
            .into();
        }
    };

    // The number is the index's to work out, and only a Version has one: it costs a trace of the
    // chain, so it is asked for the one object rather than for every one a listing names.
    let number = match &object {
        VCSIndexObject::Version(version) => runtime.block_on(index.version_num(version)).ok(),
        _ => None,
    };

    let view = view_of(object, number);
    format.set(
        "object",
        vec![serde_json::to_value(&view).unwrap_or_default()],
    );

    ResultVcsIndexRead { object: view }.into()
}

/// One index object, as the command shows it, with the number a Version was traced to.
///
/// The number is passed in rather than read off the object because which index an object came from
/// decides it: one read here is traced here, and one read from the other end is traced there and
/// carried back in the Version it names. A Creator or a Message has no number, and what is passed
/// for it is not read.
pub fn view_of(object: VCSIndexObject, number: Option<u64>) -> ReadObject {
    match object {
        VCSIndexObject::Creator(creator) => ReadObject::Creator {
            text: creator.read_to_string().to_owned(),
        },
        VCSIndexObject::Message(message) => ReadObject::Message {
            text: message.read_to_string().to_owned(),
        },
        VCSIndexObject::Variant(variant) => ReadObject::Variant {
            hash: variant.hash(),
            base_version: Key::new(*variant.base_version()),
            storage: Key::new(*variant.storage_hash()),
            join: variant.join().map(|join| Key::new(*join)),
        },
        VCSIndexObject::Version(version) => ReadObject::Version {
            number,
            hash: version.hash(),
            variant: Key::new(*version.variant()),
        },
    }
}

/// One index object, as it was read: which kind it is, and what it holds.
///
/// The kind travels in the data rather than in the line printed, so what a `--json` run prints is
/// the object itself — the text of a Creator or a Message, the hashes a Variant carries, the number
/// and hashes of a Version — rather than the string a reader would have been shown.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ReadObject {
    /// A Creator: the name it is.
    Creator {
        /// The name, as text.
        text: String,
    },
    /// A Message: the text it is.
    Message {
        /// The text, as text.
        text: String,
    },
    /// A Variant: the version it is based on, the storage entry it points at, and the variant it
    /// merges in when it is a merge.
    Variant {
        /// The variant's hash.
        hash: Key,
        /// The hash of the version it is based on.
        base_version: Key,
        /// The hash of the storage entry it points at.
        storage: Key,
        /// The hash of the variant it merges in, when it is a merge.
        join: Option<Key>,
    },
    /// A Version: its number, and the variant it points at.
    Version {
        /// The version's number, or nothing when the chain could not be traced.
        number: Option<u64>,
        /// The version's hash.
        hash: Key,
        /// The hash of the variant it points at.
        variant: Key,
    },
}

/// Result: what the object was.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexRead {
    /// The object, by the kind it turned out to be.
    pub object: ReadObject,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_read(
    result: ResultVcsIndexRead,
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
        r_println!("{}", line_of(&result.object));
    }
}

/// The object as the one line a listing prints it on.
pub fn line_of(object: &ReadObject) -> String {
    match object {
        ReadObject::Creator { text } | ReadObject::Message { text } => text.clone(),
        ReadObject::Variant {
            hash,
            base_version,
            storage,
            join,
        } => format!(
            "{} -> {}{}",
            hash.hex(),
            base_version.hex(),
            variant_tail(storage.digest(), join.as_ref().map(Key::digest)),
        ),
        ReadObject::Version {
            number,
            hash,
            variant,
        } => format!(
            "{}:{} -> {}",
            number.map_or_else(|| "?".to_owned(), |number| number.to_string()),
            hash.hex(),
            variant.hex(),
        ),
    }
}
