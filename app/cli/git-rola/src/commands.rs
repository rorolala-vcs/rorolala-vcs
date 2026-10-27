//! The `init` and `hook` commands: a project's git support set up, and kept in step afterwards.

// mingling's `#[command]` expands the entry type and the dispatch function beside the command, and
// the expansion is not documented; the items this module writes carry their own docs.
#![allow(missing_docs)]

use std::fs;

use mingling::{
    Grouped,
    consts::UNMATCHED,
    macros::{arg, buffer, chain, command, r_eprintln, r_println, renderer},
    picker::EntryPicker,
    res::ResExitCode,
};
use rola_git_support::{Error as GitError, Project, Repository, Session};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_ERR;

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
pub fn init() -> StateInit {
    StateInit
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

/// Result: a project was set up.
#[derive(Grouped)]
pub struct ResultInit {
    /// The line git-rola says about the project it set up.
    pub message: String,
}

/// Renders the line about the project that was set up.
#[renderer(buffer)]
pub fn render_init(result: ResultInit) {
    r_println!("{}", result.message);
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

    install_hooks(session.repository().git_dir())?;

    if session.project().lock_path().is_file() {
        let lock = session.lock().map_err(|error| error.to_string())?;
        let source = crate::source::of(session.project());
        let failures = session
            .attach(&lock, &source)
            .map_err(|error| error.to_string())?;
        session.exclude(&lock).map_err(|error| error.to_string())?;

        for (path, error) in &failures {
            eprintln!("git-rola: {path}: {error}");
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
    })
}

/// Keeps a project's materials in step after a git operation changed the work tree
///
/// This is what the installed hooks run. A repository that holds no project is not a failure: the
/// hook was installed here, but the project may have been removed since.
#[command(node = "hook", entry = EntryHook)]
pub fn hook(args: EntryHook) -> Next {
    let picked = args.pick(arg![Vec<String>]).pick(&UNMATCHED).to_result();

    let (words, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if crate::refused(&unmatched) {
        return crate::usage();
    }

    match words.first().map(String::as_str) {
        Some(event) if EVENTS.contains(&event) => StateHook {
            event: event.to_owned(),
        }
        .into(),
        _ => crate::usage(),
    }
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
        return ResultHook { placed: true }.into();
    };

    let session = match Session::open(&cwd) {
        Ok(Some(session)) => session,
        // A hook cannot abandon the operation git has already made, and its failure is the whole
        // command's failure for some of git's commands (`git worktree add`), so a work tree this
        // build does not act in is said and left where it is.
        Err(error @ GitError::LinkedWorkTree) => {
            eprintln!("git-rola: {error}");

            return ResultHook { placed: true }.into();
        }
        // A project there is nothing to do for is not a failure of the hook: the hook was installed
        // here, but the project may have been removed since, and a project no command could read is
        // the commands' to report.
        Ok(None) | Err(_) => return ResultHook { placed: true }.into(),
    };

    let stored = match session.lock() {
        Ok(lock) => lock,
        Err(error) => {
            return crate::Fatal::new(
                t!("git_rola.err_lock", error = error.to_string()).to_string(),
            )
            .into();
        }
    };

    let source = crate::source::of(session.project());

    match session.settle(&stored, &source) {
        Ok(failures) if failures.is_empty() => ResultHook { placed: true }.into(),
        Ok(failures) => {
            for (path, error) in &failures {
                eprintln!("git-rola: {path}: {error}");
            }

            ResultHook { placed: false }.into()
        }
        Err(error) => crate::Fatal::new(
            t!(
                "hook.err_settle",
                event = event.as_str(),
                error = error.to_string()
            )
            .to_string(),
        )
        .into(),
    }
}

/// Result: what a hook run managed.
#[derive(Grouped)]
pub struct ResultHook {
    /// Whether every material the lock names was placed.
    pub placed: bool,
}

/// Renders nothing on success, and ends a run that could not place everything with an error.
#[renderer(buffer)]
pub fn render_hook(result: ResultHook, ec: &mut ResExitCode) {
    if !result.placed {
        ec.exit_code = EC_ERR;
    }
}

/// Puts materials back as the lock names them
///
/// A name is read the way git reads a pathspec: a path relative to the run's directory, a directory
/// for everything under it, and `.` for everything under the current directory. A name that selects
/// nothing is refused before anything is written, so a run that mistypes one restores nothing.
#[command(node = "restore", entry = EntryRestore)]
pub fn restore(args: EntryRestore) -> Next {
    let picked = args.pick(arg![Vec<String>]).pick(&UNMATCHED).to_result();

    let (names, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if crate::refused(&unmatched) || names.is_empty() {
        return crate::usage();
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
        Err(message) => crate::Fatal::new(message).into(),
    }
}

/// Result: what a restore could not put back.
#[derive(Grouped)]
pub struct ResultRestore {
    /// Each material that could not be put back, and why.
    pub failures: Vec<(String, String)>,
}

/// Renders what a restore could not put back, and ends the run with an error when anything is left.
#[renderer(buffer)]
pub fn render_restore(result: ResultRestore, ec: &mut ResExitCode) {
    for (path, error) in &result.failures {
        r_eprintln!("git-rola: {path}: {error}");
    }

    if !result.failures.is_empty() {
        ec.exit_code = EC_ERR;
    }
}

/// Reads the project and puts the materials the names select back from the lock.
fn restore_materials(names: &[String]) -> Result<ResultRestore, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| t!("git_rola.err_cwd", error = error.to_string()).to_string())?;

    let Some(session) = Session::open(&cwd).map_err(|error| error.to_string())? else {
        return Err(t!("git_rola.err_no_project").to_string());
    };

    let lock = session
        .lock()
        .map_err(|error| t!("git_rola.err_lock", error = error.to_string()).to_string())?;

    // What a name means is read from where the run is: the directory it is made in, relative to the
    // project, is what a name is joined to.
    let prefix = session.name().map_err(|error| error.to_string())?;
    let here = session
        .repository()
        .relative(&cwd)
        .map_err(|error| error.to_string())?;
    let base: std::path::PathBuf = if prefix == "." {
        here
    } else {
        let project: std::path::PathBuf = prefix.split('/').collect();

        here.strip_prefix(&project)
            .map_err(|_| t!("restore.err_outside", name = cwd.display().to_string()).to_string())?
            .to_path_buf()
    };

    let only = crate::pathspec::select(&lock, &base, &prefix, names)
        .map_err(|refusal| refusal_message(&refusal))?;

    let source = crate::source::of(session.project());
    let failures = session
        .restore(&lock, &only.materials, &source)
        .map_err(|error| error.to_string())?;

    Ok(ResultRestore {
        failures: failures
            .into_iter()
            .map(|(path, error)| (path, error.to_string()))
            .collect(),
    })
}

/// A name that selected nothing, said the way the program says it.
fn refusal_message(refusal: &crate::pathspec::Refusal) -> String {
    use crate::pathspec::Refusal;

    match refusal {
        Refusal::NoMatch(name) => t!("restore.err_not_in_lock", name = name.as_str()),
        Refusal::Outside(name) => t!("restore.err_outside", name = name.as_str()),
        Refusal::Invalid(name) => t!("restore.err_path", name = name.as_str()),
    }
    .to_string()
}

/// Updates `rola.lock` and puts the project's materials in step with it
///
/// Each mapping is resolved through the Vaults the project declares: the version its pin names, and
/// the content that version points at, brought into the project's store. Nothing is written unless
/// every mapping resolves, and what was placed before is taken out before the new lock is applied.
#[command(node = "update", entry = EntryUpdate)]
pub fn update() -> StateUpdate {
    StateUpdate
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
        r_eprintln!("git-rola: {path}: {error}");
    }

    if result.failures.is_empty() {
        r_println!("{}", t!("update.updated", materials = result.materials));
    } else {
        ec.exit_code = EC_ERR;
    }
}

/// Installs the hooks a project needs, leaving any hook a person put there alone.
fn install_hooks(git_dir: &std::path::Path) -> Result<(), String> {
    let hooks = git_dir.join("hooks");

    fs::create_dir_all(&hooks).map_err(|error| {
        t!(
            "init.err_hooks_make",
            path = hooks.display().to_string(),
            error = error.to_string()
        )
        .to_string()
    })?;

    for event in EVENTS {
        let path = hooks.join(event);

        if path.exists() {
            let ours = fs::read_to_string(&path).is_ok_and(|text| text.contains(MARKER));

            if !ours {
                let said = t!("init.err_hook_exists", event = event).to_string();
                eprintln!("git-rola: {said}");

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
            eprintln!("git-rola: {said}");
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

    Ok(())
}
