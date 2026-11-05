//! Which paths of a tree are part of its work.
//!
//! Two layers decide: the Workspace's rules, asked with the tree's path as it stands in the
//! Workspace, and the tree's own `.rolaignore`, asked with the path as it stands in the tree. The
//! tree's file decides last, so a rule written in it can take back one the Workspace holds.
//!
//! A control path is not the rules' to decide: `.rola` and `.rola-tree` are data rather than work,
//! and a name holding `~theirs` is a conflict copy a merge left behind. The rules of the tree are
//! one file at its root, and that file is work like any other.

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
    /// The rules the tree keeps at its root, which are written from the tree root.
    own: IgnoreRuleSet,
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
        let ignore = root.join(IGNORE);
        let own = match fs::read_to_string(&ignore) {
            Ok(text) => IgnoreRuleSet::from_text(root, &text)
                .map_err(|error| TreeError::Io(error.to_string()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => IgnoreRuleSet::empty(),
            Err(error) => return Err(TreeError::Io(error.to_string())),
        };

        Ok(Self {
            root: root.to_path_buf(),
            workspace,
            own,
        })
    }

    /// The rules that ignore nothing, for a reading that was told to include what is ignored.
    #[must_use]
    pub fn empty(workspace: IgnoreRuleSet) -> Self {
        Self {
            root: std::path::PathBuf::new(),
            workspace,
            own: IgnoreRuleSet::empty(),
        }
    }

    /// Whether `path`, tree-root relative, is ignored.
    ///
    /// `is_dir` is what a rule that names a directory only is decided by. The tree's own file is
    /// the last word: a rule in it decides the path whether it ignores or takes back.
    #[must_use]
    pub fn ignored(&self, path: &str, is_dir: bool) -> bool {
        if let Some(decided) = self.own.decide(Path::new(path), is_dir) {
            return decided;
        }

        self.workspace
            .is_ignore(self.root.join(path).as_path(), is_dir)
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
        let workspace = IgnoreRuleSet::from_text(&dir, "main/scratch/\n").unwrap();
        let rules = TreeRules::load(&dir.join("main"), workspace).unwrap();

        assert!(rules.ignored("scratch", true));
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
