//! The keys a directory of translation files states.
//!
//! The files are the ones `rust-i18n` reads, and the keys are read the way it reads them: a
//! mapping whose every value is written out is one key, named by the place it sits at with the
//! steps joined by `.`, and every other mapping is a step on the way to one. `_version` states the
//! file's format rather than a key, so it is not walked into.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_yaml::Value;

use crate::files;

/// The key a file states its format under, which states no translation.
const VERSION_KEY: &str = "_version";

/// Every key the translation files under `dir` state, by the locales each is written in.
///
/// # Errors
///
/// Returns a message naming a file that could not be read or parsed.
pub fn read(dir: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut nodes = BTreeMap::new();

    for file in files::under(dir, &["yml", "yaml"], &[])? {
        let text =
            fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        let document: Value =
            serde_yaml::from_str(&text).map_err(|error| format!("{}: {error}", file.display()))?;

        walk(&document, &mut Vec::new(), &mut nodes);
    }

    Ok(nodes)
}

/// Gathers the keys under one place, `path` being the steps walked to reach it.
fn walk(place: &Value, path: &mut Vec<String>, nodes: &mut BTreeMap<String, BTreeSet<String>>) {
    let Value::Mapping(mapping) = place else {
        return;
    };

    if !path.is_empty() && is_written_form(mapping) {
        let locales = mapping
            .keys()
            .filter_map(|locale| locale.as_str().map(str::to_owned))
            .collect();

        nodes.insert(path.join("."), locales);

        return;
    }

    for (key, under) in mapping {
        let Some(name) = key.as_str() else {
            continue;
        };

        if name == VERSION_KEY || !matches!(under, Value::Mapping(_)) {
            continue;
        }

        path.push(name.to_owned());
        walk(under, path, nodes);
        path.pop();
    }
}

/// Whether a mapping is one key: every value under it is written out, so what it holds is the
/// forms of one key rather than the keys under another.
fn is_written_form(mapping: &serde_yaml::Mapping) -> bool {
    !mapping.is_empty()
        && mapping
            .values()
            .all(|value| !matches!(value, Value::Mapping(_) | Value::Sequence(_)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::read;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-i18n-check-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_key_is_a_mapping_written_out_and_the_steps_above_it_are_not() {
        let dir = scratch("keys");
        fs::write(
            dir.join("one.yml"),
            "_version: 2\ncommand:\n  help:\n    en: Help\n    zh-CN: 帮助\n  deep:\n    deeper:\n      en: Deep\n",
        )
        .unwrap();

        let nodes = read(&dir).unwrap();

        assert!(nodes.contains_key("command.help"));
        assert!(nodes.contains_key("command.deep.deeper"));
        // The steps and the file's own key are not keys.
        assert!(!nodes.contains_key("command"));
        assert!(!nodes.contains_key("_version"));
        assert_eq!(
            nodes["command.help"],
            ["en", "zh-CN"].map(str::to_owned).into_iter().collect()
        );
    }

    #[test]
    fn a_later_file_states_a_key_an_earlier_one_stated() {
        let dir = scratch("later");
        fs::write(dir.join("a.yml"), "key:\n  en: first\n").unwrap();
        fs::write(dir.join("b.yml"), "key:\n  en: second\n  zh-CN: 第二\n").unwrap();

        let nodes = read(&dir).unwrap();

        assert_eq!(nodes["key"].len(), 2);
    }
}
