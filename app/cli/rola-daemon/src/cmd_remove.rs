//! `rola-daemon remove`: stop serving a namespace.
//!
//! What is removed is the row, not the Vault: the directory stays where it is, and registering
//! it again is all it takes to have it served once more.

use std::path::PathBuf;

use mingling::{
    Grouped, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_daemon::{NamespaceError, Namespaces, registry_path};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::{
    Next,
    errors::{ErrorArgument, ErrorNoDataDir, ErrorNotRegistered, ErrorRegistry},
    exit_codes::EC_HELP,
};

#[help(buffer)]
pub fn help_remove(_: EntryRemove, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("remove.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryRemove)]
pub fn desc_remove() -> Description {
    t!("remove.description").to_string().into()
}

/// Completes what `rola-daemon remove` can be given next.
///
/// A namespace is one the run can only know from the table, so the names registered are offered.
#[completion(EntryRemove)]
pub fn complete_remove(ctx: ShellContext) -> Suggest {
    if ctx.current_word.starts_with('-') {
        return suggest!();
    }

    let Some(path) = registry_path() else {
        return suggest!();
    };
    let Ok(namespaces) = Namespaces::read_from(&path) else {
        return suggest!();
    };

    namespaces
        .iter()
        .map(|(namespace, _)| namespace.to_owned())
        .filter(|namespace| namespace.starts_with(&ctx.current_word))
        .collect::<Vec<_>>()
        .into()
}

#[command(node = "remove")]
pub fn remove(args: EntryRemove) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || ErrorArgument::missing("NAMESPACE").into())
        .to_result();

    let namespace = match picked {
        Ok(namespace) => namespace,
        Err(next) => return next,
    };

    StateRemove { namespace }.into()
}

/// The state of removing a namespace.
#[derive(Grouped)]
pub struct StateRemove {
    /// The name that is to stop being served.
    namespace: String,
}

#[chain]
pub fn handle_remove(state: StateRemove) -> Next {
    let StateRemove { namespace } = state;

    let Some(registry) = registry_path() else {
        return ErrorNoDataDir.into();
    };

    let mut namespaces = match Namespaces::read_from(&registry) {
        Ok(namespaces) => namespaces,
        Err(error) => return ErrorRegistry::new(registry, error.to_string()).into(),
    };

    let path = match namespaces.remove(&namespace) {
        Ok(path) => path,
        Err(NamespaceError::NotFound(name)) => return ErrorNotRegistered::new(name).into(),
        Err(error) => return ErrorRegistry::new(registry, error.to_string()).into(),
    };

    if let Err(error) = namespaces.write_to(&registry) {
        return ErrorRegistry::new(registry, error.to_string()).into();
    }

    ResultRemoved { namespace, path }.into()
}

/// Result: a namespace stopped being served.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultRemoved {
    /// The namespace that was removed.
    namespace: String,
    /// The directory it pointed at, which is left where it was.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_removed(result: ResultRemoved) {
    r_println!(
        "{}",
        t!(
            "remove.result",
            namespace = result.namespace,
            path = result.path.display().to_string()
        )
        .trim()
    );
}
