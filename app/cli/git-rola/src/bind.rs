//! The `git rola bind` command: name a Vault, or bind a store to the project.
//!
//! A project reaches its materials through the Vaults `rola.toml` names, and through the stores it
//! writes its content to. The two forms share one word: a Vault address is written under a name,
//! and a directory carrying `rolast.toml` is bound as a store. A bare word is an address, as it is
//! everywhere else in this program.

// mingling's `#[command]` expands the entry type and the dispatch function beside the command, and
// the expansion is not documented; the items this module writes carry their own docs.
#![allow(missing_docs)]
// A command's words arrive as the value the framework's entry converts to, so the parameter is the
// framework's shape rather than this module's choice.
#![allow(clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};

use just_fmt::fmt_case_style::CaseFormatter;
use librorolala::protocol::VaultAddress;
use librorolala::storage::RorolalaStorage;
use mingling::{
    Grouped,
    macros::{buffer, chain, command, r_eprintln, r_println, renderer},
};
use rola_git_support::{Project, ProjectConfig};
use rorolala_utils_location::normalize;
use rust_i18n::t;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, Value};

use crate::Next;
use crate::exit_codes::{EC_ERR, EC_USAGE};
use crate::upstream;

/// The name a Vault is bound under when none is given.
const DEFAULT_NAME: &str = "origin";

/// Binds a Vault address under a name, or a store to this project.
///
/// # Errors
///
/// Renders a `fatal:` with git's 128 when the project or its files cannot be read, or the store
/// cannot be bound, and the command's usage with 129 when the words are not its own.
#[command(node = "bind", entry = EntryBind)]
pub fn bind(words: Vec<String>) -> Next {
    let operands = crate::options::operands(&words);
    let Some(first) = operands.first() else {
        return crate::Refused { command: "bind" }.into();
    };

    if is_store_path(first) {
        if operands.len() > 1 {
            return crate::Fatal {
                message: t!("bind.err_no_name").to_string(),
                code: EC_USAGE,
            }
            .into();
        }

        return StateBindStore {
            path: first.clone(),
        }
        .into();
    }

    if operands.len() > 2 {
        return crate::Fatal {
            message: t!("bind.err_operands").to_string(),
            code: EC_USAGE,
        }
        .into();
    }

    StateBindVault {
        address: first.clone(),
        name: operands.get(1).cloned(),
    }
    .into()
}

/// The state of naming a Vault address.
#[derive(Grouped)]
pub struct StateBindVault {
    /// The address, as it was written.
    address: String,
    /// The name it is bound under, when one was given.
    name: Option<String>,
}

#[chain]
pub fn handle_bind_vault(state: StateBindVault) -> Next {
    let StateBindVault { address, name } = state;

    let Some(project) = current_project() else {
        return fatal(t!("bind.err_no_project").to_string());
    };

    let Some(name) = normalized_name(name.as_deref().unwrap_or(DEFAULT_NAME)) else {
        return fatal(t!("bind.err_name", name = name.as_deref().unwrap_or("")).to_string());
    };

    let address = match VaultAddress::parse(&address) {
        Ok(address) => address.to_string(),
        Err(error) => {
            return fatal(
                t!(
                    "bind.err_address",
                    address = address.as_str(),
                    error = error.to_string()
                )
                .to_string(),
            );
        }
    };

    let mut document = match read_document(&project) {
        Ok(document) => document,
        Err(message) => return fatal(message),
    };

    let table = document
        .as_table_mut()
        .entry("vaults")
        .or_insert_with(|| Item::Table(Table::new()));
    let Some(vaults) = table.as_table_mut() else {
        return fatal(t!("bind.err_config").to_string());
    };
    vaults.insert(&name, Item::Value(Value::from(address.as_str())));

    // The project's own default is what a `.vault` pin resolves through; one that has not chosen is
    // given the Vault just bound, rather than one picked by name order.
    let mut defaulted = false;
    let defaults = document
        .as_table_mut()
        .entry("defaults")
        .or_insert_with(|| Item::Table(Table::new()));
    if let Some(defaults) = defaults.as_table_mut()
        && defaults.get("vault").is_none()
    {
        defaults.insert("vault", Item::Value(Value::from(name.as_str())));
        defaulted = true;
    }

    if let Err(message) = write_document(&project, &document) {
        return fatal(message);
    }

    ResultBindVault {
        name,
        address,
        defaulted,
    }
    .into()
}

/// The state of binding a store to the project.
#[derive(Grouped)]
pub struct StateBindStore {
    /// The store, as it was written.
    path: String,
}

#[chain]
pub fn handle_bind_store(state: StateBindStore) -> Next {
    let StateBindStore { path } = state;

    let Some(project) = current_project() else {
        return fatal(t!("bind.err_no_project").to_string());
    };

    let path = absolute(&path);

    if path.starts_with(project.root().join(".rola")) {
        return fatal(
            t!(
                "bind.err_storage_inside_data",
                path = path.display().to_string()
            )
            .to_string(),
        );
    }

    if !path.is_dir() {
        return fatal(
            t!(
                "bind.err_storage_not_directory",
                path = path.display().to_string()
            )
            .to_string(),
        );
    }

    let Some(store) = RorolalaStorage::at(&path) else {
        return fatal(
            t!(
                "bind.err_storage_not_a_store",
                path = path.display().to_string()
            )
            .to_string(),
        );
    };

    // The reverse record goes first: a store that records nobody is one a collecting run cannot
    // keep this project's content for, and a project bound to it anyway would be the unsafe order.
    let read_only = !store.is_writable();
    let recorded = if read_only {
        false
    } else {
        match store.connect_workspace(project.root()) {
            Ok(changed) => changed,
            Err(error) => {
                return fatal(
                    t!(
                        "bind.err_storage_reverse",
                        path = path.display().to_string(),
                        error = error.to_string()
                    )
                    .to_string(),
                );
            }
        }
    };

    let already = match upstream::project_stores(&project) {
        Ok(stores) => stores
            .bindings()
            .iter()
            .any(|binding| binding.root() == path),
        Err(message) => return fatal(message),
    };

    if !already {
        let mut document = match read_document(&project) {
            Ok(document) => document,
            Err(message) => return fatal(message),
        };

        let item = document
            .as_table_mut()
            .entry("storages")
            .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()));
        let Some(storages) = item.as_array_of_tables_mut() else {
            return fatal(t!("bind.err_config").to_string());
        };

        let mut table = Table::new();
        table.insert(
            "path",
            Item::Value(Value::from(path.to_string_lossy().into_owned().as_str())),
        );
        storages.push(table);

        if let Err(message) = write_document(&project, &document) {
            return fatal(message);
        }
    }

    ResultBindStore {
        path,
        recorded,
        read_only,
    }
    .into()
}

/// Result: a Vault address was bound under a name.
#[derive(Grouped)]
pub struct ResultBindVault {
    /// The name it is bound under.
    name: String,
    /// The address it is bound to.
    address: String,
    /// Whether the project's default was set by this run.
    defaulted: bool,
}

#[renderer(buffer)]
pub fn render_result_bind_vault(result: ResultBindVault) {
    r_println!(
        "{}",
        t!(
            "bind.result_vault",
            name = result.name.as_str(),
            address = result.address.as_str()
        )
        .trim()
    );

    if result.defaulted {
        r_println!(
            "{}",
            t!("bind.result_default", name = result.name.as_str()).trim()
        );
    }
}

/// Result: a store was bound to the project.
#[derive(Grouped)]
pub struct ResultBindStore {
    /// The store that was bound.
    path: PathBuf,
    /// Whether the store recorded the binding back.
    recorded: bool,
    /// Whether the store is read-only.
    read_only: bool,
}

#[renderer(buffer)]
pub fn render_result_bind_store(result: ResultBindStore) {
    r_println!(
        "{}",
        t!(
            "bind.result_storage",
            path = result.path.display().to_string()
        )
        .trim()
    );

    if result.recorded {
        r_println!("{}", t!("bind.result_recorded").trim());
    }

    if result.read_only {
        r_eprintln!(
            "{}",
            t!(
                "bind.result_readonly",
                path = result.path.display().to_string()
            )
            .trim()
        );
    }
}

/// A command that could not be made, as [`crate::Fatal`] says it.
fn fatal(message: String) -> Next {
    crate::Fatal {
        message,
        code: EC_ERR,
    }
    .into()
}

/// The project the run is in.
fn current_project() -> Option<Project> {
    let cwd = std::env::current_dir().ok()?;

    Project::locate(&cwd)
}

/// The project file, read as text.
fn read_document(project: &Project) -> Result<DocumentMut, String> {
    let text = std::fs::read_to_string(project.config_path())
        .map_err(|error| t!("bind.err_config", error = error.to_string()).to_string())?;

    text.parse().map_err(|error: toml_edit::TomlError| {
        t!("bind.err_config", error = error.to_string()).to_string()
    })
}

/// Writes the project file, after it has been read back as a project.
fn write_document(project: &Project, document: &DocumentMut) -> Result<(), String> {
    let text = document.to_string();

    // `toml_edit` is about text; whether the text means a project is `ProjectConfig`'s to say.
    if let Err(error) = toml::from_str::<ProjectConfig>(&text) {
        return Err(t!("bind.err_config", error = error.to_string()).to_string());
    }

    upstream::write_atomically(&project.config_path(), &text)
}

/// `path` made absolute against the run's current directory, then normalised.
fn absolute(path: &str) -> PathBuf {
    let path = Path::new(path);

    if path.is_absolute() {
        return normalize(path);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    normalize(&cwd.join(path))
}

/// The name a binding may be given, as `rola bind` normalizes one.
fn normalized_name(name: &str) -> Option<String> {
    let normalized = CaseFormatter::from(name).to_kebab_case();

    (!normalized.is_empty()).then_some(normalized)
}

/// Whether what a bind was given names a store rather than a Vault address.
fn is_store_path(word: &str) -> bool {
    if word.starts_with("rola://") {
        return false;
    }

    let at = word.find('@');
    let colon = word.find(':');
    if at.is_some() && (colon.is_none() || at < colon) {
        return false;
    }

    if word.starts_with(['/', '\\', '.']) {
        return true;
    }

    let mut symbols = word.chars();
    if matches!((symbols.next(), symbols.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic())
    {
        return true;
    }

    word.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::{is_store_path, normalized_name};

    #[test]
    fn an_address_and_a_path_are_told_apart_by_their_shape() {
        assert!(!is_store_path("rola://127.0.0.1/"));
        assert!(!is_store_path("127.0.0.1:7717"));
        assert!(!is_store_path("origin"));
        assert!(!is_store_path("alice@example.test:/srv/vault"));

        assert!(is_store_path("/srv/rola/shared"));
        assert!(is_store_path("./shared"));
        assert!(is_store_path(r"C:\rola\shared"));
        assert!(is_store_path("shared/"));
        assert!(is_store_path("../shared"));
    }

    #[test]
    fn a_name_is_written_as_one_word_with_hyphens() {
        assert_eq!(normalized_name("My Vault"), Some("my-vault".to_owned()));
        assert_eq!(normalized_name("origin"), Some("origin".to_owned()));
        assert_eq!(normalized_name("!!!"), None);
    }
}
