//! What a new Workspace or Vault ignores to begin with.
//!
//! The rules are a file like any other in the managed directory, rather than something compiled
//! in: what a project ignores is the project's to change, and a rule set that began with nothing
//! would leave the platform's own files as work to be confirmed on the first run of every new
//! project. What is written is only what no work ever carries — the files the platforms keep
//! beside it — so that a project that wants none of it can delete one line.

use std::fs;
use std::io;
use std::path::Path;

/// What a new Workspace or Vault ignores to begin with.
pub const DEFAULT_RULES: &str = "\
# What no work carries: the files the platforms keep beside it, which nothing is to be done with.
#
# Windows reserves these names for its devices. A program that writes to `nul`, or that copies a
# file through one of the others, leaves a file of that name beside the work; the two files below
# are what Explorer and Office leave behind, and the two directories are the system's own.
nul
con
aux
prn
com1
com2
com3
com4
com5
com6
com7
com8
com9
lpt1
lpt2
lpt3
lpt4
lpt5
lpt6
lpt7
lpt8
lpt9
Thumbs.db
desktop.ini
$RECYCLE.BIN/
System Volume Information/

# macOS: what Finder, Spotlight and the trash keep beside a file.
.DS_Store
._*
.Spotlight-V100/
.fseventsd/
.Trashes/
";

/// The name a Workspace keeps its default rules under.
pub const WORKSPACE_FILE: &str = "default";

/// The name a Vault hands its default rules down under.
///
/// It is longer than the Workspace's because it is a different thing: a Vault's rules arrive in a
/// Workspace that wrote its own, and a name that said which was fetched would only be read as the
/// other one until someone looked.
pub const VAULT_FILE: &str = "default-ignore-rule";

/// Writes [`DEFAULT_RULES`] into `dir` as `name`, unless a file of that name is already there.
///
/// The directory is made with it, so that a place whose rules were never written to has the one
/// the new project starts with. A file that is already there is left alone: the rules are the
/// project's, and what a run found is not a run's to put back.
///
/// # Errors
///
/// Returns the writing's own failure.
pub fn write_default(dir: &Path, name: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;

    let file = dir.join(name);

    if file.exists() {
        return Ok(());
    }

    fs::write(file, DEFAULT_RULES)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{DEFAULT_RULES, VAULT_FILE, WORKSPACE_FILE, write_default};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-ignore-defaults-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);

        dir
    }

    #[test]
    fn a_new_place_gets_the_default_rules() {
        let dir = scratch("written");
        let rules = dir.join("ignore");

        write_default(&rules, WORKSPACE_FILE).unwrap();

        let file = rules.join(WORKSPACE_FILE);
        assert_eq!(fs::read_to_string(&file).unwrap(), DEFAULT_RULES);
        assert!(
            DEFAULT_RULES.contains("nul\n"),
            "the platform's devices are named"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rule_file_that_is_there_is_left_alone() {
        let dir = scratch("kept");
        let rules = dir.join("ignore");
        fs::create_dir_all(&rules).unwrap();
        fs::write(rules.join(WORKSPACE_FILE), "*.mine\n").unwrap();

        write_default(&rules, WORKSPACE_FILE).unwrap();

        // The rules are the project's: what a run found is not a run's to put back.
        assert_eq!(
            fs::read_to_string(rules.join(WORKSPACE_FILE)).unwrap(),
            "*.mine\n"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_and_a_workspace_write_the_same_rules_under_their_own_names() {
        let dir = scratch("names");

        write_default(&dir.join("workspace"), WORKSPACE_FILE).unwrap();
        write_default(&dir.join("vault"), VAULT_FILE).unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("workspace").join(WORKSPACE_FILE)).unwrap(),
            fs::read_to_string(dir.join("vault").join(VAULT_FILE)).unwrap()
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
