//! The `rola fs-ops` namespace: moving, copying and removing paths, and nothing besides.
//!
//! These are the file operations the Desktop program hands its work to. What a drag, a paste or a
//! removal *means* is decided there — whether a name already taken is replaced, skipped or renamed,
//! and whether a transfer moves or copies — and what is left is the act itself, which is all this is.
//! It is therefore deliberately thin, and deliberately a namespace rather than one command with
//! switches: a path is moved, copied or removed, each by its own word, so that adding a fourth
//! operation is adding a word rather than reshaping this one.
//!
//! **Nothing is asked and nothing is confirmed.** A caller that wants to ask asks first and calls this
//! with the answer; this does what it is told, which is what makes it usable from a program rather
//! than only from a terminal. There are no `-r` and no `-f` either: whether a path is a directory is
//! read from the path itself, and nothing is ever coerced.
//!
//! Reaching `fs-ops` with none of those words is a question about the namespace rather than about one
//! of its commands, so it is answered with what the namespace holds — see [`handle_fs_ops`] — and the
//! same words answer `rola fs-ops -h`.

use std::io;
use std::path::{Path, PathBuf};

use librorolala::layout::{Layout, LayoutPath};
use librorolala::workspace::{Workspace, locate_workspace};
use mingling::{
    Grouped,
    macros::{buffer, chain, dispatcher, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rorolala_utils_location::normalize;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::exit_codes::{EC_ERR_FS_OPS_ARGUMENT, EC_ERR_FS_OPS_FAILED, EC_HELP};
use crate::failure::failure;

pub mod cp;
pub mod mv;
pub mod rm;

dispatcher!("fs-ops", EntryFsOps);

#[help(buffer)]
pub fn help_fs_ops(_: EntryFsOps, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fs_ops.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFsOps)]
pub fn desc_fs_ops() -> Description {
    t!("fs_ops.cmd_fs_ops_description").to_string().into()
}

/// Names what `fs-ops` holds.
///
/// A command the namespace has is matched before this is reached, so what arrives here is a run that
/// named none of them — or one it does not have. Either is a question about the namespace rather than
/// about a command of it, and both are answered the way [`help_fs_ops`] answers.
#[chain]
pub fn handle_fs_ops(_: EntryFsOps) -> Next {
    ResultFsOpsHelp.into()
}

/// Result: what `fs-ops` holds was named.
#[derive(Grouped)]
pub struct ResultFsOpsHelp;

#[renderer(buffer)]
pub fn render_result_fs_ops_help(_: ResultFsOpsHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fs_ops.help")).trim());
    ec.exit_code = EC_HELP;
}

/// Result: a path was moved, copied or removed.
#[derive(Grouped)]
pub struct ResultFsOps {
    /// The word the operation was reached by, said as it was typed rather than translated: it is the
    /// program's word, like the command names themselves.
    verb: &'static str,
    /// What the operation was done to.
    from: String,
    /// Where it was put, for the two words that put something somewhere.
    to: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_fs_ops(result: ResultFsOps) {
    let said = if let Some(to) = result.to {
        t!(
            "fs_ops.result_two",
            verb = result.verb,
            from = result.from,
            to = to
        )
    } else {
        t!("fs_ops.result_one", verb = result.verb, from = result.from)
    };

    r_println!("{}", said.trim());
}

/// Error: an operation was named without the paths it works on.
#[derive(Grouped)]
pub struct ErrorFsOpsArguments {
    /// The word that was reached without its paths.
    verb: &'static str,
    /// Whether that word puts something somewhere, and so needs a second path.
    needs_to: bool,
}

impl Failure for ErrorFsOpsArguments {
    fn name(&self) -> &'static str {
        "error_fs_ops_argument"
    }

    fn reason(&self) -> String {
        // Said with the word's own usage in it, since which paths are wanted is not the same for every word:
        // a removal takes one and a transfer takes two, and a reader told the wrong one would type it.
        let said = if self.needs_to {
            t!("fs_ops.err_argument_two", verb = self.verb)
        } else {
            t!("fs_ops.err_argument_one", verb = self.verb)
        };

        said.trim().to_string()
    }
}

failure!(ErrorFsOpsArguments);

#[renderer(buffer)]
pub fn render_error_fs_ops_arguments(error: ErrorFsOpsArguments, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("fs_ops.err_argument_help").trim()));
    ec.exit_code = EC_ERR_FS_OPS_ARGUMENT;
}

/// Error: the filesystem refused the operation.
#[derive(Grouped)]
pub struct ErrorFsOpsFailed {
    /// The word that failed.
    verb: &'static str,
    /// What the filesystem said, which is the whole of what is known about why.
    cause: String,
}

impl Failure for ErrorFsOpsFailed {
    fn name(&self) -> &'static str {
        "error_fs_ops_failed"
    }

    fn reason(&self) -> String {
        t!("fs_ops.err_failed", verb = self.verb, reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorFsOpsFailed);

#[renderer(buffer)]
pub fn render_error_fs_ops_failed(error: ErrorFsOpsFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("fs_ops.err_failed_help").trim()));
    ec.exit_code = EC_ERR_FS_OPS_FAILED;
}

/// Puts a path at another, whichever of the two is a directory.
///
/// A rename the filesystem refuses — across two devices, most often — is still a move to a reader, so
/// it is done the long way instead: copied, and then removed. Nothing is asked and nothing is
/// confirmed: a name already taken at the destination is what the destination being there means, and
/// whether it should have been replaced was settled before this was called.
///
/// # Errors
///
/// The error the filesystem gave, when the path could not be read, copied, written or removed.
pub fn move_to(from: &Path, to: &Path) -> io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }

    copy_to(from, to)?;
    remove_at(from)
}

/// Copies a path to another, a directory and everything under it included.
///
/// # Errors
///
/// The error the filesystem gave, when the path could not be read, or a directory under it could not
/// be made or a file under it could not be written.
pub fn copy_to(from: &Path, to: &Path) -> io::Result<()> {
    if std::fs::metadata(from)?.is_dir() {
        copy_tree(from, to)
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Removes a path, a directory and everything under it included.
///
/// The kind of the path is read from the link itself rather than from what it points at, so that a
/// symbolic link is unlinked and not followed: removing a link is removing the link, and walking
/// through it would remove whatever it names.
///
/// # Errors
///
/// The error the filesystem gave, when the path could not be read or removed.
pub fn remove_at(path: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(path)?.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// Copies a directory and everything under it.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;

    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let under = to.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &under)?;
        } else {
            std::fs::copy(entry.path(), under)?;
        }
    }

    Ok(())
}

/// The Layout a run in a Workspace works in, when it is in one.
///
/// It is the Workspace's own Layout and never a Vault's: a filesystem operation is about the tree, so a
/// run on a Vault — or one whose paths are in no Workspace — has nothing here to keep in step and is
/// left to the filesystem alone.
pub fn working(held: &Workspace) -> Option<Layout> {
    let layouts = held.layouts();
    let name = layouts.current().ok().flatten()?;

    layouts.get(&name).ok().flatten()
}

/// The Workspace `disk` sits in, when it sits in one.
///
/// It is looked for from the path itself and never from where the run was made: a filesystem operation
/// names the paths it works on, and those are what say which tree — if any — it is about. A run made
/// outside every Workspace, or inside another one, still keeps in step the tree its paths are in.
pub fn workspace_of(cwd: &Path, disk: &Path) -> Option<Workspace> {
    locate_workspace(&normalize(&resolve(cwd, disk)))
}

/// The Layout paths `disk` is or holds, in the Layout's own order.
///
/// `disk` is read where the run was made, the way `rola track` reads a name; what a Layout names is
/// relative to the Workspace root, so one becomes the other here. Nothing is named for a path outside
/// the root — no Layout names it — and a directory names every path under it, itself included, the
/// Workspace root naming the whole of the work.
pub fn touched(layout: &Layout, held: &Workspace, cwd: &Path, disk: &Path) -> Vec<LayoutPath> {
    let root = normalize(held.get_root());
    let absolute = normalize(&resolve(cwd, disk));

    let Ok(relative) = absolute.strip_prefix(&root) else {
        return Vec::new();
    };

    if absolute.is_dir() {
        let prefix = prefix(relative);

        return layout
            .paths()
            .into_iter()
            .map(|(path, _)| path)
            .filter(|path| under(&prefix, path))
            .collect();
    }

    LayoutPath::from_relative(relative)
        .ok()
        .into_iter()
        .collect()
}

/// Where `disk` sits in the Layout, or nothing when it sits outside the Workspace.
pub fn inside(held: &Workspace, cwd: &Path, disk: &Path) -> Option<String> {
    let root = normalize(held.get_root());
    let absolute = normalize(&resolve(cwd, disk));

    absolute.strip_prefix(&root).ok().map(prefix)
}

/// The `Uuid` each of `paths` names, for the paths that name one.
pub fn named(layout: &Layout, paths: &[LayoutPath]) -> Vec<(LayoutPath, Uuid)> {
    paths
        .iter()
        .filter_map(|path| layout.id_of(path).map(|id| (path.clone(), id)))
        .collect()
}

/// The path `moving` has after a move from `from` to `to`, both as Layout paths.
///
/// A move of a directory moves everything under it the same way: what sits below the source keeps its
/// place below the destination.
pub fn moved(from: &str, to: &str, moving: &LayoutPath) -> LayoutPath {
    let rest = moving
        .as_str()
        .strip_prefix(from)
        .map_or("", |rest| rest.trim_start_matches('/'));

    let text = if rest.is_empty() {
        to.to_owned()
    } else if to.is_empty() {
        rest.to_owned()
    } else {
        format!("{to}/{rest}")
    };

    // Joining two names a `LayoutPath` already accepted makes one it accepts, so the fallback is a
    // name that could not be read rather than a refusal the caller would have to carry.
    LayoutPath::new(&text).unwrap_or_else(|_| moving.clone())
}

/// The account that holds `id` upstream, when that is not the one this run acts as.
///
/// Ownership is the Vault's, and what answers is the fetched copy — the same reading `rola track`
/// makes before it records. A Layout that tracks no Vault, a copy nobody fetched, and a run with no
/// account bound are each "no one is in the way" rather than a reason to refuse.
pub fn held_elsewhere(
    held: &Workspace,
    layout: &Layout,
    id: Uuid,
    me: Option<&str>,
) -> Option<String> {
    let me = me?;
    let vault = crate::ownership::tracked_vault(held, layout)?;
    let dir = crate::layout::readonly_layout_dir(held, &vault, VAULT_LAYOUT_NAME);
    let copy = Layout::open(&dir).ok()?;
    let owner = copy.entry(id)?.owner()?.to_owned();

    (owner != me).then_some(owner)
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve(cwd: &Path, given: &Path) -> PathBuf {
    if given.is_absolute() {
        given.to_path_buf()
    } else {
        cwd.join(given)
    }
}

/// The prefix the paths under `relative` share, or `""` for the Workspace root itself.
fn prefix(relative: &Path) -> String {
    LayoutPath::from_relative(relative)
        .map_or_else(|_| String::new(), |path| path.as_str().to_owned())
}

/// Whether `path` is `prefix`'s path or under it, with `""` naming the whole tree.
fn under(prefix: &str, path: &LayoutPath) -> bool {
    if prefix.is_empty() {
        return true;
    }

    path.as_str() == prefix
        || path
            .as_str()
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Error: a filesystem operation was stopped by what the Layout names.
#[derive(Grouped)]
pub struct ErrorFsOpsBlocked {
    /// The path the operation was about.
    path: String,
    /// Why it was stopped.
    kind: FsOpsBlock,
}

/// The ways the Layout stops a filesystem operation.
pub enum FsOpsBlock {
    /// Another account holds the entry upstream.
    Held(String),
    /// The destination already names something in this Layout.
    Named,
    /// A variant file would be moved to where no record can say it lies.
    Variant,
}

impl Failure for ErrorFsOpsBlocked {
    fn name(&self) -> &'static str {
        "error_fs_ops_blocked"
    }

    fn reason(&self) -> String {
        let said = match &self.kind {
            FsOpsBlock::Held(owner) => {
                t!("fs_ops.err_blocked_held", path = self.path, owner = owner)
            }
            FsOpsBlock::Named => t!("fs_ops.err_blocked_named", path = self.path),
            FsOpsBlock::Variant => t!("fs_ops.err_blocked_variant", path = self.path),
        };

        said.trim().to_string()
    }
}

failure!(ErrorFsOpsBlocked);

#[renderer(buffer)]
pub fn render_error_fs_ops_blocked(error: ErrorFsOpsBlocked, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("fs_ops.err_blocked_help").trim()));
    ec.exit_code = EC_ERR_FS_OPS_FAILED;
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use librorolala::layout::LayoutPath;

    use super::{copy_to, move_to, moved, remove_at, under};

    /// A directory of this test's own, emptied first, under the temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rola-fs-ops-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");

        dir
    }

    /// A path a Layout would name.
    fn path(text: &str) -> LayoutPath {
        LayoutPath::new(text).expect("a path a Layout could name")
    }

    #[test]
    fn a_move_takes_everything_under_it_to_the_same_place() {
        assert_eq!(moved("d", "e", &path("d/x.txt")).as_str(), "e/x.txt");
        assert_eq!(moved("d", "e", &path("d")).as_str(), "e");
        assert_eq!(
            moved("d/sub", "e", &path("d/sub/y.txt")).as_str(),
            "e/y.txt"
        );
        assert_eq!(moved("d", "", &path("d/x.txt")).as_str(), "x.txt");
    }

    #[test]
    fn what_a_path_is_or_holds_is_named_by_the_prefix_it_shares() {
        assert!(under("", &path("a/b.txt")));
        assert!(under("d", &path("d")));
        assert!(under("d", &path("d/x.txt")));
        assert!(!under("d", &path("dx.txt")));
        assert!(!under("d", &path("e/x.txt")));
    }

    #[test]
    fn a_file_is_moved_under_its_new_name() {
        let dir = scratch("move");
        let from = dir.join("one.txt");
        let to = dir.join("two.txt");
        fs::write(&from, b"hello").expect("a file");

        move_to(&from, &to).expect("the move");

        assert!(!from.exists());
        assert_eq!(fs::read(&to).expect("the moved file"), b"hello");
    }

    #[test]
    fn a_directory_is_copied_with_everything_under_it() {
        let dir = scratch("copy");
        let from = dir.join("a");
        fs::create_dir_all(from.join("b")).expect("a tree");
        fs::write(from.join("b/c.txt"), b"deep").expect("a file");

        copy_to(&from, &dir.join("copy")).expect("the copy");

        assert_eq!(
            fs::read(dir.join("copy/b/c.txt")).expect("the copied file"),
            b"deep"
        );
        assert!(from.join("b/c.txt").exists(), "the original is left alone");
    }

    #[test]
    fn a_directory_is_removed_with_everything_under_it() {
        let dir = scratch("remove");
        fs::create_dir_all(dir.join("a/b")).expect("a tree");
        fs::write(dir.join("a/b/c.txt"), b"deep").expect("a file");

        remove_at(&dir.join("a")).expect("the removal");

        assert!(!dir.join("a").exists());
    }

    #[test]
    fn an_operation_on_a_path_that_is_not_there_says_so() {
        let dir = scratch("missing");

        assert!(move_to(&dir.join("nothing"), &dir.join("elsewhere")).is_err());
        assert!(copy_to(&dir.join("nothing"), &dir.join("elsewhere")).is_err());
        assert!(remove_at(&dir.join("nothing")).is_err());
    }
}
