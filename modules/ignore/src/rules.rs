//! The set of rules a directory of rule files comes to.

use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use rorolala_utils_lazyffi::lazyffi;

use crate::natural::natural;

/// The rules a Workspace or a Vault is managed by, read from its `info/ignore/` directory.
///
/// A set is built once and asked many times: what it costs to build is a read of every rule
/// file, and what a question costs is the matching of a path against the patterns those files
/// held.
///
/// # FFI
///
/// Opaque: C takes a pointer from `load_rola_ignore_rules` and gives it back with
/// `free_rola_ignore_rules`.
#[lazyffi(export = RolaIgnoreRules)]
pub struct IgnoreRuleSet {
    /// The matcher the rules were built into.
    inner: Gitignore,
    /// What an absolute path is stripped against before it is matched: the root the rules are
    /// written from, which is the Workspace's root rather than the directory the rules sit in.
    root: PathBuf,
    /// How many rule files went in, which is what tells an empty directory apart from no
    /// directory at all — both answer "nothing is ignored", and a reader that wants to know
    /// whether rules were ever here asks this.
    files: usize,
}

impl IgnoreRuleSet {
    /// The set that ignores nothing.
    ///
    /// It is what a run that was asked to include the ignored is handed: the questions are still
    /// asked and answered, so the one flag is the whole of the difference between the two runs.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            inner: Gitignore::empty(),
            root: PathBuf::new(),
            files: 0,
        }
    }

    /// Reads every rule file under `rules`, at whatever depth it sits, into one set.
    ///
    /// The order the files are applied in is the order the rules decide by: a deeper file is
    /// applied before a shallower one, and beside its siblings a file whose name sorts sooner is
    /// applied before one whose name sorts later — so a shallower rule, or a later-sorting one,
    /// is the one that stands when two rules contradict. `root` is what the rules are written
    /// from: patterns are relative to it wherever the file holding them sits.
    ///
    /// A directory that is not there is an empty set rather than an error: rules are managed,
    /// and a place none have been written to yet is a place nothing is ignored in. A file that
    /// cannot be read, or a line that does not read as a pattern, is left out of the set the
    /// rest still make — one bad rule file is a reason to say so, not a reason the tree has no
    /// answer.
    ///
    /// # Errors
    ///
    /// Returns the reading's own failure when a directory under `rules` cannot be listed.
    pub fn load(rules: &Path, root: &Path) -> io::Result<Self> {
        if !rules.is_dir() {
            return Ok(Self {
                root: root.to_path_buf(),
                ..Self::empty()
            });
        }

        let mut found = Vec::new();
        collect(rules, 0, &mut found)?;

        // Deeper first, so the shallower rule is added last and decides; name order within, so
        // two files beside each other are read the way their names sort.
        found.sort_by(rule_order);

        let mut builder = GitignoreBuilder::new(root);
        let mut files = 0;

        for (_, file) in found {
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };

            for line in text.lines() {
                // A line that does not read as a pattern is not one: gitignore's own answer to a
                // malformed pattern is to match it literally, and what cannot even be that is
                // dropped rather than failing the set the rest of the lines make.
                let _ = builder.add_line(None, line);
            }

            files += 1;
        }

        let inner = builder
            .build()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        Ok(Self {
            inner,
            root: root.to_path_buf(),
            files,
        })
    }

    /// Whether no rule files went into the set.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.files == 0
    }

    /// Whether `path` is ignored: the last rule that matches it says it is.
    ///
    /// `is_dir` is what a rule that names a directory only — one ending in `/` — is decided by.
    /// An absolute path is stripped against the root the set was loaded with first, so a caller
    /// holding system paths and one holding the Layout's own spelling ask the same question.
    ///
    /// A path under an ignored directory is ignored, the way git reads it: the matcher answers
    /// for the one path it is given, so the directories above it are asked in turn, and one that
    /// is ignored settles it. What git also says holds here — a negation cannot bring back a
    /// file whose directory is ignored — since the directory's own answer is reached before the
    /// file's rules are ever read.
    #[must_use]
    pub fn is_ignore(&self, path: &Path, is_dir: bool) -> bool {
        let path = path.strip_prefix(&self.root).unwrap_or(path);

        if self.inner.matched(path, is_dir).is_ignore() {
            return true;
        }

        let mut dir = path.parent();
        while let Some(asking) = dir {
            if asking.as_os_str().is_empty() {
                break;
            }

            if self.inner.matched(asking, true).is_ignore() {
                return true;
            }

            dir = asking.parent();
        }

        false
    }
}

/// Every rule file under `dir`, each with its depth below the root the walk started at.
///
/// A file whose name does not read as UTF-8 is not collected: it could not be ordered against
/// the others, and an unordered rule is one two runs may apply in either order. A directory that
/// cannot be listed fails the walk, since the set would otherwise silently lack everything it
/// holds.
fn collect(dir: &Path, depth: usize, found: &mut Vec<(usize, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;

        // A symbolic link is followed nowhere: what it points at is somewhere else, and a rule
        // file is a file this directory holds.
        if kind.is_symlink() {
            continue;
        }

        if kind.is_dir() {
            collect(&entry.path(), depth + 1, found)?;
            continue;
        }

        if !kind.is_file() {
            continue;
        }

        // The name is the rule's say in the ordering, so one that cannot be read is left out
        // rather than guessed at.
        if entry.file_name().to_str().is_some() {
            found.push((depth, entry.path()));
        }
    }

    Ok(())
}

/// The order two rule files are applied in: deeper before shallower, then by name.
///
/// The name comparison runs over the path's components, so two files in one directory order by
/// their own names, and two in different directories by the directories' names first — either
/// way two runs of the loader apply one set of files in one order.
fn rule_order(left: &(usize, PathBuf), right: &(usize, PathBuf)) -> Ordering {
    right
        .0
        .cmp(&left.0)
        .then_with(|| by_components(&left.1, &right.1))
}

/// Compares two paths component by component, each by the natural ordering.
fn by_components(left: &Path, right: &Path) -> Ordering {
    let mut left = left.components();
    let mut right = right.components();

    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let a = a.as_os_str().to_string_lossy();
                let b = b.as_os_str().to_string_lossy();

                match natural(&a, &b) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    use super::IgnoreRuleSet;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-ignore-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A rule file with `lines` under `dir`, made with its parents.
    fn rule(dir: &Path, name: &str, lines: &str) {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, lines).unwrap();
    }

    /// The set `dir` makes, with the questions asked as relative paths.
    fn load(dir: &Path) -> IgnoreRuleSet {
        IgnoreRuleSet::load(dir, dir).unwrap()
    }

    #[test]
    fn a_directory_that_is_not_there_ignores_nothing() {
        let dir = scratch("absent").join("not-there");

        let set = IgnoreRuleSet::load(&dir, &dir).unwrap();

        assert!(set.is_empty());
        assert!(!set.is_ignore(Path::new("a.mb"), false));
    }

    #[test]
    fn a_pattern_ignores_what_it_matches() {
        let dir = scratch("plain");
        rule(&dir, "maya", "*.mb\n");

        let set = load(&dir);

        assert!(set.is_ignore(Path::new("scenes/a.mb"), false));
        assert!(!set.is_ignore(Path::new("scenes/a.ma"), false));
    }

    #[test]
    fn a_later_sorting_sibling_decides_over_an_earlier_one() {
        let dir = scratch("siblings");
        rule(&dir, "1-maya", "*.mb\n");
        rule(&dir, "2-keep", "!*.mb\n");

        let set = load(&dir);

        assert!(!set.is_ignore(Path::new("a.mb"), false));
    }

    #[test]
    fn numbers_in_names_sort_by_value() {
        let dir = scratch("numbers");
        // Were the names ordered by byte, `10` would apply before `2` and the negation would win.
        rule(&dir, "2", "*.mb\n");
        rule(&dir, "10", "!*.mb\n");

        let set = load(&dir);

        assert!(!set.is_ignore(Path::new("a.mb"), false));
    }

    #[test]
    fn a_shallower_file_decides_over_a_deeper_one() {
        let dir = scratch("depths");
        rule(&dir, "vaults/origin/maya", "*.mb\n");
        rule(&dir, "keep", "!*.mb\n");

        let set = load(&dir);

        // The deep rule is the fetched one, and the shallow one suppresses it.
        assert!(!set.is_ignore(Path::new("a.mb"), false));

        let other = scratch("depths-reversed");
        rule(&other, "vaults/origin/keep", "!*.mb\n");
        rule(&other, "maya", "*.mb\n");

        let set = load(&other);
        assert!(set.is_ignore(Path::new("a.mb"), false));
    }

    #[test]
    fn a_negation_inside_one_file_still_decides() {
        let dir = scratch("within");
        rule(&dir, "maya", "*.mb\n!keep.mb\n");

        let set = load(&dir);

        assert!(set.is_ignore(Path::new("a.mb"), false));
        assert!(!set.is_ignore(Path::new("keep.mb"), false));
    }

    #[test]
    fn a_directory_rule_is_decided_by_what_the_path_is() {
        let dir = scratch("dirs");
        rule(&dir, "builds", "build/\n");

        let set = load(&dir);

        assert!(set.is_ignore(Path::new("build"), true));
        assert!(!set.is_ignore(Path::new("build"), false));
        assert!(set.is_ignore(Path::new("build/out.bin"), false));
    }

    #[test]
    fn an_absolute_path_is_stripped_against_the_root() {
        let dir = scratch("absolute");
        rule(&dir, "rules/maya", "*.mb\n");

        // The rules sit in a directory of their own, and the paths asked about are written from
        // the root the work is at — the two are not the same directory.
        let set = IgnoreRuleSet::load(&dir.join("rules"), &dir).unwrap();

        assert!(set.is_ignore(&dir.join("scenes/a.mb"), false));
        assert!(!set.is_ignore(&dir.join("scenes/a.ma"), false));
    }

    #[test]
    fn a_set_with_no_files_is_empty_but_builds() {
        let dir = scratch("none");

        let set = load(&dir);

        assert!(set.is_empty());
        assert!(!set.is_ignore(Path::new("a.mb"), false));
    }
}
