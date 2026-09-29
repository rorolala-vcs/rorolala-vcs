//! Checks that every i18n key a call site names is stated by the translation files it reads.
//!
//! A call site is a Rust `t!("key")` or a C# `I18n.Get("key")`. What answers one depends on where
//! it is written: a command line crate's keys are its own `i18n/` directory, a Desktop plugin's
//! are the host's plus its own — a plugin registers after the host — and the host's are its own.
//!
//! The scan is deliberately static and deliberately shallow. A key written as anything but a
//! string literal is counted and left alone, since what it adds up to is not known until the
//! program runs; a key that a literal names and no file states is reported, with the file and line
//! it is written at, and the program ends non-zero.
//!
//! This is a development tool: it is crude on purpose, and it is the check, not the interface.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod calls;
mod catalog;
mod csharp_calls;
mod files;
mod rust_calls;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Where the scan starts when the caller names nowhere.
const DEFAULT_ROOT: &str = ".";

/// The directory names no scan walks into.
///
/// These are build trees and caches: what they hold is made from what is already scanned, or is
/// not source at all.
const IGNORED: &[&str] = &[
    ".git",
    ".cache",
    "build",
    "target",
    "bin",
    "obj",
    "node_modules",
];

/// How a project's call sites are read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A Rust crate, its `t!` calls read from tokens.
    Rust,
    /// A C# project, its `Get` calls read from text.
    CSharp,
}

/// A place that states translations, and the sources whose call sites they answer.
struct Site {
    /// How the call sites are read.
    kind: Kind,
    /// The directory of translation files.
    catalog: PathBuf,
    /// The directory the call sites are written under.
    sources: PathBuf,
    /// Whether the project registers its translations after the host's, and so may name them too.
    plugin: bool,
}

/// What the scan found: what it checked, what it could not, and what it could not state.
#[derive(Default)]
struct Report {
    /// How many keys a literal named.
    checked: usize,
    /// How many calls named a key this cannot read.
    unchecked: usize,
    /// The keys no translation states.
    missing: Vec<Problem>,
}

/// One call naming a key no translation states.
struct Problem {
    /// The file the call is written in.
    file: PathBuf,
    /// The line the call starts on.
    line: usize,
    /// The key it names.
    key: String,
}

fn main() -> ExitCode {
    let root = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from(DEFAULT_ROOT), PathBuf::from);

    match scan(&root) {
        Err(error) => {
            eprintln!("i18n: {error}");

            ExitCode::FAILURE
        }
        Ok(report) => report.exit(),
    }
}

impl Report {
    /// Says what was checked and what was missing, and ends the program on it.
    fn exit(self) -> ExitCode {
        for problem in &self.missing {
            let file = problem.file.display();
            let line = problem.line;
            let key = &problem.key;

            eprintln!("{file}:{line}: no translation states `{key}`");
        }

        let unchecked = self.unchecked_note();

        if self.missing.is_empty() {
            println!("{} keys, every one stated{unchecked}", self.checked);

            return ExitCode::SUCCESS;
        }

        let found = self.missing.len();
        let checked = self.checked;

        eprintln!("{found} of {checked} keys are stated by no translation file{unchecked}");

        ExitCode::FAILURE
    }

    /// How much of the scan was left unread, said in parentheses or not at all.
    fn unchecked_note(&self) -> String {
        if self.unchecked == 0 {
            String::new()
        } else {
            let unchecked = self.unchecked;

            format!(" ({unchecked} named by something this cannot read)")
        }
    }
}

/// Finds every site under `root` and checks the keys its call sites name.
///
/// # Errors
///
/// Returns a message naming a file or directory that could not be read, or one that is not
/// readable as the language it is written in.
fn scan(root: &Path) -> Result<Report, String> {
    let mut sites = Vec::new();
    find(root, &mut sites)?;

    // Every catalogue is read before any call site is checked, since a plugin may name a key the
    // host states and the host's keys are only all known once every catalogue is read.
    let mut catalogs = Vec::new();
    let mut host = BTreeSet::new();

    for site in sites {
        let keys = catalog::read(&site.catalog)?;

        if site.kind == Kind::CSharp && !site.plugin {
            host.extend(keys.keys().cloned());
        }

        catalogs.push((site, keys));
    }

    let mut report = Report::default();

    for (site, keys) in catalogs {
        let mut allowed: BTreeSet<&str> = keys.keys().map(String::as_str).collect();

        if site.kind == Kind::CSharp && site.plugin {
            allowed.extend(host.iter().map(String::as_str));
        }

        let calls = match site.kind {
            Kind::Rust => rust_calls::calls(&site.sources)?,
            Kind::CSharp => csharp_calls::calls(&site.sources)?,
        };

        report.unchecked += calls.unchecked;

        for call in calls.calls {
            report.checked += 1;

            if !allowed.contains(call.key.as_str()) {
                report.missing.push(Problem {
                    file: call.file,
                    line: call.line,
                    key: call.key,
                });
            }
        }
    }

    Ok(report)
}

/// Finds every directory under `dir` that states translations and the sources they answer.
///
/// A project is a directory holding an `i18n/` beside its `Cargo.toml` or `.csproj`, so a directory
/// the walk reaches is one of those or nothing: what is a step on the way to a project is walked
/// into, and a project is not, since its translations are its own.
///
/// # Errors
///
/// Returns a message naming a directory that could not be read.
fn find(dir: &Path, found: &mut Vec<Site>) -> Result<(), String> {
    if ignored(dir) {
        return Ok(());
    }

    let catalog = dir.join("i18n");

    if catalog.is_dir() {
        if dir.join("Cargo.toml").is_file() {
            found.push(Site {
                kind: Kind::Rust,
                catalog,
                sources: dir.join("src"),
                plugin: false,
            });

            return Ok(());
        }

        if has_project(dir)? {
            found.push(Site {
                kind: Kind::CSharp,
                catalog,
                sources: dir.to_path_buf(),
                plugin: is_plugin(dir),
            });

            return Ok(());
        }
    }

    let entries = fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
        let is_dir = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", dir.display()))?
            .is_dir();

        if is_dir {
            find(&entry.path(), found)?;
        }
    }

    Ok(())
}

/// Whether a directory is one no scan walks into.
fn ignored(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORED.contains(&name))
}

/// Whether a directory holds a C# project of its own.
///
/// # Errors
///
/// Returns a message naming the directory when it cannot be read.
fn has_project(dir: &Path) -> Result<bool, String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
        let project = entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("csproj"));

        if project {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Whether a project sits under a `Plugins` directory, which is what makes it a plugin.
fn is_plugin(dir: &Path) -> bool {
    dir.components()
        .any(|component| component.as_os_str() == "Plugins")
}
