//! The directories a key search covers for a Vault.
//!
//! A Vault admits the members whose public keys are under its own `keys/`. A Vault nested
//! inside another is admitted to by both, so a search for a key goes outward: the keys of the
//! Vault itself, then of the Vault holding it, and so on to the outermost one. What the root
//! holds is therefore what the whole tree supports, and what only a Vault below the root holds
//! admits to that Vault and to nothing above it.
//!
//! The order is the precedence, and it is the whole of it: a key found nearer shadows the same
//! name further out, so a Vault's own record of a member is the one that stands for that
//! member wherever both hold it.

use crate::{CONFIG_PATH, KEYS_DIR, Vault};
use rorolala_utils_location::Locate;
use std::path::{Path, PathBuf};

/// The directories a key search covers for the Vault rooted at `root`, nearest first.
///
/// `root` is taken to be a Vault — it is where a configuration was found — so its own keys are
/// always the first scope, whether or not the directory is there yet. What follows is every
/// directory above that is a Vault in its own right; a directory in between that is not one
/// contributes nothing, since a Vault is one only because a configuration says so.
#[must_use]
pub fn key_scopes(root: &Path) -> Vec<PathBuf> {
    let mut scopes = vec![root.join(KEYS_DIR)];

    scopes.extend(
        root.ancestors()
            // The Vault itself is already the first scope; what is added is what holds it.
            .skip(1)
            .filter(|dir| dir.join(CONFIG_PATH).exists())
            .map(|dir| dir.join(KEYS_DIR)),
    );

    scopes
}

impl Vault {
    /// The directories a key search covers for this Vault, nearest first.
    ///
    /// As [`key_scopes`], which is where the rule is written down: this only says which
    /// directory the walk starts from.
    #[must_use]
    pub fn key_scopes(&self) -> Vec<PathBuf> {
        key_scopes(self.get_root())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate;

    use super::key_scopes;
    use crate::{KEYS_DIR, VAULTS_DIR, Vault};

    /// A parent directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-vault-scopes-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A Vault at `dir`, made the way a caller makes one.
    fn vault_at(dir: &Path) {
        Vault::create(dir).unwrap();
    }

    #[test]
    fn a_vault_on_its_own_covers_its_own_keys() {
        let parent = scratch("alone");
        let dir = parent.join("vault");
        vault_at(&dir);

        assert_eq!(key_scopes(&dir), [dir.join(KEYS_DIR)]);

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn a_vault_inside_another_covers_both_its_keys_and_the_ones_holding_it() {
        let parent = scratch("nested");
        let outer = parent.join("vault");
        let inner = outer.join(VAULTS_DIR).join("alpha");
        vault_at(&outer);
        vault_at(&inner);

        // Nearest first, so the Vault's own record of a member stands over the root's.
        assert_eq!(
            key_scopes(&inner),
            [inner.join(KEYS_DIR), outer.join(KEYS_DIR)]
        );

        // The root covers only its own: what a Vault below it holds is not its to admit.
        assert_eq!(key_scopes(&outer), [outer.join(KEYS_DIR)]);

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn a_directory_that_is_not_a_vault_adds_no_scope() {
        let parent = scratch("between");
        let outer = parent.join("vault");
        // Two levels below the root with nothing in between that is a Vault of its own.
        let inner = outer.join("code").join("deeper");
        vault_at(&outer);
        vault_at(&inner);

        assert_eq!(
            key_scopes(&inner),
            [inner.join(KEYS_DIR), outer.join(KEYS_DIR)]
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn the_rule_is_read_the_same_way_wherever_it_is_started_from() {
        let parent = scratch("method");
        let outer = parent.join("vault");
        let inner = outer.join("code");
        vault_at(&outer);
        vault_at(&inner);

        let vault = Vault::locate(&inner).unwrap();

        assert_eq!(vault.key_scopes(), key_scopes(&inner));

        let _ = fs::remove_dir_all(&parent);
    }
}
