//! The git-rola side of `rola fs-ops mv`: a project whose record is `rola.toml` and `rola.lock`.
//!
//! A git-rola project is a git repository with a `rola.toml`: `git rola` keeps the working tree in
//! step with `rola.lock`, and the mappings in `rola.toml` say where each entry is meant to sit. A
//! move in such a project changes those two files and nothing else of Rorolala's. There is no
//! Workspace Layout to keep in step, and the materials are placed by `git rola`, so the record the
//! move has to change is the project's own.
//!
//! The toml is edited with `toml_edit` rather than parsed into values and written again, because the
//! file is a person's: comments, order and the spelling of every key this move is not about stay as
//! they were. The lock is this project's own format rather than a person's, so it is read and
//! encoded through [`rola_define_format::Lock`], which is also what refuses two materials at one
//! path.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rola_define_format::{Lock, LockPath, Material};
use rorolala_utils_location::normalize;

/// The directory of the git-rola project `path` sits in, or nothing when it sits in none.
///
/// A git-rola project is read from the two files that make one: a `rola.toml` file and a `.git`
/// that is a directory or, for a work tree or a submodule, a file that names one. The nearest
/// directory holding both is the project, which is what a path inside a nested project resolves to.
#[must_use]
pub fn project_of(path: &Path) -> Option<PathBuf> {
    let path = if path.is_absolute() {
        normalize(path)
    } else {
        normalize(&std::env::current_dir().ok()?.join(path))
    };
    let mut at = if path.is_dir() {
        path
    } else {
        path.parent()?.to_path_buf()
    };

    loop {
        if at.join("rola.toml").is_file() && at.join(".git").exists() {
            return Some(at);
        }

        at = at.parent()?.to_path_buf();
    }
}

/// A path under `root`, written the way `rola.toml` and `rola.lock` write one.
///
/// Both files name a path relative to the project's root with `/` between the parts and no leading
/// slash, while the command line hands this run paths of the machine's own spelling.
#[must_use]
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// A file this move could not read or write, and what was said about it.
///
/// The path is kept beside the words because the words are the reader's language and the path is
/// not: a message that named only one of them would leave the other to be guessed.
#[derive(Debug)]
pub struct Failed {
    /// The file.
    pub path: PathBuf,
    /// What went wrong, as the reader of the file said it.
    pub said: String,
}

impl Failed {
    /// The failure of `path`, said by `error`.
    fn new(path: &Path, error: impl std::fmt::Display) -> Self {
        Self {
            path: path.to_path_buf(),
            said: error.to_string(),
        }
    }
}

/// Whether `rola.toml` or `rola.lock` already names `path`.
///
/// Asked before anything moves so that a destination the project already names is refused rather
/// than written twice: two materials at one path is what the lock refuses, and a mapping repeated
/// is a second answer to where one entry sits.
///
/// # Errors
///
/// Returns [`Failed`] when the project's toml or lock cannot be read or is not a file this build
/// knows.
pub fn taken(root: &Path, path: &str) -> Result<bool, Failed> {
    let toml_path = root.join("rola.toml");
    let text = fs::read_to_string(&toml_path).map_err(|error| Failed::new(&toml_path, error))?;
    let document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| Failed::new(&toml_path, error))?;

    let named = document
        .get("mapping")
        .and_then(toml_edit::Item::as_array_of_tables)
        .is_some_and(|tables| {
            tables.iter().any(|table| {
                table
                    .get("to")
                    .and_then(toml_edit::Item::as_str)
                    .is_some_and(|to| to.strip_prefix('/').unwrap_or(to) == path)
            })
        });

    if named {
        return Ok(true);
    }

    let lock_path = root.join("rola.lock");

    match fs::read_to_string(&lock_path) {
        Ok(text) => {
            let lock = Lock::decode(&text).map_err(|error| Failed::new(&lock_path, error))?;

            Ok(lock
                .materials()
                .iter()
                .any(|material| material.path().as_str() == path))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(Failed::new(&lock_path, error)),
    }
}

/// The project's record as it would read after the move, with nothing written yet.
///
/// Read and computed before the filesystem moves anything, so a toml or lock this build cannot read
/// is a refusal rather than a move half made. The files that would not change are left out, so a
/// move of a path neither file names writes nothing at all.
#[derive(Debug)]
pub struct Plan {
    /// `rola.toml` and its text, when the move changes it.
    toml: Option<(PathBuf, String)>,
    /// `rola.lock` and its text, when the move changes it.
    lock: Option<(PathBuf, String)>,
}

/// Works out what the move does to the project's record.
///
/// `from` and `to` are project paths, as [`relative`] writes them.
///
/// # Errors
///
/// Returns [`Failed`] when `rola.toml` or `rola.lock` cannot be read, or when the lock the move
/// would leave is one this build does not accept.
pub fn plan(root: &Path, from: &str, to: &str) -> Result<Plan, Failed> {
    changed(
        root,
        |text| retarget_toml(text, from, to),
        |text| retarget_lock(text, from, to),
    )
}

/// Works out what a removal does to the project's record.
///
/// A path a mapping names takes that mapping with it, and a path a material names takes the
/// material; a file inside a tree is neither, since the tree is one material rather than the paths
/// in it, so a removal there leaves the record as it is.
///
/// # Errors
///
/// Returns [`Failed`] when `rola.toml` or `rola.lock` cannot be read, or when the lock the removal
/// would leave is one this build does not accept.
pub fn removing(root: &Path, path: &str) -> Result<Plan, Failed> {
    changed(
        root,
        |text| without_toml(text, path),
        |text| without_lock(text, path),
    )
}

/// The project's record as it would read after `toml` and `lock` are applied to it, unwritten.
///
/// # Errors
///
/// Returns [`Failed`] when either file cannot be read, or when what the change makes of it is not
/// something this build accepts.
fn changed(
    root: &Path,
    toml: impl Fn(&str) -> Result<String, String>,
    lock: impl Fn(&str) -> Result<String, String>,
) -> Result<Plan, Failed> {
    let toml_path = root.join("rola.toml");
    let toml_text =
        fs::read_to_string(&toml_path).map_err(|error| Failed::new(&toml_path, error))?;
    let next = toml(&toml_text).map_err(|said| Failed::new(&toml_path, said))?;
    let toml = (next != toml_text).then_some((toml_path, next));

    let lock_path = root.join("rola.lock");
    let lock_text = match fs::read_to_string(&lock_path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(Failed::new(&lock_path, error)),
    };
    let lock = match lock_text {
        Some(text) => {
            let next = lock(&text).map_err(|said| Failed::new(&lock_path, said))?;

            (next != text).then_some((lock_path, next))
        }
        None => None,
    };

    Ok(Plan { toml, lock })
}

/// `text` with every mapping at `path` or under it taken out.
///
/// What is taken out is the whole `[[mapping]]` block, which is the entry rather than a line in it;
/// the comments and order around the other blocks stay as they were.
///
/// # Errors
///
/// Returns a message when `text` is not toml this build can read.
pub fn without_toml(text: &str, path: &str) -> Result<String, String> {
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| error.to_string())?;

    let Some(tables) = document
        .get_mut("mapping")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
    else {
        return Ok(document.to_string());
    };

    let mut remove = Vec::new();

    for (index, table) in tables.iter().enumerate() {
        if let Some(to) = table.get("to").and_then(toml_edit::Item::as_str)
            && at_or_under(to, path)
        {
            remove.push(index);
        }
    }

    for index in remove.into_iter().rev() {
        tables.remove(index);
    }

    Ok(document.to_string())
}

/// `text` with every material at `path` or under it taken out.
///
/// The materials are sorted by the lock's own rule again, so the file keeps one spelling per set.
///
/// # Errors
///
/// Returns a message when `text` is not a lock this build reads, or when what is left is not one it
/// accepts.
pub fn without_lock(text: &str, path: &str) -> Result<String, String> {
    let lock = Lock::decode(text).map_err(|error| error.to_string())?;

    let mut materials = Vec::with_capacity(lock.materials().len());
    let mut changed = false;

    for material in lock.materials() {
        if at_or_under(material.path().as_str(), path) {
            changed = true;

            continue;
        }

        materials.push(material.clone());
    }

    if !changed {
        return Ok(text.to_owned());
    }

    Lock::new(lock.vaults().to_vec(), materials)
        .map(|lock| lock.encode())
        .map_err(|error| error.to_string())
}

/// Writes the record [`plan`] worked out, the toml first and the lock after it.
///
/// The two are not one file, so no order is whole. The toml is the project's own answer and goes
/// first: a lock left behind is regenerated by `git rola update`, while a toml left behind would be
/// a mapping the next update resolves to the old place again.
///
/// # Errors
///
/// Returns [`Failed`] when either file cannot be written.
pub fn commit(plan: Plan) -> Result<(), Failed> {
    if let Some((path, text)) = plan.toml {
        write_atomically(&path, &text).map_err(|error| Failed::new(&path, error))?;
    }

    if let Some((path, text)) = plan.lock {
        write_atomically(&path, &text).map_err(|error| Failed::new(&path, error))?;
    }

    Ok(())
}

/// `text` with every mapping whose `to` is at `from` or under it moved to `to`.
///
/// A path outside `from` is left as it is, and so is every comment, every blank line and every key
/// this move is not about. What changes is the inside of the `to` string alone; its quotes and the
/// whitespace and comment around it are the value's decor, which `toml_edit` keeps.
///
/// # Errors
///
/// Returns a message when `text` is not toml this build can read.
pub fn retarget_toml(text: &str, from: &str, to: &str) -> Result<String, String> {
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| error.to_string())?;

    let Some(tables) = document
        .get_mut("mapping")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
    else {
        return Ok(document.to_string());
    };

    for table in tables.iter_mut() {
        let Some(value) = table.get_mut("to").and_then(toml_edit::Item::as_value_mut) else {
            continue;
        };
        let Some(current) = value.as_str() else {
            continue;
        };
        let Some(next) = shifted(current, from, to) else {
            continue;
        };

        let decor = value.decor().clone();
        *value = toml_edit::Value::from(next);
        *value.decor_mut() = decor;
    }

    Ok(document.to_string())
}

/// `text` with every material at `from` or under it moved to `to`, the rest untouched.
///
/// The content hash and the kind travel with the path: a move is where an entry lies rather than
/// what it holds. The materials are sorted by the lock's own rule again, so the file keeps one
/// spelling per set.
///
/// # Errors
///
/// Returns a message when `text` is not a lock this build reads, or when the moved materials would
/// leave two of them at one path.
pub fn retarget_lock(text: &str, from: &str, to: &str) -> Result<String, String> {
    let lock = Lock::decode(text).map_err(|error| error.to_string())?;

    let mut materials = Vec::with_capacity(lock.materials().len());
    let mut changed = false;

    for material in lock.materials() {
        let Some(next) = shifted(material.path().as_str(), from, to) else {
            materials.push(material.clone());
            continue;
        };

        let path = LockPath::new(&next).map_err(|error| error.to_string())?;
        materials.push(Material::new(path, material.content(), material.kind()));
        changed = true;
    }

    if !changed {
        return Ok(text.to_owned());
    }

    Lock::new(lock.vaults().to_vec(), materials)
        .map(|lock| lock.encode())
        .map_err(|error| error.to_string())
}

/// Whether `path` is at `prefix` or under it, by path part rather than by text.
fn at_or_under(path: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }

    let relative = path.strip_prefix('/').unwrap_or(path);

    relative == prefix
        || relative
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `path` moved from `from` to `to`, or nothing when it is not under `from`.
///
/// The comparison is by path part rather than by text: `Assets/old` is not a prefix of
/// `Assets/older`, and the slash a toml's `to` opens with is kept as it was found.
fn shifted(path: &str, from: &str, to: &str) -> Option<String> {
    let (lead, relative) = path
        .strip_prefix('/')
        .map_or(("", path), |relative| ("/", relative));
    let rest = relative.strip_prefix(from)?;

    if !rest.is_empty() && !rest.starts_with('/') {
        return None;
    }

    Some(format!("{lead}{to}{rest}"))
}

/// Writes `text` to `path` by staging it beside the file and renaming it over.
///
/// A reader of the file sees the old text or the new one rather than a half written one, and a
/// failure leaves the file as it was rather than truncated.
fn write_atomically(path: &Path, text: &str) -> io::Result<()> {
    let name = path
        .file_name()
        .map_or_else(|| "rola".into(), |name| name.to_string_lossy().into_owned());
    let staging = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));

    fs::write(&staging, text)?;

    if let Err(error) = fs::rename(&staging, path) {
        let _ = fs::remove_file(&staging);

        return Err(error);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{
        commit, plan, project_of, removing, retarget_lock, retarget_toml, taken, without_lock,
        without_toml,
    };

    /// A toml with one mapping at a tree.
    const TREE_TOML: &str = "\
# a tree, recorded as one entry
[vaults]
origin = \"rola://127.0.0.1/rainbow\"

[[mapping]]
from = \"00000000-0000-0000-0000-000000000003\"
to = \"/Tree\"
pin = \"latest\"
";

    /// A lock naming the same tree.
    const TREE_LOCK: &str = "? FORMAT 2\n? VAULT origin rola://127.0.0.1/rainbow\n/Tree blake3:2222222222222222222222222222222222222222222222222222222222222222 tree\n";

    #[test]
    fn a_removed_path_leaves_the_other_mappings() {
        let next = without_toml(TOML, "Assets").expect("a toml this build reads");

        assert!(!next.contains("/Assets/hero.psd"), "{next}");
        assert!(next.contains("/Source/c.fbx"), "{next}");
        assert!(next.contains("# where the work comes from"), "{next}");
    }

    #[test]
    fn a_removed_path_leaves_the_other_materials() {
        let next = without_lock(LOCK, "Assets").expect("a lock this build reads");

        assert!(!next.contains("/Assets/hero.psd"), "{next}");
        assert!(next.contains("/Source/c.fbx"), "{next}");
        assert!(next.contains("tree"), "{next}");
    }

    #[test]
    fn a_file_inside_a_tree_is_not_the_tree() {
        let toml = without_toml(TREE_TOML, "Tree/a.txt").expect("a toml this build reads");
        let lock = without_lock(TREE_LOCK, "Tree/a.txt").expect("a lock this build reads");

        assert!(toml.contains("to = \"/Tree\""), "{toml}");
        assert!(lock.contains("/Tree blake3:"), "{lock}");
    }

    #[test]
    fn a_tree_is_taken_out_with_the_directory() {
        let toml = without_toml(TREE_TOML, "Tree").expect("a toml this build reads");
        let lock = without_lock(TREE_LOCK, "Tree").expect("a lock this build reads");

        assert!(!toml.contains("/Tree"), "{toml}");
        assert!(!toml.contains("[[mapping]]"), "{toml}");
        assert!(!lock.contains("/Tree "), "{lock}");
    }

    #[test]
    fn removing_a_file_inside_a_tree_writes_nothing() {
        let dir = scratch("tree-removal");

        fs::write(dir.join("rola.toml"), TREE_TOML).expect("a project file");
        fs::write(dir.join("rola.lock"), TREE_LOCK).expect("a lock");

        let before_toml = fs::read(dir.join("rola.toml")).expect("the project file");
        let before_lock = fs::read(dir.join("rola.lock")).expect("the lock");

        let planned = removing(&dir, "Tree/a.txt").expect("a project this build reads");
        commit(planned).expect("the record written");

        assert_eq!(fs::read(dir.join("rola.toml")).ok(), Some(before_toml));
        assert_eq!(fs::read(dir.join("rola.lock")).ok(), Some(before_lock));
    }

    /// A directory of this test's own, emptied first, under the temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rola-fs-ops-git-rola-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");

        dir
    }

    /// A toml with a comment, a vault and two mappings.
    const TOML: &str = "\
# where the work comes from
[vaults]
origin = \"rola://127.0.0.1/rainbow\"

[[mapping]]
from = \"00000000-0000-0000-0000-000000000001\"
to = \"/Assets/hero.psd\"
pin = \"latest\"

[[mapping]]
from = \"00000000-0000-0000-0000-000000000002\"
to = \"/Source/c.fbx\"
pin = \"latest\"
";

    /// A lock naming the same two materials, in the order the paths sort to.
    const LOCK: &str = "? FORMAT 2\n? VAULT origin rola://127.0.0.1/rainbow\n/Assets/hero.psd blake3:0000000000000000000000000000000000000000000000000000000000000000 file\n/Source/c.fbx blake3:1111111111111111111111111111111111111111111111111111111111111111 tree\n";

    #[test]
    fn a_mapping_is_moved_and_the_rest_is_kept() {
        let moved = retarget_toml(TOML, "Assets", "Media").expect("a toml this build reads");

        assert!(moved.contains("to = \"/Media/hero.psd\""), "{moved}");
        assert!(moved.contains("to = \"/Source/c.fbx\""), "{moved}");
        assert!(moved.contains("# where the work comes from"), "{moved}");
        assert!(
            moved.contains("origin = \"rola://127.0.0.1/rainbow\""),
            "{moved}"
        );
    }

    #[test]
    fn a_path_that_is_not_under_the_old_one_is_untouched() {
        let moved = retarget_toml(TOML, "Missing", "Media").expect("a toml this build reads");

        assert!(moved.contains("to = \"/Assets/hero.psd\""), "{moved}");
        assert!(!moved.contains("Media"), "{moved}");
    }

    #[test]
    fn a_sibling_with_the_old_prefix_is_not_under_it() {
        let toml = TOML.replace("/Assets/hero.psd", "/Assets-old/hero.psd");

        let moved = retarget_toml(&toml, "Assets", "Media").expect("a toml this build reads");

        assert!(moved.contains("to = \"/Assets-old/hero.psd\""), "{moved}");
        assert!(!moved.contains("Media"), "{moved}");
    }

    #[test]
    fn the_lock_moves_the_path_and_keeps_what_it_holds() {
        let moved = retarget_lock(LOCK, "Assets", "Media").expect("a lock this build reads");

        assert!(
            moved.contains("/Media/hero.psd blake3:0000000000000000000000000000000000000000000000000000000000000000 file"),
            "{moved}"
        );
        assert!(moved.contains("/Source/c.fbx"), "{moved}");
        assert!(!moved.contains("/Assets/hero.psd"), "{moved}");
    }

    #[test]
    fn the_lock_keeps_the_paths_sorted() {
        let lock = "? FORMAT 2\n/Source/c.fbx blake3:1111111111111111111111111111111111111111111111111111111111111111 file\n/Assets/hero.psd blake3:0000000000000000000000000000000000000000000000000000000000000000 file\n";

        let moved = retarget_lock(lock, "Source", "Aaa").expect("a lock this build reads");

        let (first, second) = moved
            .lines()
            .filter(|line| line.starts_with('/'))
            .fold((None, None), |(first, _), line| {
                (first.or(Some(line)), Some(line))
            });

        assert!(
            first.expect("a material").starts_with("/Aaa/c.fbx"),
            "{moved}"
        );
        assert!(
            second.expect("a material").starts_with("/Assets/hero.psd"),
            "{moved}"
        );
    }

    #[test]
    fn two_materials_at_one_path_are_refused() {
        let lock = "? FORMAT 2\n/Assets/old blake3:0000000000000000000000000000000000000000000000000000000000000000 file\n/Assets/new blake3:1111111111111111111111111111111111111111111111111111111111111111 file\n";

        assert!(retarget_lock(lock, "Assets/old", "Assets/new").is_err());
    }

    #[test]
    fn a_project_is_a_rola_toml_inside_a_git_repository() {
        let dir = scratch("project");

        fs::write(dir.join("rola.toml"), TOML).expect("a project file");
        assert_eq!(project_of(&dir), None);

        fs::create_dir(dir.join(".git")).expect("a git directory");
        assert_eq!(project_of(&dir), Some(dir.clone()));
    }

    #[test]
    fn a_destination_the_project_names_is_taken() {
        let dir = scratch("taken");

        fs::write(dir.join("rola.toml"), TOML).expect("a project file");
        fs::write(dir.join("rola.lock"), LOCK).expect("a lock");

        assert!(taken(&dir, "Assets/hero.psd").expect("a toml this build reads"));
        assert!(taken(&dir, "Source/c.fbx").expect("a toml this build reads"));
        assert!(!taken(&dir, "Assets/other.psd").expect("a toml this build reads"));
    }

    #[test]
    fn a_planned_move_writes_both_files() {
        let dir = scratch("planned");

        fs::write(dir.join("rola.toml"), TOML).expect("a project file");
        fs::write(dir.join("rola.lock"), LOCK).expect("a lock");

        let planned = plan(&dir, "Assets", "Media").expect("a project this build reads");
        commit(planned).expect("the record written");

        let toml = fs::read_to_string(dir.join("rola.toml")).expect("the project file");
        let lock = fs::read_to_string(dir.join("rola.lock")).expect("the lock");

        assert!(toml.contains("to = \"/Media/hero.psd\""), "{toml}");
        assert!(lock.contains("/Media/hero.psd"), "{lock}");
        assert!(!lock.contains("/Assets/hero.psd"), "{lock}");
    }

    #[test]
    fn a_project_without_a_lock_is_still_a_project() {
        let dir = scratch("without-lock");

        fs::write(dir.join("rola.toml"), TOML).expect("a project file");

        let planned = plan(&dir, "Assets", "Media").expect("a project this build reads");
        commit(planned).expect("the record written");

        assert!(
            fs::read_to_string(dir.join("rola.toml"))
                .expect("the project file")
                .contains("to = \"/Media/hero.psd\"")
        );
        assert!(!dir.join("rola.lock").exists());
    }

    #[test]
    fn a_move_the_record_does_not_name_writes_nothing() {
        let dir = scratch("unnamed");

        fs::write(dir.join("rola.toml"), TOML).expect("a project file");
        fs::write(dir.join("rola.lock"), LOCK).expect("a lock");

        let before = fs::metadata(dir.join("rola.toml")).expect("the project file");

        let planned = plan(&dir, "Other", "Elsewhere").expect("a project this build reads");
        commit(planned).expect("the record written");

        let after = fs::metadata(dir.join("rola.toml")).expect("the project file");

        assert_eq!(before.len(), after.len());
        assert!(
            fs::read_to_string(dir.join("rola.toml"))
                .expect("the project file")
                .contains("to = \"/Assets/hero.psd\"")
        );
    }
}
