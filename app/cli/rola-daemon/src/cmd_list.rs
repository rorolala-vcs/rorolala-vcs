//! `rola-daemon list`: the namespaces this machine serves.
//!
//! A listing is the table's own, in namespace order, and changes nothing. It is read through
//! the same [`Namespaces`] a request is, so what is listed is what the daemon would answer for.

use mingling::{
    Grouped, StructuralData, Suggest,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, suggest,
    },
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResFormat;
use rorolala_daemon::{Namespaces, registry_path};
use rorolala_utils_cli_theme::{err_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::{
    Next,
    errors::{ErrorNoDataDir, ErrorRegistry},
    exit_codes::{EC_ERR_FORMAT, EC_HELP},
};

/// How a listing is drawn when no template is named.
///
/// One namespace a line. The path is in the data a template or `--json` reads, so a reader that
/// wants to see where a namespace points writes `{{ namespaces.path }}`.
const DEFAULT_LIST_FORMAT: &str = "{{ namespaces.namespace }}";

#[help(buffer)]
pub fn help_list(_: EntryList, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("list.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryList)]
pub fn desc_list() -> Description {
    t!("list.description").to_string().into()
}

/// Lists the namespaces registered for this user.
///
/// Nothing is named, so there is nothing this run can only learn at the end: what it lists is
/// read from the table as the command is reached.
#[completion(EntryList)]
pub fn complete_list() -> Suggest {
    suggest!()
}

#[command(node = "list")]
pub fn list(format: &mut ResFormat) -> StateList {
    format.default_template(DEFAULT_LIST_FORMAT);
    StateList
}

/// The state a listing starts in.
#[derive(Grouped)]
pub struct StateList;

#[chain]
pub fn handle_list(_state: StateList, format: &mut ResFormat) -> Next {
    let Some(path) = registry_path() else {
        return ErrorNoDataDir.into();
    };

    let namespaces = match Namespaces::read_from(&path) {
        Ok(namespaces) => namespaces,
        Err(error) => return ErrorRegistry::new(path, error.to_string()).into(),
    };

    let namespaces: Vec<NamespaceItem> = namespaces
        .iter()
        .map(|(namespace, path)| NamespaceItem {
            namespace: namespace.to_owned(),
            path: path.display().to_string(),
        })
        .collect();

    format.set(
        "namespaces",
        namespaces
            .iter()
            .map(|item| serde_json::json!(item))
            .collect(),
    );

    ResultList { namespaces }.into()
}

/// One namespace, as `list` shows it.
#[derive(Serialize)]
pub struct NamespaceItem {
    /// The namespace a request names.
    namespace: String,
    /// The directory the namespace points at.
    path: String,
}

/// Result: the namespaces were listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultList {
    /// The namespaces, in order.
    namespaces: Vec<NamespaceItem>,
}

#[renderer(buffer)]
pub fn render_result_list(result: ResultList, format: &ResFormat, ec: &mut ResExitCode) {
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
        for namespace in &result.namespaces {
            r_println!("{}", namespace.namespace);
        }
    }
}
