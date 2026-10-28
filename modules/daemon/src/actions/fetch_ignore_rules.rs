//! `fetch-ignore-rules`: the ignore rules a Vault manages, brought here whole.
//!
//! A Vault keeps a directory of ignore rules beside its Layout, and a Workspace works by the
//! rules of every Vault it has fetched from, kept under its own `info/ignore/vaults/<name>/`.
//! Being deeper than the Workspace's own files is what makes a fetched rule yield to one written
//! here: the Workspace's `!*.mb` stands over the Vault's `*.mb` because it is applied after it.
//!
//! What crosses is the directory written out whole — every rule file's name and bytes — rather
//! than a diff of it: a rule set is small, and a whole copy is what makes a rule the Vault
//! deleted vanish here on the next fetch without either side keeping a list of what used to be.
//! The copy replaces what was here as one swap, staged beside it first, so a reader either sees
//! the rules that were here or the rules the Vault holds, never half of each.

use std::fs;
use std::path::{Component, Path};

use rorolala_errors::IoError;
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_utils_constants::{
    IGNORE_RULES_VAULTS_DIR, VAULT_IGNORE_RULES_DIR, WORKSPACE_IGNORE_RULES_DIR,
};
use rorolala_utils_location::Locate as _;
use serde::{Deserialize, Serialize};

use super::sync_storage::{Side, carry_blob};

/// An Action that brings the Vault's ignore rules under the Workspace's own.
///
/// # Input
///
/// The name the Workspace keeps this Vault's rules under — the name it bound the Vault by. It
/// belongs to the Workspace and is read there; the Vault is handed it and has no use for it,
/// since which Vault is reached is the address's to say.
///
/// # Output
///
/// Nothing: what a fetch comes to is a directory the Workspace now holds, or no longer holds
/// when the Vault holds none.
pub struct ActionFetchIgnoreRules;

impl Action for ActionFetchIgnoreRules {
    /// The fetch of a Vault's rules is id 18: the first free slot after the index fetch's.
    const ID: u32 = 18;

    type Input = String;
    type Output = ();

    /// Asks the Vault for its ignore rules, and writes what it answers under the Workspace's own.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, [`ActionError::Store`] if the Vault's rule directory could not be read or sent a
    /// name that is not one, and [`ActionError::LocalFile`] if the copy could not be written here
    /// — besides whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // Which directory the copy lands in is the Workspace's own business, so the name is read
        // here and never looked at on the Vault.
        let cache = input.into_inner().unwrap_or_default();

        // What the Vault holds is read there: it is the side with a rule directory, and the
        // Workspace side has none of the Vault's to read.
        let answer = if ctx.is_vault() {
            let vault = ctx
                .current_vault()
                .into_inner()
                .ok_or(ActionError::MissingValue)?;

            Some(read_rules(&vault.get_root().join(VAULT_IGNORE_RULES_DIR))?)
        } else {
            None
        };
        let mine = answer
            .as_ref()
            .map(rorolala_protocol::Encodable::encode)
            .transpose()
            .map_err(codec_failed)?;

        // What the Vault holds crosses back, and the Workspace is where it is written: the same
        // call reads both ways, so the side that sent it keeps its own.
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        if ctx.is_workspace() {
            let files = Vec::<RuleFile>::decode(bytes).map_err(codec_failed)?;

            let workspace = ctx
                .current_workspace()
                .into_inner()
                .ok_or(ActionError::MissingValue)?;

            write_rules(workspace, &cache, &files)?;
        }

        Ok(())
    }
}

/// One rule file: the path it is kept under, relative to the rules' root, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuleFile {
    /// The path, `/`-joined, relative to the root the rules were read from.
    path: String,
    /// What the file holds.
    bytes: Vec<u8>,
}

/// Reads every rule file under `dir`, at whatever depth it sits.
///
/// A Vault that manages no rules has no directory, and that is a list of no files rather than an
/// error: the Workspace's copy is then removed, which is how a rule the Vault gave up reaches
/// the Workspace.
///
/// # Errors
///
/// Returns [`ActionError::LocalFile`] when the directory or a file under it cannot be read.
fn read_rules(dir: &Path) -> Result<Vec<RuleFile>, ActionError> {
    let mut files = Vec::new();

    if dir.is_dir() {
        read_into(dir, dir, &mut files)?;
    }

    // The order is fixed so that two runs of a fetch carry the same bytes: what crosses is
    // compared whole by whoever keeps a copy, and a directory's own listing order is no one's
    // answer to keep.
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(files)
}

/// Reads the files under `dir` into `files`, named relative to `root`.
fn read_into(root: &Path, dir: &Path, files: &mut Vec<RuleFile>) -> Result<(), ActionError> {
    let entries = fs::read_dir(dir).map_err(|error| local(dir, &error))?;

    for entry in entries {
        let entry = entry.map_err(|error| local(dir, &error))?;
        let kind = entry
            .file_type()
            .map_err(|error| local(&entry.path(), &error))?;

        // A symbolic link is followed nowhere: what it points at is somewhere else, and a rule
        // file is a file this directory holds.
        if kind.is_symlink() {
            continue;
        }

        if kind.is_dir() {
            read_into(root, &entry.path(), files)?;
            continue;
        }

        if !kind.is_file() {
            continue;
        }

        let path = entry.path();
        let bytes = fs::read(&path).map_err(|error| local(&path, &error))?;

        // UNWRAP: `path` is under `dir`, which is under `root`, by the walk that found it.
        let relative = path
            .strip_prefix(root)
            .expect("a walked file sits under the root");

        files.push(RuleFile {
            path: relative.to_string_lossy().replace('\\', "/"),
            bytes,
        });
    }

    Ok(())
}

/// Replaces the Workspace's copy of the Vault `name`'s rules with `files`.
///
/// The replacement is staged beside the copy and swapped in, so a reader never meets half of
/// two sets; a Vault holding no rules at all leaves no copy behind, which is the deletion of a
/// rule reaching here. A name that is not one directory's worth, or a file named to climb out of
/// it, is refused before anything is written.
///
/// # Errors
///
/// Returns [`ActionError::Store`] for a name or a file path that is not one the copy may hold,
/// and [`ActionError::LocalFile`] when the copy cannot be written.
fn write_rules(
    workspace: &rorolala_workspace::Workspace,
    name: &str,
    files: &[RuleFile],
) -> Result<(), ActionError> {
    if !is_plain_name(name) {
        return Err(ActionError::Store(format!(
            "`{name}` is not a name a Vault's rules can be kept under"
        )));
    }

    for file in files {
        check_path(&file.path)?;
    }

    let held = workspace
        .get_root()
        .join(WORKSPACE_IGNORE_RULES_DIR)
        .join(IGNORE_RULES_VAULTS_DIR);
    let dir = held.join(name);
    let staging = held.join(format!(".{name}.staging"));

    // What was written of a staging that failed is no use to anyone, and the copy that was here
    // was never touched.
    let _ = fs::remove_dir_all(&staging);

    if !files.is_empty() {
        fs::create_dir_all(&staging).map_err(|error| local(&staging, &error))?;

        for file in files {
            // The path crosses with `/` for a separator and is taken apart here, so what a file
            // is joined back into is this system's own spelling.
            let mut path = staging.clone();
            for part in file.path.split('/') {
                path.push(part);
            }

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| local(parent, &error))?;
            }

            if let Err(error) = fs::write(&path, &file.bytes) {
                let _ = fs::remove_dir_all(&staging);

                return Err(local(&path, &error));
            }
        }
    }

    // The swap is two renames rather than a remove and a rename: the copy that was here is let
    // go of only once the new one is whole, and a failure part of the way through leaves one of
    // the two whole under the name.
    let aside = held.join(format!(".{name}.aside"));
    let _ = fs::remove_dir_all(&aside);

    if dir.is_dir() {
        fs::rename(&dir, &aside).map_err(|error| local(&dir, &error))?;
    }

    if files.is_empty() {
        let _ = fs::remove_dir_all(&aside);

        return Ok(());
    }

    if let Err(error) = fs::rename(&staging, &dir) {
        // The copy that was here is put back: a fetch that cannot land its rules is a fetch that
        // changed nothing.
        let _ = fs::rename(&aside, &dir);

        return Err(local(&staging, &error));
    }

    let _ = fs::remove_dir_all(&aside);

    Ok(())
}

/// Refuses a path a Vault sent that is not one to write under the copy.
///
/// What a Vault sends is written under the copy by the path it was sent under, so a path that
/// climbs — an absolute one, a `..`, a drive — would write outside the one directory the fetch
/// is for.
///
/// # Errors
///
/// Returns [`ActionError::Store`] naming the path that is not a relative file path.
fn check_path(path: &str) -> Result<(), ActionError> {
    // A name the sending system allowed and this one reads as a separator — a backslash, which a
    // Unix Vault can keep in a name and a Windows Workspace would descend into — is refused with
    // the climbing ones.
    let ok = !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)));

    if ok {
        return Ok(());
    }

    Err(ActionError::Store(format!(
        "the Vault sent `{path}`, which is not a path a rule file can be kept under"
    )))
}

/// Whether `name` is one path component and nothing else.
///
/// What a Vault's rules are kept under is named by the Workspace, but the name crosses and comes
/// back, so it is checked where it is used rather than trusted from where it was read.
fn is_plain_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// What a file that could not be read or written means to an action.
///
/// The path is part of what is said: a failure that answers "access is denied" and nothing else is
/// one a reader cannot go and look at, and the two sides of a fetch fail on files of their own.
fn local(path: &Path, error: &std::io::Error) -> ActionError {
    ActionError::LocalFile(IoError::new(
        error.kind().into(),
        format!("`{}`: {error}", path.display()),
    ))
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(rorolala_errors::BincodeError::new(error.to_string()))
}

/// The directory a Workspace's copy of a Vault's rules sits in, for the tests.
#[cfg(test)]
fn dir_of(workspace: &rorolala_workspace::Workspace, name: &str) -> std::path::PathBuf {
    workspace
        .get_root()
        .join(WORKSPACE_IGNORE_RULES_DIR)
        .join(IGNORE_RULES_VAULTS_DIR)
        .join(name)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::{RuleFile, check_path, read_rules, write_rules};

    /// A Workspace of its own under the temp directory, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> (PathBuf, Workspace) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let root = std::env::temp_dir().join(format!(
            "rorolala-fetch-rules-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".rola")).unwrap();

        let workspace = Workspace::locate(&root).unwrap();

        (root, workspace)
    }

    /// Rule files as the Vault would send them.
    fn sent(files: &[(&str, &str)]) -> Vec<RuleFile> {
        files
            .iter()
            .map(|(path, bytes)| RuleFile {
                path: (*path).to_owned(),
                bytes: bytes.as_bytes().to_vec(),
            })
            .collect()
    }

    /// The copy's directory, for a Vault the Workspace keeps under `vault`.
    fn copy(workspace: &Workspace) -> PathBuf {
        super::dir_of(workspace, "vault")
    }

    /// The names under a directory, in name order, with directories descended into.
    fn names(dir: &Path) -> Vec<String> {
        let mut found = Vec::new();
        let mut stack = vec![dir.to_path_buf()];

        while let Some(held) = stack.pop() {
            let Ok(entries) = fs::read_dir(&held) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    found.push(
                        path.strip_prefix(dir)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }

        found.sort();
        found
    }

    #[test]
    fn a_copy_is_replaced_whole_and_a_deleted_rule_is_gone() {
        let (_root, workspace) = scratch("replace");

        write_rules(
            &workspace,
            "vault",
            &sent(&[("maya", "*.mb\n"), ("sub/os", "desktop.ini\n")]),
        )
        .unwrap();
        assert_eq!(names(&copy(&workspace)), ["maya", "sub/os"]);

        write_rules(&workspace, "vault", &sent(&[("maya", "*.mb\n!keep.mb\n")])).unwrap();

        assert_eq!(names(&copy(&workspace)), ["maya"]);
        assert_eq!(
            fs::read_to_string(copy(&workspace).join("maya")).unwrap(),
            "*.mb\n!keep.mb\n"
        );
    }

    #[test]
    fn a_vault_holding_no_rules_leaves_no_copy() {
        let (_root, workspace) = scratch("emptied");

        write_rules(&workspace, "vault", &sent(&[("maya", "*.mb\n")])).unwrap();
        assert!(copy(&workspace).is_dir());

        write_rules(&workspace, "vault", &[]).unwrap();

        assert!(!copy(&workspace).exists());
    }

    #[test]
    fn a_name_that_is_not_one_directory_is_refused() {
        let (_root, workspace) = scratch("names");

        for name in ["a/b", "../a", "", "."] {
            assert!(
                write_rules(&workspace, name, &[]).is_err(),
                "`{name}` was taken as a name"
            );
        }
    }

    #[test]
    fn a_path_that_climbs_is_refused_before_anything_is_written() {
        for path in ["../escape", "/absolute", "a/../../b", "", "a\\b"] {
            assert!(check_path(path).is_err(), "`{path}` was taken as a path");
        }

        assert!(check_path("sub/maya").is_ok());
    }

    #[test]
    fn a_vault_without_a_rule_directory_sends_nothing() {
        let (root, _workspace) = scratch("no-rules");

        assert_eq!(read_rules(&root.join("info/ignore")).unwrap(), vec![]);
    }

    #[test]
    fn rules_are_read_recursively_and_named_with_slashes() {
        let (root, _workspace) = scratch("reading");
        let rules = root.join("info/ignore");
        fs::create_dir_all(rules.join("sub")).unwrap();
        fs::write(rules.join("maya"), "*.mb\n").unwrap();
        fs::write(rules.join("sub").join("os"), "desktop.ini\n").unwrap();

        let files = read_rules(&rules).unwrap();

        assert_eq!(
            files,
            sent(&[("maya", "*.mb\n"), ("sub/os", "desktop.ini\n")])
        );
    }
}
