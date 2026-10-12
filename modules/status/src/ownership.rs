//! Who holds what, as the fetched copy of a Vault's Layout remembers it.
//!
//! Ownership lives upstream — the Vault's Layout says who holds what — and a Workspace keeps only
//! the copy a fetch wrote. That is enough for the one question a reading of the tree asks: which of
//! the changes are not this account's to make.
//!
//! Three things are answered with "no one is in the way" rather than with a refusal: a Layout that
//! tracks no Vault, a copy that was never fetched, and a run with no account bound. None of them is
//! a reason to call a change someone else's, and a reading that guessed otherwise would refuse work
//! it has no grounds to refuse.

use std::collections::BTreeMap;

use rorolala_layout::{Layout, LayoutError, LayoutPath};
use rorolala_tree_analyze::TreeDiff;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_workspace::Workspace;

/// The Vault the Layout being worked in tracks, when it tracks one.
#[must_use]
pub fn tracked_vault(workspace: &Workspace, layout: &Layout) -> Option<String> {
    let name = layout.dir().file_name()?.to_str()?;

    workspace.layouts().track(name).ok().flatten()
}

/// The fetched copy of the Vault's own Layout, when one is here to read.
///
/// It is what upstream says about an entry: who holds it, and which version it is at. A Layout that
/// tracks no Vault, and a copy that was never fetched, both answer nothing — opening a copy makes one
/// where none is there, so what is not a directory is not a copy with nothing in it.
#[must_use]
pub fn vault_copy(workspace: &Workspace, layout: &Layout) -> Option<Layout> {
    let vault = tracked_vault(workspace, layout)?;
    let dir = workspace.readonly_layout_dir(&vault, VAULT_LAYOUT_NAME);

    dir.is_dir().then(|| Layout::open(&dir).ok()).flatten()
}

/// The paths among the tree's content changes that the fetched copy says are not `me`'s to make.
///
/// The changes counted are the ones the Layout names: a path that changed where it was, and the
/// destination of a move that was edited as well. The latter is what the entry is known by *where
/// it was* — a move is not recorded until the file is — so that is where its holder is read.
///
/// A `Uuid` the copy does not name is one upstream does not hold yet, so nothing there is in the
/// way of changing it; and a `Vault` or an account that is not there is nothing to judge by.
///
/// # Errors
///
/// Returns [`LayoutError`] when a copy that is there cannot be read.
pub fn unowned(
    workspace: &Workspace,
    local: &Layout,
    vault: Option<&str>,
    me: Option<&str>,
    diff: &TreeDiff,
) -> Result<Vec<String>, LayoutError> {
    let (Some(vault), Some(me)) = (vault, me) else {
        return Ok(Vec::new());
    };

    let dir = workspace.readonly_layout_dir(vault, VAULT_LAYOUT_NAME);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let copy = Layout::open(&dir)?;
    let moved_from: BTreeMap<&str, &LayoutPath> = diff
        .renamed
        .iter()
        .map(|rename| (rename.to.as_str(), &rename.from))
        .collect();

    let mut unowned = Vec::new();

    for path in &diff.modified {
        let named = moved_from.get(path.as_str()).copied().unwrap_or(path);
        let Some(id) = local.id_of(named) else {
            continue;
        };
        let Some(data) = copy.entry(id) else {
            continue;
        };

        if data.owner() != Some(me) {
            unowned.push(path.as_str().to_owned());
        }
    }

    Ok(unowned)
}
