//! The `entries` command: the paths `rola.lock` names, one a line.
//!
//! The lock is what the project actually places, so listing its paths is the question a script or a
//! person asks before `restore`: what can be named? A tree is one material and is listed at its own
//! path, not at the paths inside it, since its own path is the name that selects the whole tree.
//!
//! The paths are written as the lock writes them, not quoted the way `check` quotes a status line.
//! This output is meant to be read back, so `git rola entries | xargs git rola restore` works;
//! `check` keeps the quoting because what it prints is a status to look at rather than a name to
//! hand back.

// mingling's `#[command]` expands the entry type and the dispatch function beside the command, and
// the expansion is not documented; the items this module writes carry their own docs.
#![allow(missing_docs)]
// A command's words arrive as the value the framework's entry converts to, so the parameter is the
// framework's shape rather than this module's choice: taking them by reference is not an option the
// generated wrapper offers.
#![allow(clippy::needless_pass_by_value)]

use mingling::{
    Grouped,
    macros::{buffer, chain, command, r_println, renderer},
};
use rola_define_format::Lock;
use rola_git_support::Session;
use rust_i18n::t;

use crate::Next;
use crate::Refused;
use crate::options::operands;

/// Lists the paths `rola.lock` names, one per line
///
/// Nothing is compared and nothing is written: the lock is read and its materials are written out
/// in the order it keeps them, which is path order.
#[command(node = "entries", entry = EntryEntries)]
pub fn entries(words: Vec<String>) -> Next {
    if !operands(&words).is_empty() {
        return Refused { command: "entries" }.into();
    }

    StateEntries.into()
}

/// The state an entries run starts in: the command names nothing.
#[derive(Grouped)]
pub struct StateEntries;

/// Reads the lock and lists what it names.
#[chain]
pub fn handle_entries(_state: StateEntries) -> Next {
    match inspect() {
        Ok(result) => result.into(),
        Err(message) => crate::Fatal::new(message).into(),
    }
}

/// Result: the paths the lock names.
#[derive(Grouped)]
pub struct ResultEntries {
    /// Each material's path, in the lock's order.
    pub paths: Vec<String>,
}

/// Renders each path on a line of its own.
#[renderer(buffer)]
pub fn render_entries(result: ResultEntries) {
    for path in &result.paths {
        r_println!("{path}");
    }
}

/// Reads the project's lock and collects the paths it names.
fn inspect() -> Result<ResultEntries, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| t!("git_rola.err_cwd", error = error.to_string()).to_string())?;

    let Some(session) = Session::open(&cwd).map_err(|error| error.to_string())? else {
        return Err(t!("git_rola.err_no_project").to_string());
    };

    let lock = session
        .lock()
        .map_err(|error| t!("git_rola.err_lock", error = error.to_string()).to_string())?;

    Ok(ResultEntries {
        paths: paths(&lock),
    })
}

/// The paths `lock` names, in the order it keeps them.
fn paths(lock: &Lock) -> Vec<String> {
    lock.materials()
        .iter()
        .map(|material| material.path().as_str().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use rola_define_format::{ContentHash, Lock, LockPath, Material, MaterialKind};

    use super::paths;

    /// A lock naming `paths` at one content each.
    fn lock(paths: &[&str]) -> Lock {
        let materials = paths
            .iter()
            .map(|path| {
                Material::new(
                    LockPath::new(path).unwrap(),
                    ContentHash::from_digest([1; 32]),
                    MaterialKind::File,
                )
            })
            .collect();

        Lock::new(vec![], materials).unwrap()
    }

    #[test]
    fn the_paths_are_the_locks_own_and_in_its_order() {
        let held = lock(&["Source/c.fbx", "Assets/hero.psd"]);

        assert_eq!(paths(&held), ["Assets/hero.psd", "Source/c.fbx"]);
    }

    #[test]
    fn a_tree_is_listed_at_its_own_path() {
        let materials = vec![Material::new(
            LockPath::new("Assets/Textures").unwrap(),
            ContentHash::from_digest([2; 32]),
            MaterialKind::Tree,
        )];
        let held = Lock::new(vec![], materials).unwrap();

        assert_eq!(paths(&held), ["Assets/Textures"]);
    }

    #[test]
    fn an_empty_lock_names_nothing() {
        assert_eq!(paths(&Lock::empty()), Vec::<String>::new());
    }
}
