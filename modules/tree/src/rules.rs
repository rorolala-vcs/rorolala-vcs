//! Which paths of a tree are part of its work.
//!
//! Two layers decide: the Workspace's rules, asked with the tree's path as it stands in the
//! Workspace, and the tree's own `.rolaignore` files, asked with the path as it stands under the
//! directory holding each file. The tree's files decide last, and a deeper file decides after a
//! shallower one, so a rule written in a tree can take back one the Workspace or a parent directory
//! holds. This is how git reads `.gitignore`, except that the files are named `.rolaignore`.
//!
//! A control path is not the rules' to decide: `.rola` and `.rola-tree` are data rather than work,
//! and a name holding `~theirs` is a conflict copy a merge left behind. A rules file is work like
//! any other.

use std::fs;
use std::path::Path;

use rorolala_ignore::IgnoreRuleSet;

use crate::TreeError;
use crate::consts::{CONFLICT_MARK, DATA, IGNORE, MARK};

/// The rules one tree is read by.
pub struct TreeRules {
    /// The root of the tree the rules are for.
    root: std::path::PathBuf,
    /// The rules the Workspace keeps, which are written from the Workspace root.
    workspace: IgnoreRuleSet,
    /// The rules the tree keeps, one set per `.rolaignore`, written from the directory holding it
    /// and ordered from the tree root down.
    own: Vec<(String, IgnoreRuleSet)>,
}

impl TreeRules {
    /// Reads the rules of the tree at `root`, with `workspace` for the Workspace it sits in.
    ///
    /// A tree with no `.rolaignore` has no rules of its own, and a file that cannot be read is an
    /// error rather than an empty set: a rule file that is there and unreadable is work the reading
    /// cannot account for.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the rules file cannot be read, and whatever building the
    /// patterns from it fails with.
    pub fn load(root: &Path, workspace: IgnoreRuleSet) -> Result<Self, TreeError> {
        let mut own = Vec::new();
        collect(root, root, &mut own)?;

        // The tree root decides first and a deeper directory after it, since a nested file is the
        // more particular rule of the two.
        own.sort_by(|(left, _), (right, _)| {
            left.matches('/')
                .count()
                .cmp(&right.matches('/').count())
                .then_with(|| left.cmp(right))
        });

        Ok(Self {
            root: root.to_path_buf(),
            workspace,
            own,
        })
    }

    /// The rules that ignore nothing, for a reading that was told to include what is ignored.
    #[must_use]
    pub const fn empty(workspace: IgnoreRuleSet) -> Self {
        Self {
            root: std::path::PathBuf::new(),
            workspace,
            own: Vec::new(),
        }
    }

    /// Whether `path`, tree-root relative, is ignored.
    ///
    /// `is_dir` is what a rule that names a directory only is decided by. The tree's own file is
    /// the last word: a rule in it decides the path whether it ignores or takes back.
    #[must_use]
    pub fn ignored(&self, path: &str, is_dir: bool) -> bool {
        let mut decided = None;

        for (base, rules) in &self.own {
            let relative = if base.is_empty() {
                Some(path)
            } else {
                path.strip_prefix(base)
                    .and_then(|rest| rest.strip_prefix('/'))
            };

            if let Some(relative) = relative
                && let Some(answer) = rules.decide(Path::new(relative), is_dir)
            {
                decided = Some(answer);
            }
        }

        decided.unwrap_or_else(|| {
            self.workspace
                .is_ignore(self.root.join(path).as_path(), is_dir)
        })
    }

    /// Whether `path`, tree-root relative, is one no tree holds whatever the rules say.
    #[must_use]
    pub fn is_control(path: &str) -> bool {
        path.split('/')
            .any(|part| part == DATA || part == MARK || part.contains(CONFLICT_MARK))
    }

    /// The root the rules were read from.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Reads every `.rolaignore` file under `directory`, with `root` the tree the bases are written
/// against.
///
/// A directory holding another tree or the Workspace's data is not this tree's to read, and neither
/// is a control name. A file that cannot be read is an error: rules that are there and unreadable
/// are rules the reading cannot account for, the same as at the tree's root.
fn collect(
    root: &Path,
    directory: &Path,
    own: &mut Vec<(String, IgnoreRuleSet)>,
) -> Result<(), TreeError> {
    let ignore = directory.join(IGNORE);
    match fs::read_to_string(&ignore) {
        Ok(text) => {
            let rules = IgnoreRuleSet::from_text(directory, &text)
                .map_err(|error| TreeError::Io(error.to_string()))?;
            let base = directory
                .strip_prefix(root)
                .map_err(|_| TreeError::Io(ignore.display().to_string()))?;
            let base = base
                .components()
                .map(|component| component.as_os_str().to_str())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| TreeError::Io(ignore.display().to_string()))?
                .join("/");

            own.push((base, rules));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(TreeError::Io(error.to_string())),
    }

    let held = match fs::read_dir(directory) {
        Ok(held) => held,
        Err(error) => return Err(TreeError::Io(error.to_string())),
    };

    for entry in held {
        let entry = entry.map_err(|error| TreeError::Io(error.to_string()))?;
        let Ok(kind) = entry.file_type() else {
            continue;
        };

        if !kind.is_dir() {
            continue;
        }

        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if name == DATA || name == MARK || name.contains(CONFLICT_MARK) {
            continue;
        }

        if path.join(MARK).is_dir() || path.join(DATA).is_dir() {
            continue;
        }

        collect(root, &path, own)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_ignore::IgnoreRuleSet;

    use super::TreeRules;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-tree-rules-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_tree_without_rules_holds_what_the_workspace_holds() {
        let dir = scratch("none");
        let workspace = IgnoreRuleSet::from_text(&dir, "*.psd\n").unwrap();
        let rules = TreeRules::load(&dir, workspace).unwrap();

        assert!(rules.ignored("a.psd", false));
        assert!(!rules.ignored("a.ma", false));
    }

    #[test]
    fn the_trees_file_decides_last() {
        let dir = scratch("own");
        fs::write(dir.join(".rolaignore"), "!*.psd\nkeep.ma\n").unwrap();

        let workspace = IgnoreRuleSet::from_text(&dir, "*.psd\n").unwrap();
        let rules = TreeRules::load(&dir, workspace).unwrap();

        // The tree's rule takes back the one the Workspace holds, and adds one of its own.
        assert!(!rules.ignored("a.psd", false));
        assert!(rules.ignored("keep.ma", false));
        assert!(!rules.ignored("other.ma", false));
    }

    #[test]
    fn a_rule_the_workspace_holds_reaches_into_a_tree() {
        let dir = scratch("reach");
        fs::create_dir_all(dir.join("main")).unwrap();
        let workspace = IgnoreRuleSet::from_text(&dir, "main/scratch/\n").unwrap();
        let rules = TreeRules::load(&dir.join("main"), workspace).unwrap();

        assert!(rules.ignored("scratch", true));
    }

    /// A rules file under a directory decides that directory and everything under it, the way a
    /// nested `.gitignore` does.
    #[test]
    fn a_nested_rules_file_decides_its_own_directory() {
        let dir = scratch("nested");
        fs::create_dir_all(dir.join("sub").join("deep")).unwrap();
        fs::write(dir.join("sub").join(".rolaignore"), "*.psd\n/anchored.ma\n").unwrap();

        let rules = TreeRules::load(&dir, IgnoreRuleSet::empty()).unwrap();

        assert!(rules.ignored("sub/a.psd", false));
        assert!(rules.ignored("sub/deep/a.psd", false));
        assert!(!rules.ignored("a.psd", false));
        assert!(rules.ignored("sub/anchored.ma", false));
        assert!(!rules.ignored("sub/deep/anchored.ma", false));
    }

    /// A nested file decides after the tree root, so it can take back what the root ignores.
    #[test]
    fn a_nested_rules_file_decides_after_the_tree_root() {
        let dir = scratch("nested-back");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join(".rolaignore"), "*.psd\n").unwrap();
        fs::write(dir.join("sub").join(".rolaignore"), "!keep.psd\n").unwrap();

        let rules = TreeRules::load(&dir, IgnoreRuleSet::empty()).unwrap();

        assert!(rules.ignored("a.psd", false));
        assert!(rules.ignored("sub/a.psd", false));
        assert!(!rules.ignored("sub/keep.psd", false));
    }

    #[test]
    fn a_data_directory_and_a_conflict_copy_are_never_work() {
        assert!(TreeRules::is_control(".rola/x"));
        assert!(TreeRules::is_control("a/.rola-tree/x"));
        assert!(TreeRules::is_control("a/raw~theirs.psd"));
        assert!(!TreeRules::is_control(".rolaignore"));
        assert!(!TreeRules::is_control("main.psd"));
    }

    #[test]
    fn rules_from_a_file_a_tree_cannot_read_are_an_error() {
        let dir = scratch("unreadable");
        let rules = TreeRules::load(&dir, IgnoreRuleSet::empty()).unwrap();
        assert!(!rules.ignored("a.psd", false));
    }
}
