//! `rola-daemon check`: whether every registered namespace still reaches a Vault.
//!
//! What is checked is what a request needs: the entry names a directory, and that directory
//! carries a configuration of its own. A row that fails is reported as failing rather than
//! left out, and the run ends non-zero so a script can branch on the whole table.

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
    exit_codes::{EC_CHECK_FAILED, EC_ERR_FORMAT, EC_HELP},
};

/// How a check is drawn when no template is named.
///
/// The word in brackets is what a program reads, and the namespace and path are what a person
/// needs to act on it.
const DEFAULT_CHECK_FORMAT: &str = "{% if checks.ok %}[OK]{% else %}[FAIL]{% endif %} \
                                    {{ checks.namespace }} ({{ checks.path }})";

#[help(buffer)]
pub fn help_check(_: EntryCheck, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("check.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryCheck)]
pub fn desc_check() -> Description {
    t!("check.description").to_string().into()
}

/// Checks every namespace registered for this user.
///
/// Nothing is named: the table is the whole of what there is to check.
#[completion(EntryCheck)]
pub fn complete_check() -> Suggest {
    suggest!()
}

#[command(node = "check")]
pub fn check(format: &mut ResFormat) -> StateCheck {
    format.default_template(DEFAULT_CHECK_FORMAT);
    StateCheck
}

/// The state a check starts in.
#[derive(Grouped)]
pub struct StateCheck;

#[chain]
pub fn handle_check(_state: StateCheck, format: &mut ResFormat, ec: &mut ResExitCode) -> Next {
    let Some(path) = registry_path() else {
        return ErrorNoDataDir.into();
    };

    let namespaces = match Namespaces::read_from(&path) {
        Ok(namespaces) => namespaces,
        Err(error) => return ErrorRegistry::new(path, error.to_string()).into(),
    };

    let checks: Vec<CheckItem> = namespaces
        .iter()
        .map(|(namespace, path)| CheckItem {
            namespace: namespace.to_owned(),
            path: path.display().to_string(),
            ok: path.join(rorolala_vault::CONFIG_PATH).is_file(),
        })
        .collect();

    // The code is set here rather than where the rows are drawn, since `--json` draws them
    // through the framework and never reaches the renderer.
    if checks.iter().any(|check| !check.ok) {
        ec.exit_code = EC_CHECK_FAILED;
    }

    format.set(
        "checks",
        checks.iter().map(|item| serde_json::json!(item)).collect(),
    );

    ResultCheck { checks }.into()
}

/// One namespace, and whether it still reaches a Vault.
#[derive(Serialize)]
pub struct CheckItem {
    /// The namespace a request names.
    namespace: String,
    /// The directory the namespace points at.
    path: String,
    /// Whether that directory holds a Vault.
    ok: bool,
}

/// Result: every namespace was checked.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultCheck {
    /// The namespaces and what was found, in order.
    checks: Vec<CheckItem>,
}

#[renderer(buffer)]
pub fn render_result_check(result: ResultCheck, format: &ResFormat, ec: &mut ResExitCode) {
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
        for check in &result.checks {
            let word = if check.ok { "[OK]" } else { "[FAIL]" };
            r_println!("{word} {} ({})", check.namespace, check.path);
        }
    }
}
