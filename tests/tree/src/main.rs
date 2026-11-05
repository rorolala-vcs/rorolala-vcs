//! `tree`: what the `rola tree` commands do when someone runs them.
//!
//! A program rather than a set of tests cargo runs, because what it checks is a program a caller
//! meets: the built `rola`, started the way a terminal starts it, told things and read back. It
//! takes a Workspace through a tree's whole local life: making one, staging its work, recording
//! versions of it, reading them back, and staging a removal.
//!
//! Everything happens inside one sandbox, and the places the program keeps for the user are pointed
//! into it as well. The language is pinned to English, since what is read back is text.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rorolala_utils_sandbox::{Guard, Ran, command, run};

/// The language every run is made in, so what is read back is a known text.
const EN: &str = "en";

fn main() {
    let sandbox = Guard::new("tree");

    let home = sandbox.join("home");
    let data = sandbox.join("data");
    let rola_home = sandbox.join("rola-home");
    let ws = sandbox.join("ws");

    for dir in [&home, &data, &rola_home, &ws] {
        fs::create_dir_all(dir).expect("a sandbox directory");
    }

    let rola = Rola {
        home,
        data,
        rola_home,
    };
    let mut checked = Checked::default();

    creation(&rola, &ws, &mut checked);
    lifecycle(&rola, &ws, &mut checked);
    reading(&rola, &ws, &mut checked);
    removal(&rola, &ws, &mut checked);
    retrack(&rola, &ws, &mut checked);
    conflicts(&rola, &ws, &mut checked);
    restore(&rola, &ws, &mut checked);
    refusals(&rola, &ws, &mut checked);

    checked.report();
}

/// A Workspace is made, and a tree is made of a directory under it.
fn creation(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["init"]);
    checked.exits("a Workspace is made", &ran, 0);

    let ran = rola.run(ws, &["key", "generate", "--install", "alice"]);
    checked.exits("a key pair is made and installed", &ran, 0);

    let ran = rola.run(ws, &["account", "alice"]);
    checked.exits("the account is the one a run acts as", &ran, 0);

    fs::create_dir_all(ws.join("main").join("sub")).expect("a directory to make a tree of");
    fs::write(ws.join("main").join("a.psd"), b"hello").expect("a file in the tree");
    fs::write(ws.join("main").join("sub").join("b.psd"), b"world").expect("a file under it");

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("a directory that is not a tree is refused", &ran, 95);
    checked.stderr_has("the refusal says the path is not inside a tree", &ran, "not inside a tree");

    let ran = rola.run(ws, &["tree", "init", "main"]);
    checked.exits("a directory is made a tree", &ran, 0);
    checked.wants(
        "the tree has its own mark",
        ws.join("main").join(".rola-tree").is_dir(),
        "the mark is not there",
    );

    let ran = rola.run(ws, &["tree", "add", "main"]);
    checked.exits("the work of the tree is staged", &ran, 0);
    checked.stdout_has("what was staged is said", &ran, "added");
    checked.stdout_has("a file is named", &ran, "a.psd");

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree's work is read", &ran, 0);
    checked.stdout_has("what is staged is a part of the reading", &ran, "Staged:");
    checked.stdout_has("the directory is staged as an entry", &ran, "sub");
}

/// The staged work is recorded, and the tree's whole life goes on in versions.
fn lifecycle(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["track", "main", "--message", "first tree"]);
    checked.exits("the tree is recorded", &ran, 0);
    checked.stdout_has("the recording says what it did", &ran, "main");
    let first = layout_version(rola, ws);

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read after recording", &ran, 0);
    checked.stdout_has("a recorded tree with nothing staged is clean", &ran, "clean");

    let ran = rola.run(ws, &["tree", "ls-tree", "main"]);
    checked.exits("the recorded version is listed", &ran, 0);
    checked.stdout_has("a file of the version is listed", &ran, "a.psd");
    checked.stdout_has("a file under a directory is listed", &ran, "sub/b.psd");

    fs::write(ws.join("main").join("a.psd"), b"changed").expect("work changed after recording");

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the change is read", &ran, 0);
    checked.stdout_has("what is not staged is a part of the reading", &ran, "Not staged:");
    checked.stdout_has("the change is a modification", &ran, "modified");

    let ran = rola.run(ws.join("main").as_path(), &["tree", "diff-stat"]);
    checked.exits("the recorded version is compared with the work", &ran, 0);
    checked.stdout_has("the comparison names the changed path", &ran, "M a.psd");

    let ran = rola.run(
        ws.join("main").as_path(),
        &[
            "tree",
            "diff-stat",
            "--format",
            "{{ changes.status }}:{{ changes.path }}",
        ],
    );
    checked.exits("the comparison is drawn through a template", &ran, 0);
    checked.stdout_has("the template sees the change", &ran, "M:a.psd");

    let ran = rola.run(ws, &["status", "main"]);
    checked.exits("a named tree is read as a tree", &ran, 0);
    checked.stdout_has("the tree's own reading is drawn", &ran, "Tree `main`");
    checked.stdout_has("the change is not staged", &ran, "Not staged:");

    let ran = rola.run(ws, &["status", "main/a.psd"]);
    checked.exits("a file inside a tree is refused as a status target", &ran, 94);
    checked.stderr_has("the refusal says the path is inside a tree", &ran, "inside a tree");

    let ran = rola.run(ws.join("main").as_path(), &["tree", "diff-stat", "--staged"]);
    checked.exits("the staged structure is compared with the work", &ran, 0);
    checked.stdout_has("the staged comparison names the path", &ran, "M a.psd");

    let ran = rola.run(ws.join("main").as_path(), &["tree", "add", "."]);
    checked.exits("the change is staged from inside the tree", &ran, 0);

    let ran = rola.run(ws.join("main").as_path(), &["tree", "write-desc", "a longer body"]);
    checked.exits("a description is written without an editor", &ran, 0);

    let ran = rola.run(ws.join("main").as_path(), &["track", ".", "--message", "second tree"]);
    checked.exits("a tree is recorded from inside it", &ran, 0);

    if let (Some(first), Some(second)) = (first, layout_version(rola, ws)) {
        let ran = rola.run(ws, &["tree", "diff-stat", &first, &second]);
        checked.exits("two versions are compared", &ran, 0);
        checked.stdout_has("the comparison names the changed path", &ran, "M a.psd");
    } else {
        checked.wants(
            "two versions are compared",
            false,
            "the tree's versions could not be read",
        );
    }

    let ran = rola.run(ws.join("main").as_path(), &["tree", "cat-desc", "."]);
    checked.exits("the description is read", &ran, 0);
    checked.stdout_has("the description is what was written", &ran, "a longer body");

    let ran = rola.run(ws.join("main").as_path(), &["status"]);
    checked.exits("a run inside a tree reads the tree", &ran, 0);
    checked.stdout_has("the reading is the tree's", &ran, "Tree `main`");

    let ran = rola.run(ws, &["layout", "entries"]);
    checked.exits("the Layout is read", &ran, 0);
    checked.stdout_has("the tree is one entry rather than its files", &ran, "main");
}

/// What a version holds is read back, as lines and as data.
fn reading(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["tree", "ls-tree", "main"]);
    checked.exits("the version is listed", &ran, 0);
    checked.stdout_has("a file is a blob", &ran, "100644 blob ");
    checked.stdout_has("a directory is a tree", &ran, "040000 tree -");

    let ran = rola.run(
        ws,
        &[
            "tree",
            "ls-tree",
            "main",
            "--format",
            "{{ entries.mode }}:{{ entries.path }}",
        ],
    );
    checked.exits("the listing is drawn through a template", &ran, 0);
    checked.stdout_has("the template sees the entry", &ran, "100644:a.psd");

    let ran = rola.run(ws, &["tree", "cat-desc", "main"]);
    checked.exits("the version's description is read from its root", &ran, 0);
    checked.stdout_has("the description is the one recorded", &ran, "a longer body");

    let ran = rola.run(ws, &["vcs-index", "ls-variants"]);
    let hashes: Vec<String> = ran
        .stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|token| token.len() == 64)
        .map(str::to_owned)
        .collect();
    let mut read_from_variant = false;
    for hash in &hashes {
        let ran = rola.run(ws, &["tree", "cat-desc", hash]);
        if ran.stdout.contains("a longer body") {
            read_from_variant = true;

            break;
        }
    }
    checked.wants(
        "a variant hash reads the description of its structure",
        read_from_variant,
        "no variant hash read the recorded description",
    );

    let ran = rola.run(ws, &["tree", "status", "main", "--json"]);
    checked.exits("the reading is asked for as data", &ran, 0);
    checked.wants(
        "the data is the tree's reading",
        json(&ran).is_some_and(|value| value.get("status").is_some()),
        "the data does not hold a status",
    );
}

/// Work is staged as removed, and the version after it no longer holds it.
fn removal(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["tree", "rm", "main/sub/b.psd"]);
    checked.exits("a file's removal is staged", &ran, 0);

    let ran = rola.run(ws, &["tree", "ls-staging-tree", "main"]);
    checked.exits("the staged structure is listed after the removal", &ran, 0);
    checked.wants(
        "the empty directory is not in the staged structure",
        !ran.stdout.contains("sub"),
        "the emptied directory is still listed",
    );
    checked.wants(
        "the file is taken out of the work",
        !ws.join("main").join("sub").join("b.psd").exists(),
        "the file is still there",
    );

    let ran = rola.run(ws, &["track", "main", "--message", "without b"]);
    checked.exits("the removal is recorded", &ran, 0);

    let ran = rola.run(ws, &["tree", "ls-tree", "main"]);
    checked.exits("the new version is listed", &ran, 0);
    checked.wants(
        "the removed file is not in the new version",
        !ran.stdout.contains("b.psd"),
        "the removed file is still listed",
    );

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read after the removal is recorded", &ran, 0);
    checked.stdout_has("the tree is clean again", &ran, "clean");
}

/// A tree's record moves back to a version, and the work is put back from it.
fn retrack(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["retrack", "main", "~1"]);
    checked.exits("the tree's record moves to the version before", &ran, 0);

    let ran = rola.run(ws, &["tree", "ls-tree", "main"]);
    checked.exits("the version moved to is listed", &ran, 0);
    checked.stdout_has(
        "the file the later version removed is back",
        &ran,
        "sub/b.psd",
    );

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read after the move", &ran, 0);
    checked.stdout_has("the work matches what the tree records", &ran, "clean");

    fs::write(ws.join("main").join("sub").join("b.psd"), b"dirty").expect("work changed");

    let ran = rola.run(ws, &["retrack", "main", "~1"]);
    checked.exits("work that was never recorded is refused", &ran, 253);
    checked.stderr_has("the refusal says the tree is dirty", &ran, "never recorded");

    let ran = rola.run(ws, &["retrack", "--force", "main", "~1"]);
    checked.exits("`--force` writes over the work", &ran, 0);

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read after the forced move", &ran, 0);
    checked.stdout_has("the work matches what the tree records again", &ran, "clean");
}

/// A conflict a merge left is read, and staging the path settles it.
fn conflicts(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let conflicts = ws.join("main").join(".rola-tree").join("conflicts");
    fs::write(&conflicts, "a.psd\n").expect("a conflict list");
    fs::write(ws.join("main").join("a~theirs.psd"), b"theirs").expect("a conflict copy");

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read with a conflict", &ran, 0);
    checked.stdout_has("the conflict is a part of the reading", &ran, "Conflicts:");
    checked.stdout_has("the conflict names its copy", &ran, "a~theirs.psd");

    let ran = rola.run(ws, &["tree", "add", "main/a.psd"]);
    checked.exits("staging the path settles the conflict", &ran, 0);
    checked.wants(
        "the conflict copy is gone",
        !ws.join("main").join("a~theirs.psd").exists(),
        "the conflict copy is still there",
    );

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read after settling", &ran, 0);
    checked.wants(
        "the conflict is no longer read",
        !ran.stdout.contains("Conflicts:"),
        "the conflict is still listed",
    );
}

/// The work of a tree is put back to what it records.
fn restore(rola: &Rola, ws: &Path, checked: &mut Checked) {
    fs::write(ws.join("main").join("a.psd"), b"dirty").expect("work changed");
    fs::write(ws.join("main").join("untracked.psd"), b"new").expect("work that was never recorded");

    let ran = rola.run(ws, &["align", "main", "--restore"]);
    checked.exits("the tree's work is put back", &ran, 0);
    checked.wants(
        "a recorded file holds what the version holds",
        fs::read(ws.join("main").join("a.psd")).ok().as_deref() == Some(b"hello".as_slice()),
        "the file still holds the change",
    );
    checked.wants(
        "work that was never recorded is left where it is",
        ws.join("main").join("untracked.psd").exists(),
        "the untracked file is gone",
    );

    let ran = rola.run(ws, &["align", "main", "--restore", "--completely"]);
    checked.exits("the work is put back completely", &ran, 0);
    checked.wants(
        "`--completely` takes out what was never recorded",
        !ws.join("main").join("untracked.psd").exists(),
        "the untracked file is still there",
    );

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read after the restore", &ran, 0);
    checked.stdout_has("the rest of the work matches what the tree records", &ran, "clean");
}

/// The ways a tree command refuses what it cannot do.
fn refusals(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["tree", "init", "."]);
    checked.exits("making the Workspace root a tree asks first", &ran, 94);

    let ran = rola.run(ws.join("main").as_path(), &["tree", "init", "."]);
    checked.exits("a directory that is already a tree is refused", &ran, 94);
    checked.wants(
        "the tree already made is still the same tree",
        ws.join("main").join(".rola-tree").is_dir(),
        "the mark is gone",
    );

    let ran = rola.run(ws, &["tree", "add", "."]);
    checked.exits("a run outside a tree cannot stage its work", &ran, 95);

    let ran = rola.run(ws, &["tree", "rm", "main/a.psd"]);
    checked.exits("a recorded file's removal is staged", &ran, 0);

    let ran = rola.run(ws, &["tree", "rm", "main/sub"]);
    checked.exits("a directory is not taken without `--recursive`", &ran, 94);
    checked.stderr_has("the refusal says what a directory needs", &ran, "recursive");
}

/// A command that runs `rola`, working in `dir` and keeping the user's files in the sandbox.
struct Rola {
    /// Where the user's own files are looked for.
    home: PathBuf,
    /// Where the program keeps what it keeps for the user.
    data: PathBuf,
    /// Where the keys named by `ROLA_HOME` are looked for.
    rola_home: PathBuf,
}

impl Rola {
    /// A command that runs the built `rola` in `dir`, with nothing of the machine's in reach.
    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        let mut command = command("rola");

        command
            .current_dir(dir)
            .env("HOME", &self.home)
            .env("XDG_DATA_HOME", &self.data)
            .env("ROLA_DATA_HOME", &self.data)
            .env("ROLA_HOME", &self.rola_home)
            .env("ROLA_LANG", EN)
            .env("APP_LANG", EN)
            .env("LANG", EN)
            .env("LC_ALL", EN)
            .args(args);

        command
    }

    /// Runs the program in `dir` and collects what it did.
    fn run(&self, dir: &Path, args: &[&str]) -> Ran {
        run(&mut self.command(dir, args))
    }
}

/// The version the Layout names for `main`, as `layout entries` writes it.
fn layout_version(rola: &Rola, ws: &Path) -> Option<String> {
    let ran = rola.run(ws, &["layout", "entries"]);

    ran.stdout
        .lines()
        .find(|line| line.split_whitespace().next() == Some("main"))
        .and_then(|line| {
            line.split_whitespace()
                .find(|token| token.len() == 64)
                .map(str::to_owned)
        })
}

/// What a run printed, read as JSON, when it printed JSON at all.
fn json(ran: &Ran) -> Option<serde_json::Value> {
    serde_json::from_str(ran.stdout.trim()).ok()
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

/// What was asked of the program, and which of the questions were not answered as they should be.
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
            self.wrong.push(format!("{question}: {why}"));
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

    /// Says what was asked and what was wrong with the answers, and ends the program on them.
    fn report(self) {
        for said in &self.wrong {
            eprintln!("FAIL: {said}");
        }

        if self.wrong.is_empty() {
            println!("{0} checks passed", self.asked);
        } else {
            eprintln!("{0} of {1} checks failed", self.wrong.len(), self.asked);
            std::process::exit(1);
        }
    }
}
