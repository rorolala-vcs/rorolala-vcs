//! Putting a version's content back into the tree.
//!
//! A version is what the Layout names and the store is what it means, so a run that brings a
//! version down — `rola sync`'s pull and `rola checkin` — writes the content at the path and then
//! writes down that the two agree. A path left marked as disagreed would be reported as changed by
//! the very next reading, which is the one thing a pull must not leave behind.

use std::path::Path;

use librorolala::layout::{Layout, LayoutPath};
use librorolala::tree_analyze::{Cache, cache_path, entry_of};

/// Writes down that the path `path` under `root` holds what the Layout now names.
///
/// A path that is not a file — one a pull did not write, or wrote nowhere — is nothing to remember.
///
/// # Errors
///
/// Returns a message when the path, the file, or the cache could not be read or written.
pub fn remember(layout: &Layout, root: &Path, path: &str) -> Result<(), String> {
    let path = LayoutPath::new(path).map_err(|error| error.to_string())?;
    let disk = root.join(path.to_path_buf());

    if !disk.is_file() {
        return Ok(());
    }

    let cache_file = cache_path(root, layout);
    let mut cache = Cache::read(&cache_file);
    let entry = entry_of(&disk).map_err(|error| error.to_string())?;

    cache.insert(path, entry);
    cache.write(&cache_file).map_err(|error| error.to_string())
}
