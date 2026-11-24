//! The page that lets the set be looked through.
//!
//! The set is some 1500 names under `assets/<weight>/`, and choosing one by its name alone is guesswork.
//! The page is therefore generated rather than kept in the tree: it is only useful beside what it draws, and
//! it is only right when it is what the pinned commit holds. It is written next to the clone rather than
//! into it, so that the clone stays exactly what the pin holds and `git status` in it says nothing.
//!
//! The set has no index of its own, unlike the one this tool was first written for: the directory of the
//! weight being drawn is the index, and the page is built from the names of the files in it.

use std::fs;
use std::path::{Path, PathBuf};

/// Where the page goes, from the root of the tree: a directory of its own, beside the clone it draws from.
pub const HOME: &str = "THIRD_PARTY/phosphor-icon-preview";

/// The page itself, with `__ICONS__` standing where its index goes.
const TEMPLATE: &str = include_str!("preview.html");

/// Writes the page beside the clone, and hands back where it went.
///
/// Each row is `[name, tags, categories]`, the shape the page reads. Phosphor's files carry no tags or
/// categories, so those two are empty and the page searches the name alone.
pub fn write(root: &Path, clone: &Path, weight: &str) -> Result<PathBuf, String> {
    let folder = clone.join("assets").join(weight);
    let mut names = Vec::new();

    for entry in fs::read_dir(&folder)
        .map_err(|error| format!("`{}` could not be read: {error}", folder.display()))?
    {
        let entry =
            entry.map_err(|error| format!("`{}` could not be read: {error}", folder.display()))?;
        let path = entry.path();

        if path.extension().is_none_or(|extension| extension != "svg") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        // The regular weight is filed under the bare name and every other weight carries its own as a
        // suffix, so the name the page shows is the one the manifest can name.
        let name = if weight == "regular" {
            stem.to_owned()
        } else {
            let suffix = format!("-{weight}");
            match stem.strip_suffix(&suffix) {
                Some(name) => name.to_owned(),
                None => continue,
            }
        };

        names.push(name);
    }

    names.sort();

    let rows = names
        .into_iter()
        .map(|name| (name, [0u8; 0], [0u8; 0]))
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
