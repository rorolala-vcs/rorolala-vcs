//! The page that lets the set be looked through.
//!
//! The set is some 2200 names under `svg/`, and choosing one by its name alone is guesswork. The page is
//! therefore generated rather than kept in the tree: it is only useful beside what it draws, and it is only
//! right when it is what the pinned commit holds. It is written next to the clone rather than into it, so
//! that the clone stays exactly what the pin holds and `git status` in it says nothing.
//!
//! Being generated, it can carry the set's own index — a page opened from disk may not fetch the set's
//! `data.json` for itself — so it is also the one file of this tool that no one has to serve.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Where the page goes, from the root of the tree: a directory of its own, beside the clone it draws from.
pub const HOME: &str = "THIRD-PARTY/material-icon-preview";

/// The page itself, with `__ICONS__` standing where its index goes.
const TEMPLATE: &str = include_str!("preview.html");

/// What `data.json` says, of which the page reads the index and nothing else.
#[derive(Deserialize)]
struct Data {
    /// Every icon the set holds.
    icons: Vec<Icon>,
}

/// One icon, as the set's index names it.
#[derive(Deserialize)]
struct Icon {
    /// The name the pictures are filed under.
    name: String,
    /// The words the icon is found by, which the set gives most of them but not all.
    #[serde(default)]
    tags: Vec<String>,
    /// The group the icon belongs to.
    #[serde(default)]
    categories: Vec<String>,
}

/// Writes the page beside the clone, and hands back where it went.
pub fn write(root: &Path, set: &Path) -> Result<PathBuf, String> {
    let index = set.join("data.json");
    let text = fs::read_to_string(&index)
        .map_err(|error| format!("`{}` could not be read: {error}", index.display()))?;
    let data: Data = serde_json::from_str(&text)
        .map_err(|error| format!("`{}` could not be read: {error}", index.display()))?;

    // An array per icon rather than the objects `data.json` holds: there are 2191 of them, and the key
    // names would be most of what the page carried.
    let rows = data
        .icons
        .iter()
        .map(|icon| (&icon.name, &icon.tags, &icon.categories))
        .collect::<Vec<_>>();
    let rows = serde_json::to_string(&rows)
        .map_err(|error| format!("the set's index could not be written: {error}"))?;

    let home = root.join(HOME);
    fs::create_dir_all(&home)
        .map_err(|error| format!("`{}` could not be made: {error}", home.display()))?;

    let page = home.join("index.html");
    fs::write(&page, TEMPLATE.replace("__ICONS__", &rows))
        .map_err(|error| format!("`{}` could not be written: {error}", page.display()))?;

    Ok(page)
}
