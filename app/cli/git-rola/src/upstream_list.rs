//! The `upstream` command: the paths a Vault's Layout names, one per line.
//!
//! The Desktop's `git-rola@` place reads a Vault through this. A git project's Vaults are written in
//! `rola.toml` rather than bound to a Workspace, so `rola layout fetch` cannot reach them, while
//! `bring` fetches the same Layout the same way. Nothing is written: the copy is this run's own and
//! is removed when the run ends.
//!
//! Each line is a path and, after a tab, whether it is a `file` or a `tree`. The path is written the
//! way `rola.lock` writes one: `/` separated and without a leading slash. A reader that wants the
//! paths alone takes the text before the tab.

#![allow(missing_docs)]
#![allow(clippy::needless_pass_by_value)]

use librorolala::layout::EntryKind;
use mingling::{
    Grouped,
    macros::{buffer, chain, command, r_println, renderer},
};
use rola_git_support::Session;
use rust_i18n::t;

use crate::Next;
use crate::Refused;
use crate::options::operands;
use crate::upstream;

/// Lists the paths a Vault's Layout names, one per line
///
/// The Vault named is the one the run asked for; naming none reads the project's default. Each
/// line is a path, a tab, and `file` or `tree`.
#[command(node = "upstream", entry = EntryUpstream)]
pub fn upstream_list(words: Vec<String>) -> Next {
    let asked = operands(&words);

    if asked.len() > 1 {
        return Refused {
            command: "upstream",
        }
        .into();
    }

    StateUpstream {
        vault: asked.into_iter().next(),
    }
    .into()
}

/// The state an upstream run starts in: the Vault to read, or nothing for the project's default.
#[derive(Grouped)]
pub struct StateUpstream {
    /// The Vault the run named.
    vault: Option<String>,
}

/// Reads the Vault's Layout and lists what it names.
#[chain]
pub fn handle_upstream(state: StateUpstream) -> Next {
    match listing(&state) {
        Ok(result) => result.into(),
        Err(message) => crate::Fatal::new(message).into(),
    }
}

/// Result: the Layout's paths and kinds, in path order.
#[derive(Grouped)]
pub struct ResultUpstream {
    /// Each line, as it is written out.
    pub lines: Vec<String>,
}

/// Renders each path and its kind on a line of its own.
#[renderer(buffer)]
pub fn render_upstream(result: ResultUpstream) {
    for line in &result.lines {
        r_println!("{line}");
    }
}

/// Reads the project, fetches the named Vault's Layout, and lists its paths.
fn listing(state: &StateUpstream) -> Result<ResultUpstream, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| t!("git_rola.err_cwd", error = error.to_string()).to_string())?;

    let Some(session) = Session::open(&cwd).map_err(|error| error.to_string())? else {
        return Err(t!("git_rola.err_no_project").to_string());
    };

    let project = session.project();
    let config = project.config().map_err(|error| error.to_string())?;

    let name = upstream::vault_name(&config, state.vault.as_deref())
        .ok_or_else(|| t!("update.err_no_vault").to_string())?;
    let source = upstream::VaultSource::named(&config, &name)?;
    let account = upstream::account(project)?;

    let fetch = upstream::Fetch::new(project)?;
    let layout = upstream::fetch_layout(&fetch.workspace, &account, &source)?;

    let mut paths = layout.paths();
    paths.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));

    let lines = paths
        .into_iter()
        .filter_map(|(path, id)| {
            let kind = layout.entry(id)?.kind();

            Some(format!("{}\t{}", path.as_str(), token(kind)))
        })
        .collect();

    Ok(ResultUpstream { lines })
}

/// The word a kind is written by.
fn token(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::File => "file",
        EntryKind::Tree => "tree",
    }
}
