//! The commands that keep a project's git support: its files, its materials, and the hooks.

// mingling's `#[command]` expands the entry type and the dispatch function beside the command, and
// the expansion is not documented; the items this module writes carry their own docs.
#![allow(missing_docs)]
// A command's words arrive as the value the framework's entry converts to, so the parameter is the
// framework's shape rather than this module's choice: taking them by reference is not an option the
// generated wrapper offers.
#![allow(clippy::needless_pass_by_value)]

use std::fs;
use std::path::{Path, PathBuf};

use librorolala::tree::EntryKind as TreeKind;
use mingling::{
    Grouped,
    macros::{buffer, chain, command, r_eprintln, r_println, renderer},
    res::ResExitCode,
};
use rola_define_format::{ContentHash, Lock, LockPath, MaterialKind};
use rola_git_support::{
    ContentSource, Error as GitError, Project, Repository, Session, structure as read_structure,
};
use rorolala_utils_location::normalize;
use rust_i18n::t;

use crate::Next;
use crate::Refused;
use crate::exit_codes::EC_FAILED;
use crate::options::operands;

/// The events a project's materials are kept in step after.
const EVENTS: [&str; 3] = ["post-checkout", "post-merge", "post-rewrite"];

/// The line that says a hook file is the one this program installed.
const MARKER: &str = "git-rola hook";

/// The file `init` writes when the project has none.
///
/// The `[vaults]` line is a starting address a person replaces with the Vault theirs materials come
/// from; one is named rather than left empty so that a `update` after a mapping is added has a Vault
/// to resolve through without the file having to be understood first.
const TEMPLATE: &str = r#"# List of assets imported from a RorolalaVCS Vault

[features]
git_support = true

[vaults]
# origin = "rola://server_address/vault_name"
origin = "rola://127.0.0.1/default"

# [[mapping]]
# from = "00000000-0000-0000-0000-000000000000"
# to = "/Path/To/Asset.ext"
# pin = "latest"
"#;

/// Sets up git support in the project the run is inside
///
/// The project file is written when there is none, the hooks that keep the materials in step are
/// installed, and a project that already has a lock has its materials put in place.
#[command(node = "init", entry = EntryInit)]
pub fn init(words: Vec<String>) -> Next {
    if !operands(&words).is_empty() {
        return Refused { command: "init" }.into();
    }

    StateInit.into()
}

/// The state init starts in: the command names nothing.
#[derive(Grouped)]
pub struct StateInit;

/// Sets the project up.
#[chain]
pub fn handle_init(_state: StateInit) -> Next {
    match initialise() {
        Ok(result) => result.into(),
        Err(message) => crate::Fatal::new(message).into(),
    }
}

/// Result: a project was set up, and what could not be done on the way.
#[derive(Grouped)]
pub struct ResultInit {
    /// The line git-rola says about the project it set up.
    pub message: String,
    /// Whether something the set-up was to write could not be written.
    pub failed: bool,
}

/// Renders the line about the project that was set up, and ends a partial set-up with git's 1.
#[renderer(buffer)]
pub fn render_init(result: ResultInit, ec: &mut ResExitCode) {
    r_println!("{}", result.message);

    if result.failed {
        ec.exit_code = EC_FAILED;
    }
}

/// Writes the project file and the hooks, and puts a lock's materials in place.
fn initialise() -> Result<ResultInit, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| t!("git_rola.err_cwd", error = error.to_string()).to_string())?;
    let root =
        Project::locate(&cwd).map_or_else(|| cwd.clone(), |project| project.root().to_path_buf());

    // A linked work tree is refused before anything is written, so a run that cannot act does not
    // leave a project file behind in a work tree this build leaves alone.
    if Repository::discover(&root)
        .map_err(|error| error.to_string())?
        .is_some_and(|repository| repository.is_linked_work_tree())
    {
        return Err(GitError::LinkedWorkTree.to_string());
    }

    let config = root.join(Project::CONFIG_FILE);
    let existed = config.is_file();

    if !existed {
        fs::write(&config, TEMPLATE).map_err(|error| {
            t!(
                "init.err_config_write",
                path = config.display().to_string(),
                error = error.to_string()
            )
            .to_string()
        })?;
    }

    let session = Session::open(&root)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| t!("init.err_open").to_string())?;

    let mut failed = !install_hooks(session.repository().git_dir())?;

    if session.project().lock_path().is_file() {
        let lock = session.lock().map_err(|error| error.to_string())?;
        let source = crate::source::of(session.project())?;
        let failures = session
            .attach(&lock, &source)
            .map_err(|error| error.to_string())?;
        session.exclude(&lock).map_err(|error| error.to_string())?;

        for (path, error) in &failures {
            eprintln!(
                "{}{}",
                t!("message.error"),
                t!(
                    "message.failed_path",
                    path = path.as_str(),
                    error = error.to_string()
                )
            );
            failed = true;
        }
    }

    let root = root.display();
    let message = if existed {
        t!("init.reinitialized", path = root.to_string())
    } else {
        t!("init.initialized", path = root.to_string())
    };

    Ok(ResultInit {
        message: message.to_string(),
        failed,
    })
}

/// Keeps a project's materials in step after a git operation changed the work tree
///
/// This is what the installed hooks run. The first word is the event, and the words after it are
/// git's own arguments to that hook, which a hook does not read and so are passed over. A
/// repository that holds no project is not a failure: the hook was installed here, but the project
/// may have been removed since.
#[command(node = "hook", entry = EntryHook)]
pub fn hook(words: Vec<String>) -> Next {
    let named = operands(&words);

    let Some(event) = named.first() else {
        return Refused { command: "hook" }.into();
    };

    if !EVENTS.contains(&event.as_str()) {
        return Refused { command: "hook" }.into();
    }

    StateHook {
        event: event.clone(),
    }
    .into()
}

/// The state a hook run starts in: the event it is run for.
#[derive(Grouped)]
pub struct StateHook {
    /// The hook event, as git names it.
    pub event: String,
}

/// Keeps the materials in step after `event`.
#[chain]
pub fn handle_hook(state: StateHook) -> Next {
    let event = state.event;
    let Ok(cwd) = std::env::current_dir() else {
        return crate::settle::ResultSettled {
            failures: Vec::new(),
        }
        .into();
    };

    let session = match Session::open(&cwd) {
        Ok(Some(session)) => session,
        // A hook cannot abandon the operation git has already made, and its failure is the whole
        // command's failure for some of git's commands (`git worktree add`), so a work tree this
        // build does not act in is said and left where it is.
        Err(error @ GitError::LinkedWorkTree) => {
            eprintln!("{}{error}", t!("message.warning"));

            return crate::settle::ResultSettled {
                failures: Vec::new(),
            }
            .into();
        }
        // A project there is nothing to do for is not a failure of the hook: the hook was installed
        // here, but the project may have been removed since, and a project no command could read is
        // the commands' to report.
        Ok(None) | Err(_) => {
            return crate::settle::ResultSettled {
                failures: Vec::new(),
            }
            .into();
        }
    };

    // What a hook does after git changed the tree and what `refresh` does when a person asks for it
    // are the same run, so both go through the one that applies the lock.
    match crate::settle::apply(&session) {
        Ok(result) => result.into(),
        Err(message) => crate::Fatal::new(
            t!("hook.err_settle", event = event.as_str(), error = message).to_string(),
        )
        .into(),
    }
}

/// Puts materials back as the lock names them
///
/// A name is read the way git reads a pathspec: a path relative to the run's directory, a directory
/// for everything under it, `.` for everything under the current directory, and git's wildcards
/// under the variables that shape them. A name that selects nothing is refused before anything is
/// written, so a run that mistypes one restores nothing.
///
/// A tree is one material, and its own path selects the whole tree, which is written back as the
/// structure the lock names. A name inside a tree selects only what it names there: a file of the
/// tree is put back on its own, and a directory of it takes the files under it. A wildcard inside
/// a tree still selects the whole tree.
#[command(node = "restore", entry = EntryRestore)]
pub fn restore(words: Vec<String>) -> Next {
    let names = operands(&words);

    // git reads a restore with no name as a command that cannot be made, not as a line it could not
    // read: `fatal: you must specify path(s) to restore`, 128.
    if names.is_empty() {
        return crate::Fatal::new(t!("restore.err_no_path").to_string()).into();
    }

    StateRestore { names }.into()
}

/// The state a restore starts in: the names it was given.
#[derive(Grouped)]
pub struct StateRestore {
    /// The paths to put back, as the run named them.
    pub names: Vec<String>,
}

/// Puts the named materials back.
#[chain]
pub fn handle_restore(state: StateRestore) -> Next {
    let StateRestore { names } = state;

    match restore_materials(&names) {
        Ok(result) => result.into(),
        Err(RestoreFailure::Pathspec(refusal)) => crate::PathspecRefused { refusal }.into(),
        Err(RestoreFailure::Fatal(message)) => crate::Fatal::new(message).into(),
    }
}

/// Why a restore could not put anything back.
enum RestoreFailure {
    /// A name that selected nothing, which git answers as a pathspec with status 1.
    Pathspec(crate::pathspec::Refusal),
    /// The project or its lock could not be read, which git answers as `fatal:`.
    Fatal(String),
}

/// Result: what a restore could not put back.
#[derive(Grouped)]
pub struct ResultRestore {
    /// Each material that could not be put back, and why.
    pub failures: Vec<(String, String)>,
}

/// Renders what a restore could not put back, and ends the run on the first of them with git's 1.
#[renderer(buffer)]
pub fn render_restore(result: ResultRestore, ec: &mut ResExitCode) {
    for (path, error) in &result.failures {
        r_eprintln!(
            "{}{}",
            t!("message.error"),
            t!(
                "message.failed_path",
                path = path.as_str(),
                error = error.as_str()
            )
        );
    }

    if !result.failures.is_empty() {
        ec.exit_code = EC_FAILED;
    }
}

/// Reads the project and puts the materials the names select back from the lock.
fn restore_materials(names: &[String]) -> Result<ResultRestore, RestoreFailure> {
    let cwd = std::env::current_dir().map_err(|error| {
        RestoreFailure::Fatal(t!("git_rola.err_cwd", error = error.to_string()).to_string())
    })?;

    let Some(session) =
        Session::open(&cwd).map_err(|error| RestoreFailure::Fatal(error.to_string()))?
    else {
        return Err(RestoreFailure::Fatal(
            t!("git_rola.err_no_project").to_string(),
        ));
    };

    let lock = session.lock().map_err(|error| {
        RestoreFailure::Fatal(t!("git_rola.err_lock", error = error.to_string()).to_string())
    })?;

    // What a name means is read from where the run is: the directory it is made in, relative to the
    // project, is what a name is joined to.
    let prefix = session
        .name()
        .map_err(|error| RestoreFailure::Fatal(error.to_string()))?;
    let here = session
        .repository()
        .relative(&cwd)
        .map_err(|error| RestoreFailure::Fatal(error.to_string()))?;
    let base: PathBuf = if prefix == "." {
        here
    } else {
        let project: PathBuf = prefix.split('/').collect();

        here.strip_prefix(&project)
            .map_err(|_| {
                RestoreFailure::Fatal(
                    t!("restore.err_outside", name = cwd.display().to_string()).to_string(),
                )
            })?
            .to_path_buf()
    };

    let names = inside_names(&lock, &base, names);

    let only = crate::pathspec::select(&lock, &base, &prefix, &names.names)
        .map_err(RestoreFailure::Pathspec)?;

    let source = crate::source::of(session.project()).map_err(RestoreFailure::Fatal)?;

    // What a name inside a tree restores is put back here, not by session.restore: an empty list
    // of materials there means every material, and a run of names that all point inside a tree has
    // none of its own to select.
    let mut failures: Vec<(String, String)> = if only.materials.is_empty() {
        Vec::new()
    } else {
        session
            .restore(&lock, &only.materials, &source)
            .map_err(|error| RestoreFailure::Fatal(error.to_string()))?
            .into_iter()
            .map(|(path, error)| (path, error.to_string()))
            .collect()
    };

    failures.extend(restore_inside(
        &lock,
        &only.materials,
        &source,
        session.project(),
        &names.inside,
    ));

    Ok(ResultRestore { failures })
}

/// The names split into the materials they select and the places they name inside a tree.
struct Names {
    /// The names a material is selected whole by.
    names: Vec<String>,
    /// The places inside a tree, by the tree's path and the path inside it.
    inside: Vec<(LockPath, String)>,
}

/// Splits the literal names that point inside a tree from the rest.
///
/// A tree is one material, so a name at or above it selects the whole tree, which is `pathspec`'s
/// to answer. A literal name strictly inside one is this run's to put back on its own, so it is
/// taken out of what `pathspec` sees; a wildcard is left there, where it still means the tree.
fn inside_names(lock: &Lock, base: &Path, names: &[String]) -> Names {
    let trees: Vec<&str> = lock
        .materials()
        .iter()
        .filter(|material| material.kind() == MaterialKind::Tree)
        .map(|material| material.path().as_str())
        .collect();
    let mut kept = Vec::with_capacity(names.len());
    let mut inside = Vec::new();

    for name in names {
        if name.contains(['*', '?', '[']) {
            kept.push(name.clone());

            continue;
        }

        let joined = normalize(&base.join(name));
        let text = joined.to_string_lossy().replace('\\', "/");
        let text = text.trim_start_matches('/');
        let mut found = false;

        for tree in &trees {
            if let Some(rest) = text
                .strip_prefix(tree)
                .and_then(|rest| rest.strip_prefix('/'))
                && !rest.is_empty()
                && let Ok(path) = LockPath::new(tree)
            {
                inside.push((path, rest.to_owned()));
                found = true;

                break;
            }
        }

        if !found {
            kept.push(name.clone());
        }
    }

    Names {
        names: kept,
        inside,
    }
}

/// Puts back the places named inside a tree, a file of it or the files under a directory of it.
///
/// A tree the run also selected whole is skipped: the whole restore writes every path in it, and a
/// second write would be the same bytes written twice and a failure reported twice.
fn restore_inside(
    lock: &Lock,
    whole: &[LockPath],
    source: &crate::source::Source,
    project: &Project,
    inside: &[(LockPath, String)],
) -> Vec<(String, String)> {
    let mut failures = Vec::new();

    for (tree, rest) in inside {
        if whole.contains(tree) {
            continue;
        }

        let Some(material) = lock
            .materials()
            .iter()
            .find(|material| material.path() == tree)
        else {
            continue;
        };

        let structure = match read_structure(source, material.content()) {
            Ok(structure) => structure,
            Err(error) => {
                failures.push((tree.as_str().to_owned(), error.to_string()));

                continue;
            }
        };

        for entry in structure.entries() {
            if entry.kind() != TreeKind::File {
                continue;
            }

            let path = entry.path();
            let under = path
                .strip_prefix(rest.as_str())
                .is_some_and(|tail| tail.starts_with('/'));

            if path != rest.as_str() && !under {
                continue;
            }

            let Some(storage) = entry.storage() else {
                continue;
            };

            let full = format!("{}/{path}", tree.as_str());
            let disk = project.root().join(full.split('/').collect::<PathBuf>());

            if let Some(parent) = disk.parent()
                && let Err(error) = fs::create_dir_all(parent)
            {
                failures.push((full, error.to_string()));

                continue;
            }

            let content = ContentHash::from_digest(*storage.digest());

            if let Err(error) = source.write(content, &disk) {
                failures.push((full, error.to_string()));
            }
        }
    }

    failures
}

/// Updates `rola.lock` and puts the project's materials in step with it
///
/// Each mapping is resolved through the Vaults the project declares: the version its pin names, and
/// the content that version points at, brought into the project's store. Nothing is written unless
/// every mapping resolves, and what was placed before is taken out before the new lock is applied.
#[command(node = "update", entry = EntryUpdate)]
pub fn update(words: Vec<String>) -> Next {
    if !operands(&words).is_empty() {
        return Refused { command: "update" }.into();
    }

    StateUpdate.into()
}

/// The state update starts in: the command names nothing.
#[derive(Grouped)]
pub struct StateUpdate;

/// Resolves the project's mappings, writes the lock they come to, and places what it names.
#[chain]
pub fn handle_update(_state: StateUpdate) -> Next {
    match crate::update::run() {
        Ok(outcome) => ResultUpdated {
            materials: outcome.materials,
            failures: outcome.failures,
        }
        .into(),
        Err(message) => crate::Fatal::new(message).into(),
    }
}

/// Result: what an update came to.
#[derive(Grouped)]
pub struct ResultUpdated {
    /// How many materials the new lock places.
    pub materials: usize,
    /// The materials that could not be placed, each with why.
    pub failures: Vec<(String, String)>,
}

/// Says what the lock now holds, and reports what could not be placed.
#[renderer(buffer)]
pub fn render_updated(result: ResultUpdated, ec: &mut ResExitCode) {
    for (path, error) in &result.failures {
        r_eprintln!(
            "{}{}",
            t!("message.error"),
            t!(
                "message.failed_path",
                path = path.as_str(),
                error = error.as_str()
            )
        );
    }

    if result.failures.is_empty() {
        r_println!("{}", t!("update.updated", materials = result.materials));
    } else {
        ec.exit_code = EC_FAILED;
    }
}

/// Prints the program's usage, or one command's
///
/// This is git's `git help` with the documentation viewer left out: the same text a `-h` gives is
/// written here, so a run that wants to read about a command gets it without a man page or a
/// browser. A word after the command name is ignored, the way `git help status extra` ignores it.
#[command(node = "help", entry = EntryHelp)]
pub fn help(words: Vec<String>) -> ResultHelp {
    let command = operands(&words)
        .first()
        .and_then(|word| crate::usage::named(word));

    ResultHelp { command }
}

/// Result: which usage was asked for.
#[derive(Grouped)]
pub struct ResultHelp {
    /// The command whose usage was asked for, or nothing for the program's own.
    pub command: Option<&'static str>,
}

/// Renders the usage that was asked for, on stdout and with git's status.
#[renderer(buffer)]
pub fn render_help(result: ResultHelp) {
    match result.command {
        Some(command) => r_println!("{}", crate::usage::command(command)),
        None => r_println!("{}", crate::usage::program()),
    }
}

/// Prints the version of this program
///
/// git answers `git version` as `git version X` whatever follows it, so this is the same line under
/// this program's name, and a word after it is ignored.
#[command(node = "version", entry = EntryVersion)]
pub fn version() -> ResultVersion {
    ResultVersion
}

/// Result: the version line is all there is to say.
#[derive(Grouped)]
pub struct ResultVersion;

/// Renders the version line, the way `git version` writes one.
#[renderer(buffer)]
pub fn render_version(_: ResultVersion) {
    r_println!(
        "{}",
        t!("git_rola.version", version = env!("CARGO_PKG_VERSION"))
    );
}

/// Installs the hooks a project needs, leaving any hook a person put there alone.
///
/// Answers whether every hook this program was to write is now written. A hook that belongs to
/// someone else is left where it is and is not counted against the run; one that could not be
/// written is, since the set-up is then only part done.
fn install_hooks(git_dir: &Path) -> Result<bool, String> {
    let hooks = git_dir.join("hooks");

    fs::create_dir_all(&hooks).map_err(|error| {
        t!(
            "init.err_hooks_make",
            path = hooks.display().to_string(),
            error = error.to_string()
        )
        .to_string()
    })?;

    let mut every_written = true;

    for event in EVENTS {
        let path = hooks.join(event);

        if path.exists() {
            let ours = fs::read_to_string(&path).is_ok_and(|text| text.contains(MARKER));

            if !ours {
                let said = t!("init.err_hook_exists", event = event).to_string();
                eprintln!("{}{said}", t!("message.warning"));

                continue;
            }
        }

        let script = format!(
            "#!/bin/sh\n# Installed by `git rola init`. {MARKER}: remove this file to stop it.\nexec git rola hook {event} \"$@\"\n"
        );

        if let Err(error) = fs::write(&path, script) {
            let said = t!(
                "init.err_hook_write",
                path = path.display().to_string(),
                error = error.to_string()
            )
            .to_string();
            eprintln!("{}{said}", t!("message.error"));

            every_written = false;
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;

                // A hook that cannot be made executable is one git will not run, but the file is
                // still the one this program wrote: what failed is said on the next run.
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
            }
        }
    }

    Ok(every_written)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rola_define_format::{ContentHash, Lock, LockPath, Material, MaterialKind};

    use super::inside_names;

    /// A lock naming a tree and a file beside it.
    fn held() -> Lock {
        let materials = vec![
            Material::new(
                LockPath::new("Tree").unwrap(),
                ContentHash::from_digest([1; 32]),
                MaterialKind::Tree,
            ),
            Material::new(
                LockPath::new("Other/x").unwrap(),
                ContentHash::from_digest([2; 32]),
                MaterialKind::File,
            ),
        ];

        Lock::new(vec![], materials).unwrap()
    }

    #[test]
    fn a_name_inside_a_tree_is_split_from_the_rest() {
        let names: Vec<String> = ["Tree/a.txt", "Tree", "Other/x", "Tree/*"]
            .into_iter()
            .map(str::to_owned)
            .collect();

        let split = inside_names(&held(), Path::new(""), &names);

        assert_eq!(split.names, ["Tree", "Other/x", "Tree/*"]);
        assert_eq!(
            split.inside,
            [(LockPath::new("Tree").unwrap(), "a.txt".to_owned())]
        );
    }
}
