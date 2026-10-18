//! `rola-daemon register`: name a Vault this machine's daemon serves.
//!
//! The name and the path are read here and checked before anything is written: a namespace is
//! one segment of an address, and the path has to be a Vault, so a row that could not be served
//! never reaches the table.

use std::path::PathBuf;

use mingling::{
    Grouped, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_daemon::{NamespaceError, Namespaces, is_namespace, registry_path};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::{
    Next,
    errors::{
        ErrorAlreadyRegistered, ErrorArgument, ErrorNamespaceInvalid, ErrorNoDataDir,
        ErrorNotAVault, ErrorRegistry,
    },
    exit_codes::EC_HELP,
};

#[help(buffer)]
pub fn help_register(_: EntryRegister, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("register.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryRegister)]
pub fn desc_register() -> Description {
    t!("register.description").to_string().into()
}

/// Completes what `rola-daemon register` can be given next.
///
/// A name is the caller's to choose and a path is the filesystem's to answer, so nothing is
/// offered here.
#[completion(EntryRegister)]
pub fn complete_register() -> Suggest {
    suggest!()
}

#[command(node = "register")]
pub fn register(args: EntryRegister) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || ErrorArgument::missing("NAMESPACE").into())
        .pick_or_route(&arg![String], || ErrorArgument::missing("PATH").into())
        .to_result();

    let (namespace, path) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateRegister { namespace, path }.into()
}

/// The state of registering a namespace.
#[derive(Grouped)]
pub struct StateRegister {
    /// The name the Vault is reached under.
    namespace: String,
    /// The directory it sits at, as it was written.
    path: String,
}

#[chain]
pub fn handle_register(state: StateRegister) -> Next {
    let StateRegister { namespace, path } = state;

    if !is_namespace(&namespace) {
        return ErrorNamespaceInvalid::new(namespace).into();
    }

    let path = match std::path::absolute(&path) {
        Ok(path) => path,
        Err(error) => return ErrorNotAVault::new(format!("{path}: {error}")).into(),
    };

    if !path.join(rorolala_vault::CONFIG_PATH).is_file() {
        return ErrorNotAVault::new(path.display().to_string()).into();
    }

    let Some(registry) = registry_path() else {
        return ErrorNoDataDir.into();
    };

    let mut namespaces = match Namespaces::read_from(&registry) {
        Ok(namespaces) => namespaces,
        Err(error) => return ErrorRegistry::new(registry, error.to_string()).into(),
    };

    match namespaces.register(&namespace, &path) {
        Ok(()) => {}
        Err(NamespaceError::InvalidNamespace(name)) => {
            return ErrorNamespaceInvalid::new(name).into();
        }
        Err(NamespaceError::AlreadyRegistered(name)) => {
            return ErrorAlreadyRegistered::new(name).into();
        }
        Err(error) => return ErrorRegistry::new(registry, error.to_string()).into(),
    }

    if let Err(error) = namespaces.write_to(&registry) {
        return ErrorRegistry::new(registry, error.to_string()).into();
    }

    ResultRegistered { namespace, path }.into()
}

/// Result: a Vault was registered under a namespace.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultRegistered {
    /// The namespace it was registered under.
    namespace: String,
    /// The directory it sits at.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_registered(result: ResultRegistered) {
    r_println!(
        "{}",
        t!(
            "register.result",
            namespace = result.namespace,
            path = result.path.display().to_string()
        )
        .trim()
    );
}
