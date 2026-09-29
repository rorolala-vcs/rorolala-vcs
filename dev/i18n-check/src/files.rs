//! Finds the files a scan reads under a directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Every file under `dir` whose extension is one of `extensions`, in path order.
///
/// A directory named in `skip` is not walked into, which is how a build tree that sits inside a
/// source tree is left alone.
///
/// # Errors
///
/// Returns a message naming `dir` when it cannot be read.
pub fn under(dir: &Path, extensions: &[&str], skip: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();

    collect(dir, extensions, skip, &mut found)
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    found.sort();

    Ok(found)
}

/// Gathers the files under one directory, walking into the directories below it.
fn collect(
    dir: &Path,
    extensions: &[&str],
    skip: &[&str],
    found: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };

        if skip.contains(&name) {
            continue;
        }

        if entry.file_type()?.is_dir() {
            collect(&entry.path(), extensions, skip, found)?;
        } else if has_extension(&entry.path(), extensions) {
            found.push(entry.path());
        }
    }

    Ok(())
}

/// Whether `path` ends in one of `extensions`, whatever case it is written in.
fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|wanted| extension.eq_ignore_ascii_case(wanted))
        })
}
