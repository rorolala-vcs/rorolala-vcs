//! The `check` command: what the working tree holds against what `rola.lock` names.

// mingling's `#[command]` expands the entry type and the dispatch function beside the command, and
// the expansion is not documented; the items this module writes carry their own docs.
#![allow(missing_docs)]

use mingling::{
    Grouped, StructuralData,
    macros::{buffer, chain, command, r_println, renderer},
    res::ResExitCode,
};
use rola_git_support::{Session, digest_of};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::{EC_DIFFERENCES, EC_OK};

/// Prints what differs from `rola.lock`
///
/// Each material is read and its content named; a material whose content is not the one the lock
/// names is one line of git's short status, and one that is not there at all is another. Nothing is
/// printed when the working tree is the lock's.
#[command(node = "check", entry = EntryCheck)]
pub fn check() -> StateCheck {
    StateCheck
}

/// The state a check starts in: the command names nothing.
#[derive(Grouped)]
pub struct StateCheck;

/// Compares the working tree with what the lock names.
#[chain]
pub fn handle_check(_state: StateCheck) -> Next {
    match inspect() {
        Ok(result) => result.into(),
        Err(message) => crate::Fatal::new(message).into(),
    }
}

/// Result: what a check found.
///
/// It is `StructuralData` as well as `Serialize`, so a run that asked for `--json` is answered with
/// this and not with the nothing a renderer that only writes text returns.
#[derive(Grouped, Serialize, StructuralData)]
pub struct ResultCheck {
    /// The lines of git's short status, one per material that differs.
    pub lines: Vec<String>,
    /// Whether anything differs at all.
    pub differing: bool,
}

/// Renders what a check found as git's short status, one line per material.
#[renderer(buffer)]
pub fn render_check(result: ResultCheck, ec: &mut ResExitCode) {
    for line in &result.lines {
        r_println!("{line}");
    }

    ec.exit_code = if result.differing {
        EC_DIFFERENCES
    } else {
        EC_OK
    };
}

/// Reads the project and compares every material with what the lock names.
fn inspect() -> Result<ResultCheck, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| t!("git_rola.err_cwd", error = error.to_string()).to_string())?;

    let Some(session) = Session::open(&cwd).map_err(|error| error.to_string())? else {
        return Err(t!("git_rola.err_no_project").to_string());
    };

    let lock = session
        .lock()
        .map_err(|error| t!("git_rola.err_lock", error = error.to_string()).to_string())?;
    let prefix = session.name().map_err(|error| error.to_string())?;

    let mut lines = Vec::new();

    for material in lock.materials() {
        let shown = shown(&prefix, material.path().as_str());
        let relative: std::path::PathBuf = material.path().as_str().split('/').collect();
        let target = session.project().root().join(relative);

        if !target.is_file() {
            lines.push(format!(" D {shown}"));

            continue;
        }

        let found = digest_of(&target).map_err(|error| {
            t!(
                "check.err_digest",
                path = shown.as_str(),
                error = error.to_string()
            )
            .to_string()
        })?;

        if found != material.content() {
            lines.push(format!(" M {shown}"));
        }
    }

    session.exclude(&lock).map_err(|error| error.to_string())?;

    Ok(ResultCheck {
        differing: !lines.is_empty(),
        lines,
    })
}

/// A lock's path under the name git gives it in the work tree.
fn shown(prefix: &str, path: &str) -> String {
    if prefix == "." {
        path.to_owned()
    } else {
        format!("{prefix}/{path}")
    }
}
