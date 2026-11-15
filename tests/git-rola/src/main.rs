//! `git-rola`: what the program does in a work tree that holds a project.
//!
//! A program rather than a set of tests cargo runs, because what it checks is programs a caller
//! meets: the built `git-rola`, started the way a person starts it, the `git rola` git reaches it
//! as, and the `git` it reaches for the operations that change the working tree. What is checked is
//! what a run exits with, what it says, and what the work tree and the repository's exclude file
//! hold afterwards.
//!
//! Everything happens inside one sandbox, and the machine's own git configuration is pointed into
//! it as well (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM` and `HOME`) so a run here reads and writes
//! nothing of the person's, and a machine whose git is configured differently answers the same. The
//! programs of this build are put on the `PATH` the child git is given, because a hook installed in
//! the repository is reached by git as `git rola` and has to find it. The language is pinned to the
//! `C` locale, which git's own tests use and which is the untranslated one; the checks that are
//! about the language name their own.
//!
//! The suite is about the one thing git and rola have to agree on: a material is content, not a
//! file git keeps. git is given every material the lock names through the repository's exclude
//! file, the material is taken out of the working tree before a checkout changes it and put back
//! after, and what is put back is what the lock the checkout left says.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use librorolala::storage::Key;
use librorolala::tree::{FileMode, TreeEntry, TreeStructure};
use rorolala_utils_sandbox::{Guard, Ran, bin_dir, run};

/// The material every project here places.
const MATERIAL: &str = "Assets/hero.psd";

/// One version of the material's content, and the other a lock may name instead.
const FIRST_CUT: &[u8] = b"the first cut\n";
const SECOND_CUT: &[u8] = b"the second cut\n";

/// The hooks `init` installs, each of which is a run of this program after git changed the tree.
const HOOKS: [&str; 3] = ["post-checkout", "post-merge", "post-rewrite"];

fn main() {
    let sandbox = Guard::new("git-rola");
    let run = Run::new(sandbox.join("home"));
    let mut checked = Checked::default();

    // git is the program this suite is about: a machine without one has nothing to check here, and
    // a suite that said so is worth more than one that panicked trying to start it.
    if Command::new("git").arg("--version").output().is_err() {
        println!("skipped: git is not on the PATH, and this suite is about what git does");
        return;
    }

    let repo = sandbox.join("repo");
    let outer = sandbox.join("outer");
    let plain = sandbox.join("plain");
    let elsewhere = sandbox.join("elsewhere");

    for dir in [&repo, &outer, &plain, &elsewhere] {
        fs::create_dir_all(dir).expect("a directory to work in");
    }

    refusals(&run, &repo, &plain, &elsewhere, &mut checked);
    initialise(&run, &repo, &mut checked);
    checking(&run, &repo, &mut checked);
    restoring(&run, &repo, &mut checked);
    reaching(&run, &repo, &mut checked);
    quoting(&run, sandbox.dir(), &mut checked);
    trees(&run, sandbox.dir(), &mut checked);
    branches(&run, &repo, &mut checked);
    hooks(&run, &repo, &mut checked);
    worktrees(&run, &repo, &mut checked);
    surface(&run, &repo, &elsewhere, &mut checked);
    updating(&run, sandbox.dir(), &mut checked);
    nested(&run, &outer, &mut checked);

    checked.report();
}

/// What the program says about itself: the version, the usage, and the language both are said in.
fn surface(run: &Run, repo: &Path, elsewhere: &Path, checked: &mut Checked) {
    let version = run.rola(elsewhere, &["--version"]);
    checked.exits("`--version` ends successfully", &version, 0);
    checked.stdout_has(
        "`--version` names the program",
        &version,
        "git rola version",
    );
    checked.wants(
        "`--version` names a version",
        names_a_version(&version),
        &said(&version),
    );
    checked.wants(
        "the version is a result, not a failure",
        version.stderr.trim().is_empty(),
        &said(&version),
    );

    let version = run.rola(elsewhere, &["-v"]);
    checked.exits("`-v` is the version too", &version, 0);
    checked.wants(
        "`-v` names a version",
        names_a_version(&version),
        &said(&version),
    );

    // The version is a command of its own as well, and it ignores what follows it, the way
    // `git version extra` does.
    for words in [&["version"][..], &["version", "extra"][..]] {
        let version = run.rola(elsewhere, words);
        let question = words.join(" ");

        checked.exits(&format!("`{question}` ends successfully"), &version, 0);
        checked.wants(
            &format!("`{question}` names a version"),
            names_a_version(&version),
            &said(&version),
        );
    }

    // The root usage is git's bare-`git` shape: a usage line, the commands, and where to read more.
    for words in [&["-h"][..], &["--help"][..]] {
        let help = run.rola(elsewhere, words);
        let question = words.join(" ");

        checked.exits(&format!("`{question}` ends successfully"), &help, 0);
        checked.stdout_has(
            "the usage has git's line",
            &help,
            "usage: git rola <command>",
        );
        checked.stdout_has(
            "the usage names the commands it lists",
            &help,
            "The rola commands are",
        );
        checked.stdout_has(
            "the usage says where to read about one",
            &help,
            "See 'git rola <command>'",
        );

        for command in ["init", "hook", "check", "restore", "update", "help", "version"] {
            checked.stdout_has(&format!("the usage lists `{command}`"), &help, command);
        }

        checked.wants(
            "the usage is a result, not a failure",
            help.stderr.trim().is_empty(),
            &said(&help),
        );
    }

    // A bare run is git's bare `git`: the same usage, on stdout, with status 1.
    let bare = run.rola(elsewhere, &[]);
    checked.exits("a bare run asks for the usage", &bare, 1);
    checked.stdout_has(
        "the usage is what a bare run says",
        &bare,
        "usage: git rola <command>",
    );
    checked.wants(
        "a bare run is not a failure",
        bare.stderr.trim().is_empty(),
        &said(&bare),
    );

    // Each command writes down its own use, and `--help` as its first word asks for it with success.
    for command in ["init", "hook", "check", "restore", "update", "help", "version"] {
        let help = run.rola(elsewhere, &[command, "--help"]);
        checked.exits(&format!("`{command} --help` ends successfully"), &help, 0);
        checked.stdout_has(
            &format!("`{command} --help` is its own usage"),
            &help,
            &format!("usage: git rola {command}"),
        );
        checked.wants(
            &format!("`{command} --help` is a result, not a failure"),
            help.stderr.trim().is_empty(),
            &said(&help),
        );
    }

    // `-h` is git's usage request inside a command: the same text, on stdout, with 129.
    for command in ["init", "hook", "check", "restore", "update", "help", "version"] {
        let usage = run.rola(elsewhere, &[command, "-h"]);
        checked.exits(&format!("`{command} -h` is a usage request"), &usage, 129);
        checked.stdout_has(
            &format!("`{command} -h` is its own usage"),
            &usage,
            &format!("usage: git rola {command}"),
        );
    }

    // The command, not the program, answers a line it cannot read.
    let extra = run.rola(elsewhere, &["check", "x"]);
    checked.exits("an operand a command does not take is refused", &extra, 129);
    checked.stderr_has(
        "the refusal is the command's usage",
        &extra,
        "usage: git rola check",
    );
    checked.stdout_empty("the refusal is not a result", &extra);

    let late = run.rola(elsewhere, &["check", "x", "--help"]);
    checked.exits("`--help` after an operand is a usage request", &late, 129);
    checked.stdout_has(
        "the usage request names the command",
        &late,
        "usage: git rola check",
    );

    let flag = run.rola(elsewhere, &["check", "--bogus"]);
    checked.exits("a flag a command does not have is refused", &flag, 129);
    checked.stderr_has(
        "the refusal names the flag",
        &flag,
        "error: unknown option `bogus'",
    );
    checked.stderr_has(
        "the refusal names the command's use",
        &flag,
        "usage: git rola check",
    );
    checked.stdout_empty("the refusal is not a result", &flag);

    let short = run.rola(elsewhere, &["check", "-C"]);
    checked.exits("a short flag a command does not have is refused", &short, 129);
    checked.stderr_has("the short flag is a switch", &short, "error: unknown switch `C'");

    // The root has no flag of its own, and git says so without an `error:` prefix.
    let root = run.rola(elsewhere, &["--bogus"]);
    checked.exits("a root flag is refused", &root, 129);
    checked.stderr_has(
        "the refusal names the option as git does",
        &root,
        "unknown option: --bogus",
    );
    checked.stderr_has(
        "the refusal is followed by the program's usage",
        &root,
        "usage: git rola <command>",
    );
    checked.stdout_empty("the refusal is not a result", &root);

    // A word no command answers to is git's unknown command: status 1, and the nearest command
    // when there is one to offer.
    for (typed, offered) in [("nosuch", None), ("chek", Some("check"))] {
        let unknown = run.rola(elsewhere, &[typed]);
        checked.exits("a word that names no command is refused", &unknown, 1);
        checked.stderr_has("the refusal has git's sentence", &unknown, "is not a rola command");
        checked.stderr_has("the refusal points at the usage", &unknown, "git rola --help");
        checked.stdout_empty("the refusal is not a result", &unknown);

        if let Some(offered) = offered {
            checked.stderr_has(
                "the nearest command is offered",
                &unknown,
                "The most similar command is",
            );
            checked.stderr_has("the offered command is named", &unknown, offered);
        }
    }

    // `help` names a command's usage and ignores what follows it, the way `git help status extra`
    // does; a word that names no command is the unknown command of the line above.
    let help = run.rola(elsewhere, &["help"]);
    checked.exits("`help` ends successfully", &help, 0);
    checked.stdout_has(
        "`help` writes the program's usage",
        &help,
        "usage: git rola <command>",
    );

    for words in [
        &["help", "check"][..],
        &["help", "check", "extra"][..],
    ] {
        let help = run.rola(elsewhere, words);
        let question = words.join(" ");

        checked.exits(&format!("`{question}` ends successfully"), &help, 0);
        checked.stdout_has(
            &format!("`{question}` writes check's usage"),
            &help,
            "usage: git rola check",
        );
    }

    let help = run.rola(elsewhere, &["help", "nosuch"]);
    checked.exits("`help nosuch` is an unknown command", &help, 1);

    // The language is gettext's: `LC_ALL` above `LC_MESSAGES` above `LANG`, `C` untranslated, and
    // `LANGUAGE` read only when the locale is a language.
    let chinese = run.rola_with(elsewhere, &[("LC_ALL", "zh_CN.UTF-8")], &["--help"]);
    checked.exits("a Chinese locale ends successfully", &chinese, 0);
    checked.stdout_has("the usage is said in the locale's language", &chinese, "用法：");
    checked.wants(
        "the usage is not also said in English",
        !chinese.stdout.contains("usage:"),
        &said(&chinese),
    );

    let english = run.rola_with(
        elsewhere,
        &[("LC_ALL", "C"), ("LANGUAGE", "zh_CN")],
        &["--help"],
    );
    checked.exits("the C locale ends successfully", &english, 0);
    checked.stdout_has("the C locale is not translated", &english, "usage: git rola");
    checked.wants(
        "the C locale does not read LANGUAGE",
        !english.stdout.contains("用法："),
        &said(&english),
    );

    let preferred = run.rola_with(
        elsewhere,
        &[("LC_ALL", "zh_CN.UTF-8"), ("LANGUAGE", "zh_CN")],
        &["--help"],
    );
    checked.exits("a named preference ends successfully", &preferred, 0);
    checked.stdout_has("the preferred language is spoken", &preferred, "用法：");

    let refused = run.rola_with(elsewhere, &[("LC_ALL", "zh_CN.UTF-8")], &["nosuch"]);
    checked.exits("a refusal is the same refusal in Chinese", &refused, 1);
    checked.stderr_has(
        "the refusal follows the run's language",
        &refused,
        "不是 rola 命令",
    );

    // A wrapped command is git's to answer, flags and all: `-h` is git's own usage request, and
    // nothing about the work tree moves.
    let before = run.git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let usage = run.rola(repo, &["switch", "-h"]);
    checked.exits("a wrapped command's `-h` is git's to answer", &usage, 129);
    checked.stdout_has("the answer is git's usage of the command", &usage, "usage: git switch");
    checked.wants(
        "the program's own usage is not written over it",
        !usage.stdout.contains("usage: git rola"),
        &said(&usage),
    );
    let after = run.git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    checked.wants(
        "a wrapped command's usage runs no checkout",
        before.stdout.trim() == after.stdout.trim(),
        &said(&usage),
    );
}

/// What a run does where there is nothing to work on, and the one command the Vault half owes.
fn refusals(run: &Run, repo: &Path, plain: &Path, elsewhere: &Path, checked: &mut Checked) {
    // A run outside every project has nothing to read, and says so rather than acting on whatever
    // directory it happened to be in.
    let check = run.rola(elsewhere, &["check"]);
    checked.exits("a check outside a project is refused", &check, 128);
    checked.stderr_has(
        "the refusal says there is no project",
        &check,
        "not inside a rola project",
    );
    checked.stdout_empty("the refusal is not a result", &check);

    let restore = run.rola(elsewhere, &["restore", MATERIAL]);
    checked.exits("a restore outside a project is refused", &restore, 128);

    // A hook that finds no project is not a failure: it was installed in a repository whose project
    // may have been removed since, and a checkout there is still a run that worked.
    let hook = run.rola(elsewhere, &["hook", "post-checkout"]);
    checked.exits("a hook that finds no project ends successfully", &hook, 0);
    checked.stdout_empty("a hook that finds no project says nothing", &hook);

    // A word no command answers to is a command line git would have refused, and a script reading
    // the status must not take silence for a run that did something.
    let unknown = run.rola(elsewhere, &["no-such-command"]);
    checked.exits("a word that names no command is refused", &unknown, 1);
    checked.stderr_has(
        "the refusal points at the help",
        &unknown,
        "git rola --help",
    );
    checked.stdout_empty("the refusal is not a result", &unknown);

    // A bare run is git's bare `git`: the usage, on stdout, with 1.
    let bare = run.rola(elsewhere, &[]);
    checked.exits("a run that names nothing asks for the usage", &bare, 1);
    checked.stdout_has("the usage is what it says", &bare, "usage: git rola <command>");
    checked.wants(
        "the usage is not a failure",
        bare.stderr.trim().is_empty(),
        &said(&bare),
    );

    // A command line the program cannot read is 129, and the prefix is git's rather than the
    // program's name.
    let unreadable = run.rola(elsewhere, &["--bogus"]);
    checked.exits("a line the program cannot read is 129", &unreadable, 129);
    checked.stderr_has(
        "the refusal is git's unknown option",
        &unreadable,
        "unknown option: --bogus",
    );

    // The command that could not be made is `fatal:`, which is 128 and not 1.
    let fatal = run.rola(elsewhere, &["restore"]);
    checked.exits("a command that cannot be made is fatal", &fatal, 128);
    checked.stderr_has(
        "the failure has git's prefix",
        &fatal,
        "fatal: you must specify path(s) to restore",
    );

    // Setting up a directory that is no work tree writes the project file and then has nowhere to
    // put the hooks, so it says that rather than pretending to have set anything up.
    let init = run.rola(plain, &["init"]);
    checked.exits("setting up inside no work tree is refused", &init, 128);
    checked.stderr_has(
        "the refusal says there is no work tree",
        &init,
        "fatal: ",
    );
    checked.stderr_has(
        "the refusal says there is no work tree",
        &init,
        "no git work tree",
    );
    checked.wants(
        "the file a person writes is still the project's",
        plain.join("rola.toml").is_file(),
        &said(&init),
    );

    // A project that turns git support off is one this program leaves alone.
    let off = sibling(repo, "off");
    fs::create_dir_all(&off).expect("a directory for the project that turns it off");
    run.git(&off, &["init", "-b", "main"]);
    fs::write(off.join("rola.toml"), "[features]\ngit_support = false\n")
        .expect("the project file that turns it off");
    fs::write(off.join("rola.lock"), lock_text(FIRST_CUT)).expect("the lock");

    let disabled = run.rola(&off, &["check"]);
    checked.exits(
        "a project that turns git support off is not checked",
        &disabled,
        128,
    );
    checked.stderr_has(
        "the refusal says the feature is off",
        &disabled,
        "does not turn git support on",
    );

    // A project with no lock has nothing to check the work tree against.
    let unlocked = sibling(repo, "unlocked");
    fs::create_dir_all(&unlocked).expect("a directory for the project with no lock");
    run.git(&unlocked, &["init", "-b", "main"]);
    fs::write(
        unlocked.join("rola.toml"),
        "[features]\ngit_support = true\n",
    )
    .expect("the project file");

    let no_lock = run.rola(&unlocked, &["check"]);
    checked.exits("a project with no lock is refused", &no_lock, 128);
    checked.stderr_has(
        "the refusal names what could not be read",
        &no_lock,
        "cannot read rola.lock",
    );
}

/// What `update` does with nothing to resolve, and what it refuses when it has.
fn updating(run: &Run, sandbox: &Path, checked: &mut Checked) {
    let project = sandbox.join("updating");
    fs::create_dir_all(&project).expect("a directory for the project to update");
    run.git(&project, &["init", "-b", "main"]);

    fs::write(
        project.join("rola.toml"),
        "[features]\ngit_support = true\n",
    )
    .expect("the project file");

    let init = run.rola(&project, &["init"]);
    checked.exits("the project to update is set up", &init, 0);

    // A project that maps nothing places nothing, and resolving it reaches nobody: the lock is
    // written from the file alone, so an update with no mappings needs no Vault and no account.
    let update = run.rola(&project, &["update"]);
    checked.exits("an update with no mappings is made", &update, 0);
    checked.stdout_has(
        "the update says what the lock now holds",
        &update,
        "Updated rola.lock: 0 materials",
    );
    checked.wants(
        "the lock is the empty one",
        fs::read_to_string(project.join("rola.lock")).is_ok_and(|text| text == "? FORMAT 2\n"),
        &said(&update),
    );

    // A mapping is resolved through a Vault, which needs an account to act as. The suite's data
    // directory holds none, so the run is refused before it reaches for the address at all.
    fs::write(
        project.join("rola.toml"),
        "[vaults]\norigin = \"rola://127.0.0.1:1/nowhere\"\n\n[features]\ngit_support = true\n\n[[mapping]]\nfrom = \"1b31755f-ad8c-4ce1-b27d-970b9b3dfd58\"\nto = \"/Assets/kitten.psd\"\npin = \"latest\"\n",
    )
    .expect("the project file with a mapping");

    let before = fs::read(project.join("rola.lock")).expect("the lock before the refused update");
    let refused = run.rola(&project, &["update"]);
    checked.exits(
        "an update that cannot act as anyone is refused",
        &refused,
        128,
    );
    checked.stderr_has(
        "the refusal names the account it needs",
        &refused,
        "no account is named",
    );
    checked.wants(
        "a refused update leaves the lock as it was",
        fs::read(project.join("rola.lock")).ok() == Some(before),
        &said(&refused),
    );
    checked.wants(
        "a refused update writes no material",
        !project.join("Assets/kitten.psd").exists(),
        &said(&refused),
    );
}

/// Setting a project up: the file a person writes, the hooks, the exclude block and the materials.
fn initialise(run: &Run, repo: &Path, checked: &mut Checked) {
    let init = run.git(repo, &["init", "-b", "main"]);
    checked.exits("a repository is made to hold the project", &init, 0);

    // A line of the person's own, which the block this program manages is written around.
    let user_line = "# a line of the person's own";
    fs::write(
        repo.join(".git/info/exclude"),
        format!("{user_line}\n*.tmp\n"),
    )
    .expect("the exclude file with a line of the person's own");

    // A project is a lock and the store the lock's content is read from; the file a person writes is
    // not there yet, so what `init` reports is an initialisation rather than a reinitialisation.
    lay_project(run, repo, FIRST_CUT);

    let init = run.rola(repo, &["init"]);
    checked.exits("a project is set up", &init, 0);
    checked.stdout_has(
        "setting up names what it did",
        &init,
        "Initialized rola project in",
    );
    checked.wants(
        "setting up names where the project is",
        names(&init.stdout, repo),
        &said(&init),
    );
    checked.wants(
        "the file a person writes is written",
        fs::read_to_string(repo.join("rola.toml"))
            .is_ok_and(|text| text.contains("git_support = true")),
        &said(&init),
    );
    checked.wants(
        "the material the lock names is placed",
        holds(repo, FIRST_CUT),
        &said(&init),
    );

    for event in HOOKS {
        let hook = repo.join(".git/hooks").join(event);
        checked.wants(
            &format!("the `{event}` hook is installed"),
            fs::read_to_string(&hook).is_ok_and(|text| text.contains("git-rola hook")),
            &said(&init),
        );
    }

    let exclude = exclude_text(repo);
    checked.wants(
        "the person's own line is left alone",
        exclude.contains(user_line),
        &exclude,
    );
    checked.wants(
        "the block is the project's",
        exclude.contains("# >>> rola: . (managed; edits will be lost)"),
        &exclude,
    );
    checked.wants(
        "the project's own data is kept out of git",
        exclude.contains("/.rola/"),
        &exclude,
    );
    checked.wants(
        "the material is kept out of git",
        exclude.contains("/Assets/hero.psd"),
        &exclude,
    );
    checked.wants(
        "the block is closed",
        exclude.contains("# <<< rola: ."),
        &exclude,
    );

    // What git itself answers with is the check that matters: the patterns are git's to read.
    let ignored = run.git(repo, &["check-ignore", MATERIAL]);
    checked.exits("git agrees the material is ignored", &ignored, 0);
    checked.stdout_has("git names the material it ignores", &ignored, MATERIAL);

    let ignored = run.git(repo, &["check-ignore", ".rola/storage/rolast.toml"]);
    checked.exits("git agrees the project's data is ignored", &ignored, 0);

    // Running it again is the same set-up said again: the project file is kept, the hooks are this
    // program's, and the material is where the lock says.
    let again = run.rola(repo, &["init"]);
    checked.exits("setting a project up again is not a failure", &again, 0);
    checked.stdout_has(
        "the second run says what it did",
        &again,
        "Reinitialized existing rola project",
    );
    checked.wants(
        "the material is still placed",
        holds(repo, FIRST_CUT),
        &said(&again),
    );

    // A hook a person wrote is not this program's to replace.
    let own = "#!/bin/sh\n# a hook of the person's own\n";
    fs::write(repo.join(".git/hooks/post-merge"), own).expect("the person's hook");
    let again = run.rola(repo, &["init"]);
    checked.exits(
        "setting up beside a person's hook is not a failure",
        &again,
        0,
    );
    checked.stderr_has(
        "the hook left alone is named",
        &again,
        "'post-merge' hook already exists",
    );
    checked.wants(
        "the person's hook is left as it was",
        fs::read_to_string(repo.join(".git/hooks/post-merge")).is_ok_and(|text| text == own),
        &said(&again),
    );

    // The project files are what git is to keep; the material and the project's data are not.
    let add = run.git(repo, &["add", "-A"]);
    checked.exits("the project files are added", &add, 0);
    let commit = run.git(repo, &["commit", "-m", "the first cut"]);
    checked.exits("the first cut is committed", &commit, 0);

    let files = run.git(repo, &["ls-files"]);
    checked.stdout_has("git keeps the lock", &files, "rola.lock");
    checked.stdout_has("git keeps the project file", &files, "rola.toml");
    checked.wants(
        "git keeps no material",
        !files.stdout.contains("hero.psd"),
        &files.stdout,
    );
    checked.wants(
        "git keeps nothing of the project's own data",
        !files.stdout.contains(".rola/"),
        &files.stdout,
    );

    let status = run.git(repo, &["status", "--porcelain"]);
    checked.wants(
        "a work tree set up and committed has nothing to report",
        status.stdout.trim().is_empty(),
        &said(&status),
    );

    // A branch at the first cut, so a later checkout has an older lock to read.
    let branch = run.git(repo, &["branch", "older"]);
    checked.exits("a branch at the first cut is made", &branch, 0);
}

/// What a run reports against the lock, which is git's short status for content git does not keep.
fn checking(run: &Run, repo: &Path, checked: &mut Checked) {
    let clean = run.rola(repo, &["check"]);
    checked.exits("a work tree that is the lock's is no difference", &clean, 0);
    checked.stdout_empty("nothing is printed when nothing differs", &clean);

    let before = exclude_text(repo);
    let again = run.rola(repo, &["check"]);
    checked.exits("checking twice is the same answer", &again, 0);
    checked.wants(
        "checking rewrites the same exclude block",
        exclude_text(repo) == before,
        "the exclude file changed",
    );

    // The material is the work's, not git's: an edit to it is a difference the lock can see and git
    // cannot.
    fs::write(repo.join(MATERIAL), b"an edit\n").expect("an edit to the material");

    let edited = run.rola(repo, &["check"]);
    checked.exits(
        "an edited material is a difference, not a failure",
        &edited,
        1,
    );
    checked.stdout_has(
        "it is drawn as git draws a modified file",
        &edited,
        " M Assets/hero.psd",
    );
    checked.wants(
        "a difference says nothing on stderr",
        edited.stderr.trim().is_empty(),
        &said(&edited),
    );

    let status = run.git(repo, &["status", "--porcelain"]);
    checked.wants(
        "git does not see the material's change",
        !status.stdout.contains("hero.psd"),
        &said(&status),
    );

    // A flag the command does not have is a line it cannot read, not a different way of writing
    // the answer: `--json` and `--renderer` were the program's own, not git's.
    let unreadable = run.rola(repo, &["check", "--json"]);
    checked.exits("a flag the command does not have is refused", &unreadable, 129);
    checked.stderr_has(
        "the refusal names the flag",
        &unreadable,
        "error: unknown option `json'",
    );

    // A material that is not there at all is another of git's short statuses.
    fs::remove_file(repo.join(MATERIAL)).expect("the material taken away");

    let gone = run.rola(repo, &["check"]);
    checked.exits("a material that is not there is a difference", &gone, 1);
    checked.stdout_has(
        "it is drawn as git draws a deleted file",
        &gone,
        " D Assets/hero.psd",
    );
}

/// Putting a material back from what the project keeps of it, without the Vault.
fn restoring(run: &Run, repo: &Path, checked: &mut Checked) {
    // The material is gone from the work tree, so the run brings it out of the store.
    let restored = run.rola(repo, &["restore", MATERIAL]);
    checked.exits("a material is put back", &restored, 0);
    checked.stdout_empty("putting a material back says nothing", &restored);
    checked.wants(
        "what is put back is the lock's content",
        holds(repo, FIRST_CUT),
        &said(&restored),
    );

    // The name may be written the way the lock writes it, with the slash that marks the root.
    fs::write(repo.join(MATERIAL), b"another edit\n").expect("an edit");
    let restored = run.rola(repo, &["restore", "/Assets/hero.psd"]);
    checked.exits(
        "a material is put back by the lock's own name",
        &restored,
        0,
    );
    checked.wants(
        "the edit is thrown away",
        holds(repo, FIRST_CUT),
        &said(&restored),
    );

    // A name no material is under is refused before anything is written, so a run that mistypes one
    // restores nothing.
    let before = fs::read(repo.join(MATERIAL)).expect("the material");
    let unknown = run.rola(repo, &["restore", "Assets/not-here.psd"]);
    checked.exits("a name no material is under is refused", &unknown, 1);
    checked.stderr_has(
        "the refusal is git's pathspec",
        &unknown,
        "error: pathspec 'Assets/not-here.psd' did not match any material in rola.lock",
    );
    checked.wants(
        "a refused restore writes nothing",
        fs::read(repo.join(MATERIAL)).ok().as_ref() == Some(&before),
        &said(&unknown),
    );

    // A restore names what to put back; with nothing named git cannot make the command at all.
    let bare = run.rola(repo, &["restore"]);
    checked.exits("a restore that names nothing is fatal", &bare, 128);
    checked.stderr_has(
        "the failure is git's sentence",
        &bare,
        "fatal: you must specify path(s) to restore",
    );

    // The store is where the content comes from: a project that has lost it cannot put the material
    // back, and the run says which material it could not place.
    let store = repo.join(".rola/storage");
    let kept = sibling(repo, "storage-kept");
    fs::rename(&store, &kept).expect("the store taken away");
    fs::write(repo.join(MATERIAL), b"a third edit\n").expect("an edit");

    let missing = run.rola(repo, &["restore", MATERIAL]);
    checked.exits(
        "a material no local source holds is a partial failure",
        &missing,
        1,
    );
    checked.stderr_has(
        "the failure names the material",
        &missing,
        "error: Assets/hero.psd:",
    );
    checked.stderr_has(
        "the failure says no source holds it",
        &missing,
        "no source holds the content",
    );

    fs::rename(&kept, &store).expect("the store put back");
    let restored = run.rola(repo, &["restore", MATERIAL]);
    checked.exits(
        "the material is put back once the store holds it",
        &restored,
        0,
    );
    checked.wants(
        "the content is the lock's again",
        holds(repo, FIRST_CUT),
        &said(&restored),
    );

    // A name is read the way git reads a pathspec: `.` is everything under the run's directory, and
    // a directory is everything under it.
    fs::write(repo.join(MATERIAL), b"a fourth edit\n").expect("an edit");

    let dotted = run.rola(repo, &["restore", "."]);
    checked.exits("`.` restores everything under the run", &dotted, 0);
    checked.wants(
        "`.` puts the material back",
        holds(repo, FIRST_CUT),
        &said(&dotted),
    );

    fs::write(repo.join(MATERIAL), b"a fifth edit\n").expect("an edit");

    let directory = run.rola(repo, &["restore", "Assets"]);
    checked.exits("a directory restores everything under it", &directory, 0);
    checked.wants(
        "the directory puts the material back",
        holds(repo, FIRST_CUT),
        &said(&directory),
    );

    // A name may hold a wildcard, read the way git reads one.
    fs::write(repo.join(MATERIAL), b"a seventh edit\n").expect("an edit");

    let wildcard = run.rola(repo, &["restore", "Assets/*"]);
    checked.exits("a wildcard restores what it selects", &wildcard, 0);
    checked.wants(
        "the wildcard puts the material back",
        holds(repo, FIRST_CUT),
        &said(&wildcard),
    );

    // Run from inside the material's own directory, the same names mean the subtree it is in.
    let assets = repo.join("Assets");
    fs::write(repo.join(MATERIAL), b"a sixth edit\n").expect("an edit");

    let here = run.rola(&assets, &["restore", "hero.psd"]);
    checked.exits("a name is read from where the run is", &here, 0);
    checked.wants(
        "the name from inside the project restores the material",
        holds(repo, FIRST_CUT),
        &said(&here),
    );

    // A name that climbs out of the project names nothing: there are no materials outside it.
    let outside = run.rola(repo, &["restore", ".."]);
    checked.exits("a name that leaves the project is refused", &outside, 1);
    checked.stderr_has(
        "the refusal is git's pathspec",
        &outside,
        "error: pathspec '..' is outside the project",
    );
}

/// What `git rola` is, as opposed to the program run by hand: the same program, reached by git.
fn reaching(run: &Run, repo: &Path, checked: &mut Checked) {
    // git prepends its own directories to the `PATH` and runs `git-rola` with the words it was
    // given, so what a person types is what this checks.
    let reached = run.git(repo, &["rola", "check"]);
    checked.exits("git reaches the program", &reached, 0);
    checked.wants(
        "git hands the program a clean work tree",
        reached.stderr.trim().is_empty(),
        &said(&reached),
    );

    // `--no-pager` is git's to set, and it leaves `GIT_PAGER=cat` for the program to read: what the
    // program said before is what it says now.
    let unpaged = run.git(repo, &["--no-pager", "rola", "check"]);
    checked.exits("a run through `--no-pager` ends the same", &unpaged, 0);
    checked.wants(
        "`--no-pager` changes no answer",
        unpaged.stdout == reached.stdout && unpaged.stderr == reached.stderr,
        &said(&unpaged),
    );

    // `-h` after the command is the program's own, since git rewrites only `--help`.
    let usage = run.git(repo, &["rola", "-h"]);
    checked.exits("`git rola -h` ends successfully", &usage, 0);
    checked.stdout_has(
        "`git rola -h` is the program's usage",
        &usage,
        "usage: git rola <command>",
    );

    let unknown = run.git(repo, &["rola", "nosuch"]);
    checked.exits("`git rola nosuch` is refused", &unknown, 1);
    checked.stderr_has(
        "the refusal is the program's",
        &unknown,
        "is not a rola command",
    );

    // `--help` right after the command never reaches the program: git turns it into its own
    // documentation lookup, and there is no page for `rola` to find. That is git's search, not this
    // program's failure, and it is left as git answers it.
    let documented = run.git(repo, &["rola", "--help"]);
    checked.exits(
        "`git rola --help` is git's documentation lookup",
        &documented,
        128,
    );
    checked.stderr_has(
        "the lookup names the documentation it could not find",
        &documented,
        "documentation file not found",
    );
}

/// How a path is written out: git's quoting, under `core.quotePath` as git reads it.
fn quoting(run: &Run, sandbox: &Path, checked: &mut Checked) {
    let project = sandbox.join("quoting");
    fs::create_dir_all(&project).expect("a directory for the quoting project");
    run.git(&project, &["init", "-b", "main"]);

    fs::write(
        project.join("rola.toml"),
        "[features]\ngit_support = true\n",
    )
    .expect("the project file");

    // A name outside ASCII is what git quotes by default.
    let material = "素材.psd";
    fs::write(
        project.join("rola.lock"),
        format!("? FORMAT 2\n/{material} {} file\n", digest(FIRST_CUT)),
    )
    .expect("the lock");
    fs::write(project.join(material), b"an edit\n").expect("an edit to the material");

    let quoted = run.rola(&project, &["check"]);
    checked.exits("a path outside ASCII is a difference", &quoted, 1);
    checked.stdout_has(
        "the path is quoted the way git quotes it",
        &quoted,
        " M \"\\347\\264\\240\\346\\235\\220.psd\"",
    );

    // `core.quotePath=false` leaves the bytes as they are.
    let configured = run.git(&project, &["config", "--global", "core.quotePath", "false"]);
    checked.exits("git is told not to quote paths", &configured, 0);

    let plain = run.rola(&project, &["check"]);
    checked.exits("the same path is still a difference", &plain, 1);
    checked.stdout_has("the path is written as it is", &plain, " M 素材.psd");
    checked.wants(
        "the path is not quoted then",
        !plain.stdout.contains("\\347"),
        &said(&plain),
    );

    // The setting is the sandbox's, so it is put back for the checks that follow.
    let unset = run.git(&project, &["config", "--global", "--unset", "core.quotePath"]);
    checked.exits("the setting is put back", &unset, 0);
}

/// A material the lock names as a tree: the directory its structure names is placed, checked, put
/// back, and moved out and back as git's own changes go around it.
fn trees(run: &Run, sandbox: &Path, checked: &mut Checked) {
    let project = sandbox.join("trees");
    fs::create_dir_all(&project).expect("a directory for the tree project");
    run.git(&project, &["init", "-b", "main"]);

    let first = b"the first texture\n";
    let second = b"the second texture\n";
    store_content(run, &project, first);
    store_content(run, &project, second);

    let first_tree = TreeStructure::build(
        None,
        [
            TreeEntry::file("a.png", FileMode::Regular, key_of(first)),
            TreeEntry::directory("sub"),
            TreeEntry::file("sub/b.psd", FileMode::Regular, key_of(second)),
        ],
    )
    .expect("the first tree");
    let first_content = store_bytes(run, &project, &first_tree.encode());
    fs::write(project.join("rola.lock"), tree_lock_text(&first_content)).expect("the lock");

    let init = run.rola(&project, &["init"]);
    checked.exits("a project with a tree is set up", &init, 0);
    checked.wants(
        "the tree's file is placed",
        holds_at(&project, "Assets/Textures/a.png", first),
        &said(&init),
    );
    checked.wants(
        "the file under the tree's directory is placed",
        holds_at(&project, "Assets/Textures/sub/b.psd", second),
        &said(&init),
    );

    let check = run.rola(&project, &["check"]);
    checked.exits("a tree that is the lock's is clean", &check, 0);
    checked.stdout_empty("a clean tree says nothing", &check);

    // A file changed inside the tree is one difference, named by the path inside it.
    fs::write(project.join("Assets/Textures/sub/b.psd"), b"an edit\n").expect("an edit");
    let check = run.rola(&project, &["check"]);
    checked.exits("a changed file inside a tree is a difference", &check, 1);
    checked.stdout_has(
        "the changed file is named",
        &check,
        " M Assets/Textures/sub/b.psd",
    );

    // A path inside the tree selects the whole tree, which is written back as the lock names it.
    let restored = run.rola(&project, &["restore", "Assets/Textures/sub/b.psd"]);
    checked.exits("a path inside a tree restores it", &restored, 0);
    checked.wants(
        "the tree's file is back",
        holds_at(&project, "Assets/Textures/sub/b.psd", second),
        &said(&restored),
    );

    // A file taken out of the tree is a difference of its own, and the tree is put back whole.
    fs::remove_file(project.join("Assets/Textures/a.png")).expect("a file taken out");
    let check = run.rola(&project, &["check"]);
    checked.exits("a file taken out of a tree is a difference", &check, 1);
    checked.stdout_has(
        "the missing file is named",
        &check,
        " D Assets/Textures/a.png",
    );
    let restored = run.rola(&project, &["restore", "Assets/Textures"]);
    checked.wants(
        "the tree is put back whole",
        holds_at(&project, "Assets/Textures/a.png", first),
        &said(&restored),
    );

    // Setting the project up again places nothing a second time: the tree already there is the
    // one the lock names.
    let again = run.rola(&project, &["init"]);
    checked.exits("a project with a tree is set up again", &again, 0);
    checked.wants(
        "the tree is still there",
        holds_at(&project, "Assets/Textures/sub/b.psd", second),
        &said(&again),
    );

    // git is kept away from the tree as one material.
    let ignored = run.git(&project, &["check-ignore", "Assets/Textures/a.png"]);
    checked.exits("git agrees the tree's file is ignored", &ignored, 0);

    let status = run.git(&project, &["status", "--porcelain"]);
    checked.wants(
        "git does not see the tree",
        !status.stdout.contains("Assets/Textures"),
        &said(&status),
    );

    // The tree follows the lock a branch leaves: the structure is taken out whole and another one
    // placed, which is the same run a tree of one cut takes to reach another.
    let add = run.git(&project, &["add", "rola.toml", "rola.lock"]);
    checked.exits("the tree's lock is added", &add, 0);
    let commit = run.git(&project, &["commit", "-m", "the first tree"]);
    checked.exits("the first tree is committed", &commit, 0);

    let switch = run.rola(&project, &["switch", "-c", "other"]);
    checked.exits("a branch is made with a tree placed", &switch, 0);

    let third = b"another first texture\n";
    store_content(run, &project, third);
    let second_tree = TreeStructure::build(
        None,
        [
            TreeEntry::file("a.png", FileMode::Regular, key_of(third)),
            TreeEntry::directory("sub"),
            TreeEntry::file("sub/b.psd", FileMode::Regular, key_of(second)),
        ],
    )
    .expect("the second tree");
    let second_content = store_bytes(run, &project, &second_tree.encode());
    fs::write(project.join("rola.lock"), tree_lock_text(&second_content)).expect("the lock");
    let add = run.git(&project, &["add", "rola.lock"]);
    checked.exits("the second tree's lock is added", &add, 0);
    let commit = run.git(&project, &["commit", "-m", "the second tree"]);
    checked.exits("the second tree is committed", &commit, 0);

    let switch = run.rola(&project, &["switch", "main"]);
    checked.exits("the work moves back to the first tree", &switch, 0);
    checked.wants(
        "the first tree's content is placed",
        holds_at(&project, "Assets/Textures/a.png", first),
        &said(&switch),
    );

    let switch = run.rola(&project, &["switch", "other"]);
    checked.exits("the work moves to the second tree", &switch, 0);
    checked.wants(
        "the second tree's content is placed",
        holds_at(&project, "Assets/Textures/a.png", third),
        &said(&switch),
    );

    // A tree changed inside stops the checkout before git runs, and the change is left where it is.
    fs::write(project.join("Assets/Textures/a.png"), b"an edit\n").expect("an edit");
    let blocked = run.rola(&project, &["switch", "main"]);
    checked.exits("a changed tree stops the checkout", &blocked, 128);
    checked.stderr_has(
        "the refusal names the tree that differs",
        &blocked,
        "differs from the tree that was placed at `a.png`",
    );
    checked.wants(
        "the edit is still there",
        holds_at(&project, "Assets/Textures/a.png", b"an edit\n"),
        &said(&blocked),
    );
}

/// The commands git has no hook for: the materials are out while git changes the tree, and back
/// after, as whatever lock the change left behind says.
fn branches(run: &Run, repo: &Path, checked: &mut Checked) {
    let switch = run.rola(repo, &["switch", "-c", "feature"]);
    checked.exits("a branch is made through the wrapper", &switch, 0);
    checked.stderr_has(
        "git's own words are git's",
        &switch,
        "Switched to a new branch 'feature'",
    );
    checked.wants(
        "the material is still in the work tree",
        holds(repo, FIRST_CUT),
        &said(&switch),
    );

    let status = run.git(repo, &["status", "--porcelain"]);
    checked.wants(
        "a run that switched leaves git nothing to report",
        status.stdout.trim().is_empty(),
        &said(&status),
    );

    // The lock is what a branch changes: the second cut is placed by the same command, from the
    // same store, with nothing of the Vault in reach.
    store_content(run, repo, SECOND_CUT);
    write_lock(repo, SECOND_CUT);

    let add = run.git(repo, &["add", "rola.lock"]);
    checked.exits("the second cut's lock is added", &add, 0);
    let commit = run.git(repo, &["commit", "-m", "the second cut"]);
    checked.exits("the second cut is committed", &commit, 0);

    // Back to the first cut: the lock the checkout leaves speaks for the content placed after it.
    let switch = run.rola(repo, &["switch", "main"]);
    checked.exits("the work moves back to the first cut", &switch, 0);
    checked.wants(
        "the first cut's content is placed",
        holds(repo, FIRST_CUT),
        &said(&switch),
    );
    checked.wants(
        "what was placed is what is recorded",
        state_text(repo).contains(&digest(FIRST_CUT)),
        &state_text(repo),
    );

    let switch = run.rola(repo, &["switch", "feature"]);
    checked.exits("the work moves to the second cut", &switch, 0);
    checked.wants(
        "the second cut's content is placed",
        holds(repo, SECOND_CUT),
        &said(&switch),
    );

    // A checkout git refuses changes no lock, so the material comes back from what the wrapper took
    // out rather than from a lock the checkout never reached.
    let plain = run.git(repo, &["switch", "no-such-branch"]);
    checked.wants(
        "git refuses a branch that is not there",
        !plain.success(),
        &said(&plain),
    );
    let expected = plain.code;

    let failed = run.rola(repo, &["switch", "no-such-branch"]);
    checked.wants(
        "a refused checkout ends the way git ended it",
        failed.code == expected,
        &said(&failed),
    );
    checked.stderr_has(
        "git's own refusal is what is read",
        &failed,
        "invalid reference",
    );
    checked.wants(
        "a refused checkout leaves the material as it was",
        holds(repo, SECOND_CUT),
        &said(&failed),
    );

    // A merge is wrapped the same way: what the merge leaves in the lock is what is placed.
    let switch = run.rola(repo, &["switch", "main"]);
    checked.exits("the work moves back for the merge", &switch, 0);

    let merge = run.rola(
        repo,
        &[
            "merge",
            "--no-ff",
            "-m",
            "the second cut comes over",
            "feature",
        ],
    );
    checked.exits("a merge is made through the wrapper", &merge, 0);
    checked.wants(
        "the merged lock's content is placed",
        holds(repo, SECOND_CUT),
        &said(&merge),
    );

    // A pull with nothing to pull is git's failure to report, not this program's.
    let plain = run.git(repo, &["pull"]);
    checked.wants(
        "git refuses a pull with no remote",
        !plain.success(),
        &said(&plain),
    );
    let expected = plain.code;

    let pull = run.rola(repo, &["pull"]);
    checked.wants(
        "a pull git refuses ends the way git ended it",
        pull.code == expected,
        &said(&pull),
    );
    checked.wants(
        "a refused pull leaves the material as it was",
        holds(repo, SECOND_CUT),
        &said(&pull),
    );
}

/// A checkout made by plain `git`, which only the hooks after it can keep in step.
fn hooks(run: &Run, repo: &Path, checked: &mut Checked) {
    let switch = run.git(repo, &["switch", "older"]);
    checked.exits("a plain checkout is made", &switch, 0);
    checked.wants(
        "the post-checkout hook placed the older lock's content",
        holds(repo, FIRST_CUT),
        &said(&switch),
    );

    let switch = run.git(repo, &["switch", "main"]);
    checked.exits("a plain checkout back is made", &switch, 0);
    checked.wants(
        "the post-checkout hook placed the newer lock's content",
        holds(repo, SECOND_CUT),
        &said(&switch),
    );

    // The hook is a command of its own, and one run by hand is the same run.
    let hook = run.rola(repo, &["hook", "post-checkout"]);
    checked.exits("a hook run by hand is not a failure", &hook, 0);
    checked.stdout_empty("a hook that placed everything says nothing", &hook);
    checked.wants(
        "the material is where the lock says",
        holds(repo, SECOND_CUT),
        &said(&hook),
    );

    let event = run.rola(repo, &["hook", "no-such-event"]);
    checked.exits(
        "a hook event the program does not know is refused",
        &event,
        129,
    );

    let bare = run.rola(repo, &["hook"]);
    checked.exits("a hook run that names no event is refused", &bare, 129);

    let status = run.git(repo, &["status", "--porcelain"]);
    checked.wants(
        "after a plain checkout git has nothing to report",
        status.stdout.trim().is_empty(),
        &said(&status),
    );
}

/// A repository with more than one work tree, which this build refuses to act in.
///
/// git keeps one exclude file for the whole repository and reads it in every work tree, so two work
/// trees of one project would manage the same block with different locks: whichever ran last would
/// leave the other's materials visible to git. The refusal is what keeps that from happening.
fn worktrees(run: &Run, repo: &Path, checked: &mut Checked) {
    let linked = sibling(repo, "linked");
    let path = text(&linked);

    // The post-checkout hook runs as the work tree is made. It cannot place the materials of a work
    // tree this build does not act in, and it must not fail git's own command over it either: a
    // hook is not where a run is refused.
    let add = run.git(repo, &["worktree", "add", "-b", "linked-branch", &path]);
    checked.exits("a linked work tree is made", &add, 0);
    checked.stderr_has(
        "the hook says it does not act in the linked work tree",
        &add,
        "linked work tree",
    );

    // Nothing this build does may be done there.
    let check = run.rola(&linked, &["check"]);
    checked.exits("a check in a linked work tree is refused", &check, 128);
    checked.stderr_has(
        "the refusal says the work tree is a linked one",
        &check,
        "linked work tree",
    );
    checked.stdout_empty("the refusal is not a result", &check);

    let restore = run.rola(&linked, &["restore", MATERIAL]);
    checked.exits("a restore in a linked work tree is refused", &restore, 128);

    let hook = run.rola(&linked, &["hook", "post-checkout"]);
    checked.exits("a hook in a linked work tree is not a failure", &hook, 0);
    checked.stderr_has(
        "the hook says it does not act there",
        &hook,
        "linked work tree",
    );

    // A wrapped command is refused before git is reached, so nothing about the work tree moves.
    let before = run.git(&linked, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let switch = run.rola(&linked, &["switch", "main"]);
    checked.exits(
        "a wrapped command in a linked work tree is refused",
        &switch,
        128,
    );
    checked.stderr_has(
        "the refusal says the work tree is a linked one",
        &switch,
        "linked work tree",
    );
    let after = run.git(&linked, &["rev-parse", "--abbrev-ref", "HEAD"]);
    checked.wants(
        "a refused wrapped command runs no git",
        before.stdout.trim() == after.stdout.trim(),
        &said(&switch),
    );

    // Setting up is refused before the project file is written, so a run that cannot act leaves
    // nothing behind.
    fs::remove_file(linked.join("rola.toml")).expect("the project file taken away");
    let init = run.rola(&linked, &["init"]);
    checked.exits("setting up in a linked work tree is refused", &init, 128);
    checked.stderr_has(
        "the refusal names the linked work tree",
        &init,
        "linked work tree",
    );
    checked.wants(
        "a refused set-up writes no project file",
        !linked.join("rola.toml").exists(),
        &said(&init),
    );

    // The main work tree is where this build acts, and what happens there is unchanged.
    let status = run.git(repo, &["status", "--porcelain"]);
    checked.wants(
        "the main work tree still has nothing to report",
        status.stdout.trim().is_empty(),
        &said(&status),
    );

    let check = run.rola(repo, &["check"]);
    checked.exits("the main work tree is still checked", &check, 0);
}

/// A project below the work tree's root: the block is named and anchored where the project is.
fn nested(run: &Run, outer: &Path, checked: &mut Checked) {
    let project = outer.join("sub");
    fs::create_dir_all(&project).expect("a project below the work tree's root");

    let init = run.git(outer, &["init", "-b", "main"]);
    checked.exits("a repository is made around the project", &init, 0);

    lay_project(run, &project, FIRST_CUT);
    let init = run.rola(&project, &["init"]);
    checked.exits("a project below the work tree's root is set up", &init, 0);

    let exclude = exclude_text(outer);
    checked.wants(
        "the block is named where the project is",
        exclude.contains("# >>> rola: sub (managed; edits will be lost)"),
        &exclude,
    );
    checked.wants(
        "the material's pattern is anchored at the project",
        exclude.contains("/sub/Assets/hero.psd"),
        &exclude,
    );
    checked.wants(
        "the project's own data is anchored there too",
        exclude.contains("/sub/.rola/"),
        &exclude,
    );

    // The paths a check prints are the work tree's, not the project's: a run made below the root
    // names what it read the way git would.
    fs::write(project.join(MATERIAL), b"an edit\n").expect("an edit");
    let check = run.rola(&project, &["check"]);
    checked.exits(
        "a material below the work tree's root is checked",
        &check,
        1,
    );
    checked.stdout_has(
        "the difference is named from the work tree",
        &check,
        " M sub/Assets/hero.psd",
    );

    let status = run.git(outer, &["status", "--porcelain"]);
    checked.wants(
        "git does not see the material below the root",
        !status.stdout.contains("hero.psd"),
        &said(&status),
    );

    // A name copied out of a `check` is written from the work tree's root, and still names the
    // material it printed.
    let restored = run.rola(&project, &["restore", "sub/Assets/hero.psd"]);
    checked.exits("a name as check prints it is restored", &restored, 0);
    checked.wants(
        "the name from the work tree's root puts the material back",
        holds(&project, FIRST_CUT),
        &said(&restored),
    );

    // A run at the work tree's root is outside the project, which is what the walk up to the file a
    // project is found by says.
    let root = run.rola(outer, &["check"]);
    checked.exits("a run above the project is outside it", &root, 128);
}

/// A program that runs the programs of this build, with a sandbox's own git configuration.
struct Run {
    /// Where the programs of this build are, made absolute so a hook finds them as well.
    programs: PathBuf,
    /// Where the user's own files are looked for.
    home: PathBuf,
    /// The git configuration every run reads, which is empty.
    gitconfig: PathBuf,
    /// Where the `rola` program keeps what it keeps for the user.
    data: PathBuf,
    /// Where the keys named by `ROLA_HOME` are looked for.
    rola_home: PathBuf,
}

impl Run {
    /// A run whose git is configured by nothing at all.
    fn new(home: PathBuf) -> Self {
        fs::create_dir_all(&home).expect("the home a run is given");
        let data = sibling(&home, "data");
        let rola_home = sibling(&home, "rola-home");
        fs::create_dir_all(&data).expect("the data a run is given");
        fs::create_dir_all(&rola_home).expect("the rola home a run is given");

        let gitconfig = home.join("gitconfig");
        fs::write(&gitconfig, "").expect("an empty git configuration");

        let mut programs = bin_dir();
        if !programs.is_absolute() {
            programs = std::env::current_dir()
                .expect("the directory the suite was started in")
                .join(programs);
        }

        Self {
            programs,
            home,
            gitconfig,
            data,
            rola_home,
        }
    }

    /// The directory the suite's fixtures are laid down in.
    fn sandbox(&self) -> &Path {
        self.home.parent().expect("the sandbox directory")
    }

    /// The workspace a fixture's store is written through.
    ///
    /// A store's objects are framed and compressed by the store's own backend, so one laid down by
    /// hand is not a store `git-rola` can read. The fixture is written by the `rola` program
    /// instead, the same program a person's store is written by, and the store copied from there.
    fn store(&self) -> PathBuf {
        self.sandbox().join("store")
    }

    /// A command that runs `program` in `dir`.
    ///
    /// The machine's own git configuration is out of reach, so what a run answers does not depend on
    /// how the person's git is configured, and the programs of this build are on the `PATH`, so a
    /// hook the child git runs reaches the program that installed it. What `rola` keeps for the user
    /// is pointed into the sandbox too, so a fixture's store and account are the suite's own.
    fn command(&self, dir: &Path, program: &Path) -> Command {
        let machine_path = std::env::var_os("PATH").unwrap_or_default();
        let path =
            std::iter::once(self.programs.clone()).chain(std::env::split_paths(&machine_path));
        let path =
            std::env::join_paths(path).expect("a PATH of the machine's own and this build's");

        let mut command = Command::new(program);
        command
            .current_dir(dir)
            .env("HOME", &self.home)
            .env("ROLA_DATA_HOME", &self.data)
            .env("ROLA_HOME", &self.rola_home)
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .env("GIT_CONFIG_SYSTEM", &self.gitconfig)
            .env("GIT_AUTHOR_NAME", "Rola Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Rola Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            // The language is pinned to the untranslated locale, so that what a check reads back
            // does not depend on the machine it runs on. A check about the language names its own.
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("PATH", path);

        command
    }

    /// Runs git in `dir`.
    fn git(&self, dir: &Path, args: &[&str]) -> Ran {
        let mut command = self.command(dir, Path::new("git"));
        command.args(args);

        run(&mut command)
    }

    /// Runs the built `git-rola` in `dir`.
    fn rola(&self, dir: &Path, args: &[&str]) -> Ran {
        let mut command = self.command(dir, &self.programs.join("git-rola"));
        command.args(args);

        run(&mut command)
    }

    /// Runs the built `rola` in `dir`: the program a fixture's store is written by.
    fn rola_cli(&self, dir: &Path, args: &[&str]) -> Ran {
        let mut command = self.command(dir, &self.programs.join("rola"));
        command.args(args);

        run(&mut command)
    }

    /// Runs the built `git-rola` in `dir` with `environment` set over the suite's own.
    ///
    /// A language check names its locale here, since what it is about is the environment the run
    /// reads rather than the one the suite settled on.
    fn rola_with(&self, dir: &Path, environment: &[(&str, &str)], args: &[&str]) -> Ran {
        let mut command = self.command(dir, &self.programs.join("git-rola"));
        command.envs(environment.iter().copied()).args(args);

        run(&mut command)
    }
}

/// A directory beside `of`, in the sandbox.
fn sibling(of: &Path, name: &str) -> PathBuf {
    of.parent().expect("a sandbox directory").join(name)
}

/// Lays down what a project carries before anything has been set up for it: the lock and the store
/// the materials are read from. The file a person writes is not one of them, `init` writes that.
fn lay_project(run: &Run, root: &Path, content: &[u8]) {
    fs::create_dir_all(root.join("Assets")).expect("the material's directory");
    store_content(run, root, content);
    write_lock(root, content);
}

/// Puts `content` into the project's store, written by the store's own program.
fn store_content(run: &Run, project: &Path, content: &[u8]) {
    store_bytes(run, project, content);
}

/// Puts `bytes` into the project's store and answers with the content hash they are stored under.
///
/// The store is made in a workspace of the suite's own and copied across, so the project is only
/// ever what `git-rola` itself would leave there. The key the program answers with is checked
/// against the content hash a lock names: the two are the same number, and that is what makes a
/// lock's token the store's key. A tree's structure is written the same way a file's content is,
/// since a structure is content like any other.
fn store_bytes(run: &Run, project: &Path, bytes: &[u8]) -> String {
    let scratch = run.store();

    if !scratch.join(".rola/workspace.toml").is_file() {
        let made = run.rola_cli(run.sandbox(), &["create", &text(&scratch)]);
        assert!(
            made.success(),
            "a workspace to write the store in: {}",
            said(&made)
        );
    }

    let file = run.sandbox().join("storing.bin");
    fs::write(&file, bytes).expect("the content to store");

    let stored = run.rola_cli(&scratch, &["storage", "write-file", &text(&file)]);
    assert!(
        stored.success(),
        "the content is stored by the store's own program: {}",
        said(&stored),
    );

    let content = stored.stdout.trim().to_owned();
    assert_eq!(
        content,
        digest(bytes),
        "the store names the content the way a lock does"
    );

    copy_tree(
        &scratch.join(".rola/storage"),
        &project.join(".rola/storage"),
    );

    content
}

/// Copies the files of `from` into `to`, making what `to` needs.
fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).expect("the directory to copy") {
        let entry = entry.expect("an entry to copy");
        let target = to.join(entry.file_name());

        if entry.file_type().expect("an entry's kind").is_dir() {
            fs::create_dir_all(&target).expect("a directory to copy into");
            copy_tree(&entry.path(), &target);
        } else {
            fs::create_dir_all(to).expect("the directory to copy into");
            fs::copy(entry.path(), &target).expect("the file to copy");
        }
    }
}

/// Writes `rola.lock` naming `content` at the material's path.
fn write_lock(root: &Path, content: &[u8]) {
    fs::write(root.join("rola.lock"), lock_text(content)).expect("the lock");
}

/// The text of a lock naming `content` at the material's path.
fn lock_text(content: &[u8]) -> String {
    format!("? FORMAT 2\n/{MATERIAL} {} file\n", digest(content))
}

/// The text of a lock naming `content` as the tree at `/Assets/Textures`.
fn tree_lock_text(content: &str) -> String {
    format!("? FORMAT 2\n/Assets/Textures {content} tree\n")
}

/// The key the store holds `bytes` under.
fn key_of(bytes: &[u8]) -> Key {
    Key::new(*blake3::hash(bytes).as_bytes())
}

/// Whether `path` under `root` holds `content`.
fn holds_at(root: &Path, path: &str, content: &[u8]) -> bool {
    fs::read(root.join(path)).is_ok_and(|found| found == content)
}

/// The content hash of `content`, as a lock writes it.
fn digest(content: &[u8]) -> String {
    format!("blake3:{}", hex(content))
}

/// The digest of `content` as hexadecimal.
fn hex(content: &[u8]) -> String {
    blake3::hash(content).to_hex().to_string()
}

/// Whether the material's place holds `content`.
fn holds(root: &Path, content: &[u8]) -> bool {
    fs::read(root.join(MATERIAL)).is_ok_and(|found| found == content)
}

/// The repository's exclude file, read.
fn exclude_text(root: &Path) -> String {
    fs::read_to_string(root.join(".git/info/exclude")).unwrap_or_default()
}

/// What the project records it placed, read.
fn state_text(root: &Path) -> String {
    fs::read_to_string(root.join(".rola/git/state.toml")).unwrap_or_default()
}

/// The path as the argument a command is given.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Whether what a run said names `path`, however that platform spells a separator.
///
/// A place is written the way the program's path types write one, `/`, and an argument is written
/// the way the machine writes one, so the two are the same place spelled twice.
fn names(said: &str, path: &Path) -> bool {
    said.replace('\\', "/")
        .contains(&path.to_string_lossy().replace('\\', "/"))
}

/// What a run did, said the way a check that failed wants to be read.
fn said(ran: &Ran) -> String {
    format!(
        "ended with {:?}; stdout {:?}; stderr {:?}",
        ran.code,
        ran.stdout.trim(),
        ran.stderr.trim()
    )
}

/// Whether what a version run said ends with a version number.
///
/// The suite is a package of its own, so its own version is not the program's: what is checked is
/// that the program names a version, not one particular number.
fn names_a_version(ran: &Ran) -> bool {
    let line = ran.stdout.trim();
    let Some(version) = line.strip_prefix("git rola version ") else {
        return false;
    };

    let mut parts = version.split('.');

    parts.next().is_some_and(|part| part.parse::<u64>().is_ok())
        && parts.all(|part| part.parse::<u64>().is_ok())
        && version.contains('.')
}

/// What was asked of the programs, and which of the questions were not answered as they should be.
#[derive(Default)]
struct Checked {
    /// How many questions were asked.
    asked: usize,
    /// Which of them were answered with something other than what was wanted.
    wrong: Vec<String>,
}

impl Checked {
    /// Counts a question, and records it as wrong when it was not answered as it should be.
    fn wants(&mut self, question: &str, ok: bool, why: &str) {
        self.asked += 1;

        if !ok {
            self.wrong.push(format!("{question}, {why}"));
        }
    }

    /// Asks for a run to have ended with `code`.
    fn exits(&mut self, question: &str, ran: &Ran, code: i32) {
        self.wants(question, ran.code == Some(code), &said(ran));
    }

    /// Asks for what a run printed on standard output to have said `needle`.
    fn stdout_has(&mut self, question: &str, ran: &Ran, needle: &str) {
        self.wants(question, ran.stdout.contains(needle), &said(ran));
    }

    /// Asks for what a run printed on standard error to have said `needle`.
    fn stderr_has(&mut self, question: &str, ran: &Ran, needle: &str) {
        self.wants(question, ran.stderr.contains(needle), &said(ran));
    }

    /// Asks for a run to have printed nothing on standard output.
    fn stdout_empty(&mut self, question: &str, ran: &Ran) {
        self.wants(question, ran.stdout.trim().is_empty(), &said(ran));
    }

    /// Says what was asked and what was wrong with the answers, and ends the program on them.
    fn report(self) {
        for said in &self.wrong {
            eprintln!("FAIL: {said}");
        }

        if self.wrong.is_empty() {
            println!(
                "{} questions, every one answered as it should be",
                self.asked
            );
            return;
        }

        eprintln!(
            "{} of {} answered as they should not",
            self.wrong.len(),
            self.asked
        );
        std::process::exit(1);
    }
}
