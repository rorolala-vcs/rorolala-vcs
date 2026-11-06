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
    missing_tree(&rola, &ws, &mut checked);
    workspace_rows(&rola, &ws, &mut checked);
    mixed_track(&rola, &ws, &mut checked);
    desc_editor(&rola, &ws, &mut checked);
    reset_staging(&rola, &ws, &mut checked);
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

    let ran = rola.run(ws, &["status", "main"]);
    checked.exits("a named tree is looked back from its version", &ran, 0);
    checked.stdout_has("the chain draws the recorded version", &ran, "V0");
    checked.wants(
        "a clean tree has no draft above the chain",
        !ran.stdout.contains("??"),
        "a draft is drawn for a clean tree",
    );

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
    checked.exits("a named tree is looked back from its version", &ran, 0);
    checked.stdout_has("the chain draws the recorded version", &ran, "V0");
    checked.stdout_has("an unrecorded change is a draft above the chain", &ran, "??");
    checked.wants(
        "a named tree is not read as the tree's own status",
        !ran.stdout.contains("Not staged:"),
        "the named tree drew the tree's own reading",
    );

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

/// A tree whose directory was deleted is put back whole by `align --restore`.
fn missing_tree(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let dir = ws.join("main");
    fs::remove_dir_all(&dir).expect("the tree's directory is gone");

    let ran = rola.run(ws, &["status"]);
    checked.exits("the Workspace reads the lost tree", &ran, 0);
    checked.stdout_has("the lost tree needs confirming", &ran, "structural");

    let ran = rola.run(ws, &["align", "main", "--restore"]);
    checked.exits("the whole tree is restored from the Layout", &ran, 0);
    checked.wants(
        "the tree's mark is back",
        dir.join(".rola-tree").is_dir(),
        "the mark is not there",
    );
    checked.wants(
        "the tree's files are back",
        dir.join("a.psd").exists(),
        "a file is not there",
    );
    checked.wants(
        "the tree's path is a directory again",
        dir.is_dir(),
        "the path is not a directory",
    );

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the restored tree is read", &ran, 0);
    checked.stdout_has("the restored tree is clean", &ran, "clean");

    // A path inside a tree names the tree here too: the restore puts the whole of it back.
    fs::remove_dir_all(&dir).expect("the tree's directory is gone again");
    let ran = rola.run(ws, &["align", "main/a.psd", "--restore"]);
    checked.exits("a path inside a lost tree restores the tree", &ran, 0);
    checked.wants(
        "the tree is whole again from a path inside it",
        dir.join("a.psd").exists() && dir.join(".rola-tree").is_dir(),
        "the tree was not put back",
    );

    let ran = rola.run(ws, &["status"]);
    checked.exits("the Workspace is read after the restore", &ran, 0);
    checked.wants(
        "the Workspace has nothing left to confirm",
        !ran.stdout.contains("structural"),
        "a structural change is still read",
    );
}

/// A tree's work is drawn among the Workspace's own rows, one line, before the files.
fn workspace_rows(rola: &Rola, ws: &Path, checked: &mut Checked) {
    fs::write(ws.join("main").join("a.psd"), b"unrecorded").expect("work in the tree");
    fs::create_dir_all(ws.join("extra")).expect("a directory for another tree");
    fs::write(ws.join("extra").join("x.psd"), b"x").expect("a file in it");
    fs::write(ws.join("z.txt"), b"z").expect("a file beside the trees");

    let ran = rola.run(ws, &["tree", "init", "extra"]);
    checked.exits("a second tree is made", &ran, 0);

    let ran = rola.run(ws, &["status"]);
    checked.exits("the Workspace is read", &ran, 0);
    checked.stdout_has("the added tree is a structural change", &ran, "+ extra (tree object)");
    checked.stdout_has("the tree's work is one content line", &ran, "main (1 not staged)");
    checked.wants(
        "trees are drawn before files",
        ran.stdout.find("extra").zip(ran.stdout.find("z.txt")).is_some_and(|(tree, file)| tree < file),
        "the tree is not drawn before the file",
    );
    checked.wants(
        "no separate tree block is drawn",
        !ran.stdout.contains("Trees:"),
        "the separate tree block is still there",
    );

    // The row is about the tree's work as a whole: a clean tree is no line at all.
    let ran = rola.run(ws, &["align", "main", "--restore"]);
    checked.exits("the tree's work is put back", &ran, 0);

    let ran = rola.run(ws, &["status"]);
    checked.exits("the Workspace is read after the restore", &ran, 0);
    checked.wants(
        "the clean tree is no line",
        !ran.stdout.contains("main ("),
        "the clean tree is still drawn",
    );
}

/// A tree is tracked in the same list as a file, with the same messages and the same order.
fn mixed_track(rola: &Rola, ws: &Path, checked: &mut Checked) {
    fs::write(ws.join("main").join("a.psd"), b"tree work").expect("work in the tree");
    fs::write(ws.join("beside.psd"), b"file work").expect("a file beside the tree");

    let ran = rola.run(ws, &["tree", "add", "main"]);
    checked.exits("the tree's work is staged", &ran, 0);

    let ran = rola.run(
        ws,
        &[
            "track",
            "main",
            "beside.psd",
            "--message",
            "group",
            "--file-message",
            "tree words",
            "--file-message",
            "file words",
        ],
    );
    checked.exits("a tree and a file are tracked together", &ran, 0);
    checked.wants(
        "the run reports them in the order it named them",
        ran.stdout.find("main").zip(ran.stdout.find("beside.psd")).is_some_and(|(tree, file)| tree < file),
        "the tree is not reported before the file",
    );

    let ran = rola.run(ws, &["status", "main"]);
    checked.exits("the tree's chain is read", &ran, 0);
    checked.stdout_has(
        "the tree carries its own message",
        &ran,
        "group: tree words",
    );

    let ran = rola.run(ws, &["status", "beside.psd"]);
    checked.exits("the file's chain is read", &ran, 0);
    checked.stdout_has(
        "the file carries its own message",
        &ran,
        "group: file words",
    );
}

/// An editor that writes `text` into the file it is given and ends with `code`.
///
/// The editor is the one `rola track` and `rola tree desc` open: a program and its arguments, with
/// the file last, so a script beside the sandbox is enough to act one out.
fn fake_editor(place: &Path, text: &str, code: i32) -> String {
    #[cfg(windows)]
    {
        let script = place.join("__fake_editor.bat");
        let written = text.replace('%', "%%");
        fs::write(
            &script,
            format!("@echo off\r\n> \"%~1\" echo {written}\r\nexit /b {code}\r\n"),
        )
        .expect("a fake editor");

        format!("cmd /c {}", script.display())
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let script = place.join("__fake_editor.sh");
        fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s' '{text}' > \"$1\"\nexit {code}\n"),
        )
        .expect("a fake editor");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("an executable editor");

        format!("sh {}", script.display())
    }
}

/// `tree desc` opens the same editing file as `track`, and stages what it wrote only after success.
fn desc_editor(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let place = ws.parent().expect("the sandbox");
    let tree = ws.join("main");
    let staged = tree.join(".rola-tree").join("description");
    let read = || fs::read_to_string(&staged).ok();

    let ran = rola.run(&tree, &["tree", "write-desc", "old staged"]);
    checked.exits("a description is staged without an editor", &ran, 0);
    checked.wants(
        "the staged description is what was written",
        read().as_deref() == Some("old staged"),
        "the staged description is not what was written",
    );

    let editor = fake_editor(place, "cancelled", 3);
    let ran = rola.run(&tree, &["tree", "desc", ".", "--editor", &editor]);
    checked.exits("an editor that fails fails the run", &ran, 210);
    checked.wants(
        "a failed editor leaves the staged description",
        read().as_deref() == Some("old staged"),
        "a failed editor changed the staged description",
    );

    let editor = fake_editor(place, "edited description", 0);
    let ran = rola.run(&tree, &["tree", "desc", ".", "--editor", &editor]);
    checked.exits("an editor that ends well writes the description", &ran, 0);
    checked.wants(
        "what the editor wrote is what is staged",
        read().as_deref() == Some("edited description"),
        "the staged description is not what the editor wrote",
    );

    let ran = rola.run(&tree, &["track", ".", "--message", "the description editor"]);
    checked.exits("the tree records the description", &ran, 0);

    let ran = rola.run(ws, &["tree", "cat-desc", "main"]);
    checked.exits("the description is read back", &ran, 0);
    checked.stdout_has("the recorded description is the edited one", &ran, "edited description");
}

/// `tree reset` puts staged work back to the recorded version, for the tree or the paths named.
fn reset_staging(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let tree = ws.join("main");
    let staged = tree.join(".rola-tree").join("description");

    fs::write(tree.join("a.psd"), b"staged work").expect("work in the tree");
    fs::write(tree.join("added.psd"), b"new").expect("a file that is not recorded");

    let ran = rola.run(&tree, &["tree", "add", "."]);
    checked.exits("the work is staged", &ran, 0);

    let ran = rola.run(&tree, &["tree", "reset"]);
    checked.exits("the whole staging area is reset", &ran, 0);
    checked.stdout_has("the run says the staging area is back", &ran, "back to the recorded version");

    let ran = rola.run(ws, &["tree", "status", "main"]);
    checked.exits("the tree is read after the whole reset", &ran, 0);
    checked.stdout_has("the reset work is not staged", &ran, "Not staged:");
    checked.stdout_has("the unrecorded file is untracked", &ran, "Untracked:");

    let ran = rola.run(&tree, &["tree", "add", "."]);
    checked.exits("the work is staged again", &ran, 0);

    let ran = rola.run(ws, &["tree", "reset", "main/a.psd"]);
    checked.exits("one path is reset", &ran, 0);
    checked.stdout_has("the run names the path", &ran, "reset a.psd");

    let ran = rola.run(ws, &["tree", "diff-stat", "--staged", "main"]);
    checked.exits("the staged structure is compared", &ran, 0);
    checked.stdout_has("the other staged path is left staged", &ran, "added.psd");
    checked.wants(
        "the reset path is no longer staged",
        !ran.stdout.contains("a.psd"),
        "the reset path is still staged",
    );

    let ran = rola.run(&tree, &["tree", "write-desc", "soon gone"]);
    checked.exits("a description is staged", &ran, 0);
    checked.wants("the staged description is there", staged.is_file(), "no staged description");

    let ran = rola.run(&tree, &["tree", "reset"]);
    checked.exits("the whole reset clears the description", &ran, 0);
    checked.wants(
        "the staged description is gone",
        !staged.is_file(),
        "the staged description is still there",
    );

    let ran = rola.run(ws, &["tree", "reset", "main/not-there.psd"]);
    checked.exits("a path the tree does not know is refused", &ran, 94);
    checked.stderr_has("the refusal says the path cannot be reset", &ran, "cannot be reset");

    // Put the work back so the refusals after this run the tree they expect.
    fs::write(tree.join("a.psd"), b"tree work").expect("the recorded work");
    fs::remove_file(tree.join("added.psd")).expect("the unrecorded file");
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
