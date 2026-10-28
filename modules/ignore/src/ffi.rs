//! The rule set, across the C ABI.
//!
//! A set crosses as an opaque handle: C takes one from `load_rola_ignore_rules`, asks it about
//! paths, and gives it back with `free_rola_ignore_rules`. What a rule is and how two rules are
//! ordered never crosses, which is what lets the ordering change without the ABI changing.

use std::path::Path;

use rorolala_utils_lazyffi::lazyffi;

use crate::IgnoreRuleSet;

/// Loads the rule set `rules` holds, matched against paths written from `root`.
///
/// # FFI
///
/// A `RolaIgnoreRules` the caller releases with `free_rola_ignore_rules`, or null when the
/// directory could not be listed. A directory that is not there at all is not a failure: the
/// set is empty and ignores nothing.
#[must_use]
#[lazyffi(export = load_rola_ignore_rules)]
pub fn load_rules(rules: &Path, root: &Path) -> Option<IgnoreRuleSet> {
    IgnoreRuleSet::load(rules, root).ok()
}

#[lazyffi(export = rola_ignore_rules_)]
impl IgnoreRuleSet {
    /// Whether `path` is ignored: the last rule that matches it says it is.
    ///
    /// # FFI
    ///
    /// `is_dir` is what a rule naming a directory only — one ending in `/` — is decided by.
    #[must_use]
    #[lazyffi(export = rola_ignore_rules_is_ignore)]
    pub fn ignored(&self, path: &Path, is_dir: bool) -> bool {
        self.is_ignore(path, is_dir)
    }
}
