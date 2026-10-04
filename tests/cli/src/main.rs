//! `cli`: what the `rola` program does when someone runs it.
//!
//! A program rather than a set of tests cargo runs, because what it checks is a program a caller
//! meets: the built `rola`, started the way a terminal starts it, told things and read back. What
//! is checked is what it exits with and what it says on each of the two streams — never how the
//! library under it is shaped, which is the business of the module suites.
//!
//! Everything happens inside one sandbox, and the places the program keeps for the user are
//! pointed into it as well: `XDG_DATA_HOME`, `HOME` and `ROLA_HOME` are the sandbox's own, so a
//! run here reads and writes nothing of the machine's. The language is pinned to English, since
//! what is read back is text; the check that is about the language says so itself.
//!
//! The commands are grouped the way a caller meets them: what the program is, making a place to
//! work, the local work that needs no Vault, the user's own keys, and the ways a run fails.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rorolala_utils_sandbox::{Guard, Ran, command, run};

/// The language every run is made in, so what is read back is a known text.
const EN: &str = "en";

fn main() {
    let sandbox = Guard::new("cli");

    let home = sandbox.join("home");
    let data = sandbox.join("data");
    let rola_home = sandbox.join("rola-home");
    let empty = sandbox.join("empty");
    let plain = sandbox.join("plain");
    let initme = sandbox.join("initme");
    let files = sandbox.join("files");
    let ws = sandbox.join("ws");
    let bindme = sandbox.join("bindme");
    let vault = sandbox.join("vault");
    let held = sandbox.join("held");
    let claimed = sandbox.join("claimed");

    for dir in [
        &home, &data, &rola_home, &empty, &plain, &initme, &files, &held, &claimed,
    ] {
        fs::create_dir_all(dir).expect("a sandbox directory");
    }

    let rola = Rola {
        home,
        data,
        rola_home,
    };

    let mut checked = Checked::default();

    identity(&rola, &empty, &mut checked);
    creation(&rola, &empty, &initme, &ws, &vault, &mut checked);
    placement(&rola, &empty, &vault, &ws, &mut checked);
    binding(&rola, &empty, &bindme, &mut checked);
    workspace(&rola, &ws, &mut checked);
    layouts(&rola, &ws, &vault, &mut checked);
    storage(&rola, &ws, &files, &mut checked);
    index(&rola, &ws, &mut checked);
    movement(&rola, &plain, &files, &mut checked);
    accounts(&rola, &plain, &ws, &mut checked);
    lookback(&rola, &ws, &mut checked);
    merging(&rola, &ws, &files, &mut checked);
    vaults(&rola, &ws, &mut checked);
    explain(&rola, &ws, &mut checked);
    output(&rola, &ws, &mut checked);
    language(&rola, &ws, &mut checked);
    held_by_another(&rola, &held, &mut checked);
    ownership(&rola, &claimed, &mut checked);

    checked.report();
}

/// What the program says it is, and what it does with a run that names nothing.
fn identity(rola: &Rola, dir: &Path, checked: &mut Checked) {
    let ran = rola.run(dir, &["--version"]);
    checked.exits("`--version` ends successfully", &ran, 0);
    checked.stdout_has(
        "`--version` draws the banner and the build",
        &ran,
        "Rorolala",
    );
    checked.stdout_has("the version line is there", &ran, "rola ");

    let ran = rola.run(dir, &["-V"]);
    checked.exits("`-V` ends successfully", &ran, 0);

    // The version is a command's result, so the output settings apply to it as they do to any
    // other: what `--json` answers with is the reference the build wrote, not the banner.
    let ran = rola.run(dir, &["--json", "-V"]);
    checked.exits("`--json -V` ends successfully", &ran, 0);
    checked.wants(
        "`--json -V` answers with the build's own fields",
        json(&ran).is_some_and(|value| {
            value.get("version").is_some()
                && value.get("commit_hash").is_some()
                && value.get("rustc_version").is_some()
        }),
        &said(&ran),
    );

    let ran = rola.run(dir, &[]);
    checked.exits("a run that names nothing ends with the help code", &ran, 2);
    checked.stdout_empty("a run that names nothing writes no result", &ran);
    checked.stderr_has("the help is what it is shown", &ran, "rola");

    let ran = rola.run(dir, &["-h"]);
    checked.exits("`-h` shows the help", &ran, 2);
    checked.stderr_has("the help lists the commands", &ran, "create");

    let ran = rola.run(dir, &["bogus"]);
    checked.exits("a word that names no command is refused", &ran, 12);
    checked.stderr_has("the refusal names what was typed", &ran, "bogus");
    checked.stdout_empty("the refusal is not a result", &ran);
}

/// Making a place to work: a Workspace, a Vault, and the refusals of both.
fn creation(
    rola: &Rola,
    empty: &Path,
    initme: &Path,
    ws: &Path,
    vault: &Path,
    checked: &mut Checked,
) {
    let ran = rola.run(empty, &["create", &text(ws)]);
    checked.exits("a Workspace is made", &ran, 0);
    checked.stdout_has("the creation names where it was made", &ran, &text(ws));
    checked.wants(
        "the Workspace holds its data directory",
        ws.join(".rola/workspace.toml").is_file(),
        &said(&ran),
    );

    let ran = rola.run(empty, &["create", &text(ws)]);
    checked.exits("a Workspace that is already there is refused", &ran, 10);
    checked.stderr_has(
        "the refusal says it is already there",
        &ran,
        "already exists",
    );
    checked.stdout_empty("the refusal is not a result", &ran);

    let ran = rola.run(empty, &["create"]);
    checked.exits("a creation with no path is refused", &ran, 22);
    checked.stderr_has(
        "the refusal says what is missing",
        &ran,
        "Path not provided",
    );
    checked.stdout_empty("a refusal is not written to stdout", &ran);

    let ran = rola.run(initme, &["init"]);
    checked.exits("`init` makes a Workspace where it is run", &ran, 0);
    checked.wants(
        "the Workspace is made in the current directory",
        initme.join(".rola/workspace.toml").is_file(),
        &said(&ran),
    );

    let ran = rola.run(empty, &["--vault", "create", &text(vault)]);
    checked.exits("a Vault is made", &ran, 0);
    checked.stdout_has("the creation names where it was made", &ran, &text(vault));
    checked.wants(
        "the Vault holds its configuration",
        vault.join("vault.toml").is_file(),
        &said(&ran),
    );

    let ran = rola.run(empty, &["--vault", "create", &text(vault)]);
    checked.exits("a Vault that is already there is refused", &ran, 10);
    checked.stderr_has(
        "the refusal says it is already there",
        &ran,
        "already exists",
    );

    // A Vault is what others sit inside, so one asked for from within a Vault is made under its
    // `vaults/` and named rather than pathed.
    let ran = rola.run(vault, &["--vault", "create", "alpha"]);
    checked.exits("a Vault inside a Vault is made by name", &ran, 0);
    checked.stdout_has("the made Vault is named under vaults", &ran, "vaults/alpha");

    let ran = rola.run(vault, &["--vault", "create", "alpha"]);
    checked.exits(
        "a Vault inside a Vault is refused the second time",
        &ran,
        10,
    );

    let ran = rola.run(vault, &["--vault", "create", &text(vault)]);
    checked.exits("a path does not name a Vault inside a Vault", &ran, 22);
}

/// Where a run has to be: inside a Workspace for some commands, inside a Vault for others.
fn placement(rola: &Rola, empty: &Path, vault: &Path, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(empty, &["status"]);
    checked.exits("a command that needs a place says there is none", &ran, 190);

    let ran = rola.run(vault, &["--vault", "status"]);
    checked.exits(
        "a command that works on a Workspace is refused inside a Vault",
        &ran,
        30,
    );
    checked.stderr_has("the refusal says where to run it", &ran, "Workspace");

    let ran = rola.run(empty, &["--workspace-dir", &text(ws), "layout", "ls"]);
    checked.exits(
        "a Workspace is reached from outside by `--workspace-dir`",
        &ran,
        0,
    );
}

/// The local work a Workspace does: what is unrecorded, and what is asked of it before an
/// account is named.
fn workspace(rola: &Rola, ws: &Path, checked: &mut Checked) {
    // A Workspace is made without a Layout, since a name is the caller's to choose; one that has
    // none cannot be worked in, and says so rather than answering as if it held nothing.
    let ran = rola.run(ws, &["status"]);
    checked.exits("a Workspace with no Layout cannot be worked in", &ran, 194);
    checked.stderr_has(
        "the refusal says there is no Layout",
        &ran,
        "no Layout to work in",
    );
    checked.stdout_empty("the refusal is not a result", &ran);

    let ran = rola.run(ws, &["layout", "ls"]);
    checked.exits("a Workspace with no Layout still lists", &ran, 0);
    checked.stdout_empty("a Workspace with no Layout lists nothing", &ran);

    // Making the first Layout is also choosing what is worked in: there is nothing else it could
    // be, and a Workspace nothing is worked in is one no local command can act on.
    let ran = rola.run(ws, &["layout", "new", "main"]);
    checked.exits("the first Layout is made", &ran, 0);
    checked.stdout_has("the made Layout is named", &ran, "main");

    let ran = rola.run(ws, &["status"]);
    checked.exits("a fresh Workspace has nothing unrecorded", &ran, 0);
    checked.stdout_has("it says so", &ran, "nothing unrecorded");

    let ran = rola.run(ws, &["status", "--json"]);
    checked.exits("`status --json` ends successfully", &ran, 0);
    checked.wants(
        "the answer is the five lists a status has",
        json(&ran).is_some_and(|value| {
            ["lost", "untagged", "modified", "renamed", "unowned"]
                .iter()
                .all(|key| value.get(key).is_some())
        }),
        &said(&ran),
    );

    let ran = rola.run(ws, &["entries", "--json"]);
    checked.exits("a Layout's entries are listed", &ran, 0);
    checked.wants(
        "an empty Layout lists no entries",
        json(&ran).is_some_and(|value| {
            value
                .get("entries")
                .is_some_and(serde_json::Value::is_array)
        }),
        &said(&ran),
    );

    // A file that is there and not recorded is what a status is for.
    let file = ws.join("file.txt");
    fs::write(&file, "work\n").expect("a file to work on");

    let ran = rola.run(ws, &["status"]);
    checked.exits("a Workspace with an unrecorded file still answers", &ran, 0);
    checked.stdout_has("the unrecorded file is named", &ran, "file.txt");

    // Recording a file is acting as someone, so without an account it is refused before anything
    // is read or written.
    let ran = rola.run(ws, &["track", &text(&file)]);
    checked.exits("tracking without an account is refused", &ran, 62);
    checked.stderr_has("the refusal says what is missing", &ran, "account");

    let ran = rola.run(ws, &["retrack", &text(&file)]);
    checked.exits(
        "retracking a path the Layout does not name is refused",
        &ran,
        251,
    );

    let ran = rola.run(ws, &["pack"]);
    checked.exits("packing what the store holds is made", &ran, 0);
}

/// Binding a Vault address as a name, which is what puts a fresh Workspace to work.
fn binding(rola: &Rola, empty: &Path, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(empty, &["create", &text(ws)]);
    checked.exits("a Workspace to bind into is made", &ran, 0);

    let ran = rola.run(ws, &["bind"]);
    checked.exits("binding with no address is refused", &ran, 70);
    checked.stderr_has("the refusal says what is missing", &ran, "address");

    // A name a Layout already holds is refused before anything is written, so what the Workspace
    // was is what it still is.
    let ran = rola.run(ws, &["layout", "new", "main"]);
    checked.exits("a Layout is made to stand in the way", &ran, 0);

    let ran = rola.run(ws, &["bind", "rola://127.0.0.1:7000/", "main"]);
    checked.exits("a name a Layout holds is refused", &ran, 10);
    checked.stderr_has("the refusal says the name is taken", &ran, "already");
    checked.stdout_empty("the refusal is not a result", &ran);
    checked.wants(
        "the refused address was not written down",
        !fs::read_to_string(ws.join(".rola/workspace.toml"))
            .unwrap_or_default()
            .contains("7000"),
        &said(&ran),
    );

    // Everything after this is about a Workspace that has nothing in its way, so it is made
    // again from nothing.
    fs::remove_dir_all(ws).expect("the Workspace with the standing Layout");
    let ran = rola.run(empty, &["create", &text(ws)]);
    checked.exits("a Workspace with nothing in the way is made", &ran, 0);

    // The name is one thing said once: the Vault is bound under it and a Layout by it is made, and
    // the Workspace reaches for the Vault and works in the Layout.
    let ran = rola.run(ws, &["bind", "rola://127.0.0.1:7001/"]);
    checked.exits("an address is bound as `origin`", &ran, 0);
    checked.stdout_has("the binding names the name", &ran, "origin");
    checked.stdout_has("the binding names the address", &ran, "127.0.0.1:7001");
    checked.stdout_has("it says the Layout is worked in", &ran, "worked in");
    checked.wants(
        "the Layout that was made is the one checked out",
        fs::read_to_string(ws.join(".rola/LAYOUT")).is_ok_and(|it| it.trim() == "origin"),
        &said(&ran),
    );
    checked.wants(
        "the Layout tracks the Vault of the same name",
        fs::read_to_string(ws.join(".rola/layouts/origin/TRACK"))
            .is_ok_and(|it| it.trim() == "origin"),
        &said(&ran),
    );

    let ran = rola.run(ws, &["vault"]);
    checked.exits("the bound Vault is listed", &ran, 0);
    checked.stdout_has("the bound Vault is the one reached for", &ran, "origin");

    let ran = rola.run(ws, &["layout", "ls", "--json"]);
    checked.exits("the Layouts are listed as data", &ran, 0);
    checked.wants(
        "the Layout being worked in is marked",
        json(&ran).is_some_and(|value| {
            layout_of(&value, "origin").and_then(|item| item.get("is_current"))
                == Some(&serde_json::Value::Bool(true))
        }),
        &said(&ran),
    );

    let ran = rola.run(ws, &["status"]);
    checked.exits("the Workspace is now one that can be worked in", &ran, 0);

    // Binding it again under the same name is how the address is changed, the same as
    // `rola vault bind`: the Layout that tracks it is the one this binding made, so it is the same
    // binding said again rather than a name in the way.
    let ran = rola.run(ws, &["bind", "rola://127.0.0.1:7002/"]);
    checked.exits("binding a name that is there changes it", &ran, 0);
    checked.stdout_has("the new address is named", &ran, "127.0.0.1:7002");
    checked.wants(
        "the changed address is written down",
        fs::read_to_string(ws.join(".rola/workspace.toml")).is_ok_and(|it| it.contains("7002")),
        &said(&ran),
    );

    // A name is normalised the way a Layout's is, so what is made is the kebab-case form of what
    // was written.
    let ran = rola.run(ws, &["bind", "rola://127.0.0.1:7004/", "Up Stream"]);
    checked.exits("a name is normalised", &ran, 0);
    checked.stdout_has("the normalised name is named", &ran, "up-stream");
    checked.wants(
        "the Layout is made under the normalised name",
        ws.join(".rola/layouts/up-stream").is_dir(),
        &said(&ran),
    );

    // A name that holds no name at all is refused: it cannot be a directory. The word is one the
    // parser reads as a positional, so what is refused is the name rather than a flag in its place.
    let ran = rola.run(ws, &["bind", "rola://127.0.0.1:7005/", "."]);
    checked.exits("a name that is not one is refused", &ran, 190);
    checked.stderr_has("the refusal says it is not a name", &ran, "name a Layout");
    checked.stdout_empty("the refusal is not a result", &ran);
    checked.wants(
        "the refused address was not written down",
        !fs::read_to_string(ws.join(".rola/workspace.toml"))
            .unwrap_or_default()
            .contains("7005"),
        &said(&ran),
    );

    // An address that will not read is refused the way `rola vault bind` refuses one.
    let ran = rola.run(ws, &["bind", "not an address", "other"]);
    checked.exits("an address that is not one is refused", &ran, 70);
    checked.stderr_has(
        "the refusal says it is not an address",
        &ran,
        "not an address",
    );

    let ran = rola.run(empty, &["bind", "rola://127.0.0.1:7006/"]);
    checked.exits("binding outside a Workspace is refused", &ran, 11);
    checked.stdout_empty("the refusal is not a result", &ran);

    // The Layout the binding made may be left unworked-in, which is what `--no-set-layout` asks
    // for; the Workspace then has one and works in none, which is the state a later command says.
    fs::remove_dir_all(ws).expect("the Workspace that was bound into");
    let ran = rola.run(empty, &["create", &text(ws)]);
    checked.exits("a third Workspace is made", &ran, 0);

    let ran = rola.run(ws, &["bind", "rola://127.0.0.1:7007/", "--no-set-layout"]);
    checked.exits("a binding may leave the work where it is", &ran, 0);
    checked.wants(
        "no Layout is checked out",
        !ws.join(".rola/LAYOUT").is_file(),
        &said(&ran),
    );

    let ran = rola.run(ws, &["status"]);
    checked.exits(
        "the Workspace with a Layout it does not work in says so",
        &ran,
        194,
    );

    // `--no-set-default-vault` binds the address without choosing the Vault to reach for.
    fs::remove_dir_all(ws).expect("the Workspace that was not worked in");
    let ran = rola.run(empty, &["create", &text(ws)]);
    checked.exits("a fourth Workspace is made", &ran, 0);

    let ran = rola.run(
        ws,
        &["bind", "rola://127.0.0.1:7008/", "--no-set-default-vault"],
    );
    checked.exits("a binding may leave the default Vault alone", &ran, 0);
    checked.stdout_has("the Layout is still worked in", &ran, "worked in");

    let ran = rola.run(ws, &["vault", "default"]);
    checked.exits("no default Vault was chosen", &ran, 0);
    checked.stdout_empty("nothing is named as the default Vault", &ran);
}

/// The Layouts a Workspace holds, and a Vault's one.
fn layouts(rola: &Rola, ws: &Path, vault: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["layout", "ls"]);
    checked.exits("the Layouts are listed", &ran, 0);
    checked.stdout_has("the Layout made first is there", &ran, "main");

    let ran = rola.run(ws, &["layout", "new", "extra"]);
    checked.exits("a Layout is made", &ran, 0);
    checked.stdout_has("the made Layout is named", &ran, "extra");

    // A Layout made beside the one being worked in is not the one being worked in: choosing one is
    // what `force-switch` is for, and making one must not choose it behind the caller's back.
    let ran = rola.run(ws, &["layout", "ls", "--json"]);
    checked.wants(
        "making a Layout leaves the work where it was",
        json(&ran).is_some_and(|value| {
            layout_of(&value, "main").and_then(|item| item.get("is_current"))
                == Some(&serde_json::Value::Bool(true))
        }),
        &said(&ran),
    );

    let ran = rola.run(ws, &["layout", "new", "extra"]);
    checked.exits("a Layout whose name is taken is refused", &ran, 10);
    checked.stderr_has("the refusal says the name is taken", &ran, "already");

    let ran = rola.run(ws, &["layout", "new", "bad name"]);
    checked.exits("a name a Layout may not be given is refused", &ran, 190);

    // Taking a Layout away asks first, and with no terminal there is no one to answer.
    let ran = rola.run(ws, &["layout", "rm", "extra"]);
    checked.exits("an unconfirmed removal is not a failure", &ran, 0);
    checked.stdout_has(
        "an unconfirmed removal says nothing was done",
        &ran,
        "Nothing was done",
    );
    checked.wants(
        "the Layout is still there",
        ws.join(".rola/layouts/extra").is_dir(),
        &said(&ran),
    );

    let ran = rola.run(ws, &["layout", "rm", "extra", "--confirm"]);
    checked.exits("a confirmed removal is made", &ran, 0);
    checked.wants(
        "the Layout is gone",
        !ws.join(".rola/layouts/extra").is_dir(),
        &said(&ran),
    );

    let ran = rola.run(ws, &["layout", "rm", "main"]);
    checked.exits("the Layout being worked in is not taken away", &ran, 190);

    let ran = rola.run(ws, &["layout", "new", "second"]);
    checked.exits("another Layout is made", &ran, 0);

    let ran = rola.run(ws, &["layout", "force-switch", "second"]);
    checked.exits("the work moves to another Layout", &ran, 0);

    let ran = rola.run(ws, &["layout", "ls", "--json"]);
    checked.exits("the Layouts are listed as data", &ran, 0);
    checked.wants(
        "the listing says which Layout is checked out",
        json(&ran).is_some_and(|value| {
            layout_of(&value, "second").and_then(|item| item.get("is_current"))
                == Some(&serde_json::Value::Bool(true))
        }),
        &said(&ran),
    );

    let ran = rola.run(ws, &["layout", "force-switch", "main"]);
    checked.exits("the work moves back", &ran, 0);

    let ran = rola.run(ws, &["layout", "rm", "second", "--confirm"]);
    checked.exits("the other Layout is taken away", &ran, 0);

    let ran = rola.run(ws, &["layout", "entry", "create"]);
    checked.exits(
        "a layout command without its arguments is refused",
        &ran,
        191,
    );

    let ran = rola.run(vault, &["--vault", "layout", "ls"]);
    checked.exits("a Vault's one Layout is listed", &ran, 0);
    checked.stdout_has("the Vault's Layout is named truth", &ran, "truth");
}

/// The store a Workspace keeps, written to and read back by hand.
fn storage(rola: &Rola, ws: &Path, files: &Path, checked: &mut Checked) {
    let source = files.join("a.txt");
    fs::write(&source, "hello storage\n").expect("a file to store");

    let ran = rola.run(ws, &["storage", "ls-storaged"]);
    checked.exits("a fresh store is listed", &ran, 0);
    checked.wants(
        "a fresh store holds nothing",
        ran.stdout.trim().is_empty(),
        &said(&ran),
    );

    let ran = rola.run(ws, &["storage", "write-file", &text(&source)]);
    checked.exits("a file's content is stored", &ran, 0);
    let key = ran.stdout.trim().to_owned();
    checked.wants(
        "what is stored is named by its hash",
        key.starts_with("blake3:") && key.len() == "blake3:".len() + 64,
        &said(&ran),
    );

    let ran = rola.run(ws, &["storage", "ls-storaged"]);
    checked.stdout_has("the stored key is listed", &ran, &key);

    let ran = rola.run(ws, &["storage", "ls-storaged", "--json"]);
    checked.wants(
        "the listing is an object of keys",
        json(&ran).is_some_and(|value| value.get("keys").is_some_and(serde_json::Value::is_array)),
        &said(&ran),
    );

    let ran = rola.run(ws, &["storage", "write-file", &text(&source), "--json"]);
    checked.exits("storing answers as data when asked", &ran, 0);
    checked.wants(
        "the answer names the hash and whether it was cut",
        json(&ran)
            .is_some_and(|value| value.get("hash").is_some() && value.get("chunked").is_some()),
        &said(&ran),
    );

    let ran = rola.run(
        ws,
        &["storage", "write-file", &text(&files.join("missing.txt"))],
    );
    checked.exits("a path that is not a file is refused", &ran, 82);
    checked.stderr_has("the refusal says it is not a file", &ran, "not a file");

    // What is written out is written under its own hash, in the directory named.
    let extracted = files.join("out");
    fs::create_dir_all(&extracted).expect("a directory to write into");
    let ran = rola.run(ws, &["storage", "extract-file", &key, &text(&extracted)]);
    checked.exits("stored content is written out", &ran, 0);
    let written = extracted.join(key.trim_start_matches("blake3:"));
    checked.wants(
        "what was written out is what was stored",
        fs::read_to_string(&written).ok().as_deref() == Some("hello storage\n"),
        &said(&ran),
    );

    // The head of the hash names the same content, since nothing here starts with it.
    let head = &key.trim_start_matches("blake3:")[..8];
    let by_head = files.join("out-by-head");
    fs::create_dir_all(&by_head).expect("a directory to write into");
    let ran = rola.run(ws, &["storage", "extract-file", head, &text(&by_head)]);
    checked.exits("a hash written by its head names the same content", &ran, 0);
    checked.wants(
        "what the head named wrote the same bytes",
        fs::read_to_string(by_head.join(key.trim_start_matches("blake3:")))
            .ok()
            .as_deref()
            == Some("hello storage\n"),
        &said(&ran),
    );

    let ran = rola.run(ws, &["storage", "extract-file", "1234"]);
    checked.exits("a head that names nothing is refused", &ran, 86);
    let ran = rola.run(ws, &["storage", "extract-file", "not-a-hash"]);
    checked.exits("a word that is not a hash at all is refused", &ran, 86);

    let ran = rola.run(ws, &["storage", "extract-file"]);
    checked.exits("extracting without a hash is refused", &ran, 84);
}

/// The version control index, written to and read back by hand.
fn index(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["vcs-index", "ls-str"]);
    checked.exits("a fresh index is listed", &ran, 0);
    checked.wants(
        "a fresh index holds no text objects",
        ran.stdout.trim().is_empty(),
        &said(&ran),
    );

    let ran = rola.run(ws, &["vcs-index", "create-rootver"]);
    checked.exits("the root version is written", &ran, 0);
    let root = ran.stdout.trim().to_owned();
    checked.wants(
        "the root version is named by a hash",
        root.len() == 64 && root.chars().all(|character| character.is_ascii_hexdigit()),
        &said(&ran),
    );

    let ran = rola.run(ws, &["vcs-index", "print-rootver"]);
    checked.exits("the root version is printed", &ran, 0);
    checked.wants(
        "what is printed is what was written",
        ran.stdout.trim() == root,
        &said(&ran),
    );

    let ran = rola.run(ws, &["vcs-index", "write-creator", "artist"]);
    checked.exits("a Creator is written", &ran, 0);
    let creator = ran.stdout.trim().to_owned();

    let ran = rola.run(ws, &["vcs-index", "write-msg", "first cut"]);
    checked.exits("a Message is written", &ran, 0);
    let message = ran.stdout.trim().to_owned();

    let ran = rola.run(ws, &["vcs-index", "ls-str"]);
    checked.exits("the text objects are listed", &ran, 0);
    checked.stdout_has("the Creator is listed", &ran, &creator);
    checked.stdout_has("the Message is listed", &ran, &message);

    let ran = rola.run(ws, &["vcs-index", "ls-versions"]);
    checked.stdout_has("the root version is listed among the versions", &ran, &root);

    let ran = rola.run(ws, &["vcs-index", "read", &root]);
    checked.exits("one index object is read back", &ran, 0);
    checked.stdout_has("what is read names the object", &ran, &root);

    // The head of the hash reads the same object, since only one here starts with it.
    let ran = rola.run(ws, &["vcs-index", "read", &root[..8]]);
    checked.exits("an index object is read by the head of its hash", &ran, 0);
    checked.stdout_has("what the head read names the object", &ran, &root);

    let ran = rola.run(ws, &["vcs-index", "read", "1234"]);
    checked.exits(
        "a head that names nothing in the index is refused",
        &ran,
        123,
    );
}

/// Moving, copying and removing a path by hand.
fn movement(rola: &Rola, plain: &Path, files: &Path, checked: &mut Checked) {
    let one = files.join("one.txt");
    let two = files.join("two.txt");
    let three = files.join("three.txt");
    fs::write(&one, "one\n").expect("a file to move about");

    let ran = rola.run(plain, &["fs-ops", "cp", &text(&one), &text(&two)]);
    checked.exits("a path is copied", &ran, 0);
    checked.wants("the copy is there", two.is_file(), &said(&ran));

    let ran = rola.run(plain, &["fs-ops", "mv", &text(&two), &text(&three)]);
    checked.exits("a path is moved", &ran, 0);
    checked.wants(
        "the moved path is at its new name and not the old one",
        three.is_file() && !two.exists(),
        &said(&ran),
    );

    let ran = rola.run(plain, &["fs-ops", "rm", &text(&three)]);
    checked.exits("a path is removed", &ran, 0);
    checked.wants("the removed path is gone", !three.exists(), &said(&ran));

    let ran = rola.run(plain, &["fs-ops", "cp"]);
    checked.exits("a copy without its paths is refused", &ran, 110);
}

/// The accounts a run can act as, and the key pairs they are.
fn accounts(rola: &Rola, plain: &Path, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["account"]);
    checked.exits("the accounts are listed", &ran, 0);
    checked.stdout_has(
        "a place with no account says so",
        &ran,
        "No account is installed",
    );

    let ran = rola.run(ws, &["account", "--json"]);
    checked.wants(
        "the listing is an object of names and the current one",
        json(&ran)
            .is_some_and(|value| value.get("names").is_some() && value.get("current").is_some()),
        &said(&ran),
    );

    let ran = rola.run(ws, &["account", "nosuch"]);
    checked.exits("a name that is not an account is refused", &ran, 60);
    checked.stderr_has("the refusal names what was asked for", &ran, "nosuch");

    let ran = rola.run(plain, &["key", "generate", "alice"]);
    checked.exits("a key pair is made where the run is", &ran, 0);
    checked.wants(
        "both halves are written",
        plain.join("alice.pem").is_file() && plain.join("alice.pub").is_file(),
        &said(&ran),
    );

    let ran = rola.run(plain, &["key", "generate", "--install", "bob"]);
    checked.exits(
        "a key pair is installed in the user's key directory",
        &ran,
        0,
    );
    let user_keys = rola.data.join("rola/keys");
    checked.wants(
        "the installed pair is under the user's data",
        user_keys.join("bob.pem").is_file() && user_keys.join("bob.pub").is_file(),
        &said(&ran),
    );

    let ran = rola.run(ws, &["key", "--pem"]);
    checked.exits("the accounts' keys are listed", &ran, 0);
    checked.stdout_has("the installed account key is listed", &ran, "bob.pem");

    let ran = rola.run(ws, &["account", "bob"]);
    checked.exits("the work is told which account to act as", &ran, 0);
    checked.stdout_has("the chosen account is named", &ran, "bob");

    let ran = rola.run(ws, &["account", "--json"]);
    checked.wants(
        "the listing says which account is current",
        json(&ran)
            .and_then(|value| {
                value
                    .get("current")
                    .and_then(|current| current.as_str())
                    .map(str::to_owned)
            })
            .as_deref()
            == Some("bob"),
        &said(&ran),
    );
}

/// Looking back from one thing in the tree, named the way a shell names it.
fn lookback(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let models = ws.join("models");
    fs::create_dir_all(&models).expect("a directory to work in");
    fs::write(models.join("hero.psd"), "hero\n").expect("a file to record");

    let ran = rola.run(
        ws,
        &[
            "track",
            "models/hero.psd",
            "--message",
            "hero",
            "--no-editor",
        ],
    );
    checked.exits("a file under a directory is recorded", &ran, 0);

    // A name is completed from where the run was made, so that is where it is read from: a run made
    // above the file and one made beside it name the same file, each the way its shell writes it.
    let ran = rola.run(ws, &["status", "models/hero.psd"]);
    checked.exits("a path from the run's own directory is found", &ran, 0);
    checked.stdout_has("the chain is drawn", &ran, "hero");

    let ran = rola.run(&models, &["status", "hero.psd"]);
    checked.exits("a name read from where the run was made is found", &ran, 0);
    checked.stdout_has("the chain is drawn there too", &ran, "hero");

    // A path written from the Layout's root is what a run elsewhere in the tree would have meant,
    // so it still reads when a run beside the file names it that way.
    let ran = rola.run(&models, &["status", "models/hero.psd"]);
    checked.exits("a path written from the root still reads", &ran, 0);
}

/// A variant waiting to be joined into a file, and the merge that records it.
fn merging(rola: &Rola, ws: &Path, files: &Path, checked: &mut Checked) {
    let work = ws.join("merge");
    fs::create_dir_all(&work).expect("a directory to work in");
    fs::write(work.join("file.txt"), "base\n").expect("a file to merge into");

    let ran = rola.run(
        ws,
        &[
            "track",
            "merge/file.txt",
            "--message",
            "base",
            "--no-editor",
        ],
    );
    checked.exits("a file to merge into is recorded", &ran, 0);

    // A variant made by hand, as one handed over from elsewhere would be, so that nothing of the
    // Vault has to be reached to check it in.
    let incoming = files.join("incoming.bin");
    fs::write(&incoming, "from elsewhere\n").expect("the incoming content");

    let ran = rola.run(ws, &["storage", "write-file", &text(&incoming)]);
    checked.exits("the incoming content is stored", &ran, 0);
    let storage = ran.stdout.trim().to_owned();

    let creator = rola.run(ws, &["vcs-index", "write-creator", "bob"]);
    let creator = creator.stdout.trim().to_owned();
    let message = rola.run(ws, &["vcs-index", "write-msg", "from bob"]);
    let message = message.stdout.trim().to_owned();
    let root = rola.run(ws, &["vcs-index", "print-rootver"]);
    let root = root.stdout.trim().to_owned();

    let ran = rola.run(
        ws,
        &[
            "vcs-index",
            "write-variant",
            &root,
            &storage,
            &creator,
            &message,
        ],
    );
    checked.exits("a variant is written", &ran, 0);
    let variant = ran.stdout.trim().to_owned();

    // A variant based on a version the file's chain does not hold is refused; `--force` is how a
    // run says it knows and means to join it anyway.
    let stranger = "11".repeat(32);
    let ran = rola.run(
        ws,
        &[
            "vcs-index",
            "write-variant",
            &stranger,
            &storage,
            &creator,
            &message,
        ],
    );
    checked.exits("a variant on another history is written", &ran, 0);
    let off_chain = ran.stdout.trim().to_owned();

    let ran = rola.run(ws, &["checkin", &off_chain, "--join", "merge/file.txt"]);
    checked.exits("a variant off the file's chain is refused", &ran, 240);
    checked.stderr_has("the refusal names the way past it", &ran, "--force");

    let ran = rola.run(ws, &["checkin", &variant, "--join", "merge/file.txt"]);
    checked.exits("a variant is checked in for a file", &ran, 0);

    let variant_file = fs::read_dir(&work)
        .expect("the files beside the target")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .find(|name| name != "file.txt")
        .expect("a variant file beside the target");

    let ran = rola.run(ws, &["status"]);
    checked.exits("a merge in progress still answers", &ran, 0);
    checked.stdout_has("the merge is named", &ran, "merge/file.txt");

    let ran = rola.run(ws, &["status", "--json"]);
    checked.wants(
        "the merge is in the data",
        json(&ran).is_some_and(|value| {
            value
                .get("merging")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|merging| !merging.is_empty())
        }),
        &said(&ran),
    );

    // The variant file is not a file of the work: a run that names one records nothing.
    let ran = rola.run(
        ws,
        &["track", &format!("merge/{variant_file}"), "--no-editor"],
    );
    checked.exits("a variant file records nothing", &ran, 0);

    // A variant file moved by hand is written down where it now lies, and the merge goes on.
    let moved = "moved.bin";
    let ran = rola.run(
        ws,
        &[
            "fs-ops",
            "mv",
            &format!("merge/{variant_file}"),
            &format!("merge/{moved}"),
        ],
    );
    checked.exits("a variant file is moved by hand", &ran, 0);

    let ran = rola.run(ws, &["status", "--json"]);
    checked.wants(
        "the merge follows the moved variant file",
        json(&ran).is_some_and(|value| {
            value
                .get("merging")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|merging| {
                    merging.iter().any(|item| {
                        item.get("path").and_then(serde_json::Value::as_str)
                            == Some("merge/moved.bin")
                    })
                })
        }),
        &said(&ran),
    );

    // A move no command of ours made — a shell `mv`, an editor saving elsewhere — is found again by
    // what the file holds, and `rola align --rename` writes down where it went.
    let renamed = "renamed.bin";
    fs::rename(work.join(moved), work.join(renamed)).expect("a move by hand");

    let ran = rola.run(ws, &["status", "--json"]);
    checked.wants(
        "a move no command made is found by content",
        json(&ran).is_some_and(|value| {
            value
                .get("merging")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|merging| {
                    merging.iter().any(|item| {
                        item.get("moved").and_then(serde_json::Value::as_str)
                            == Some("merge/renamed.bin")
                    })
                })
        }),
        &said(&ran),
    );

    let ran = rola.run(ws, &["align", &format!("merge/{moved}"), "--rename"]);
    checked.exits("the move is confirmed", &ran, 0);

    let ran = rola.run(ws, &["status"]);
    checked.stdout_has(
        "the merge names where the file went",
        &ran,
        "merge/renamed.bin",
    );

    // Recording the file the variant waits for joins it and ends the wait.
    fs::write(work.join("file.txt"), "base and theirs\n").expect("the merged work");
    let ran = rola.run(
        ws,
        &[
            "track",
            "merge/file.txt",
            "--message",
            "merged",
            "--no-editor",
        ],
    );
    checked.exits("the merge is recorded", &ran, 0);
    checked.wants(
        "the variant file is gone",
        !work.join(renamed).exists(),
        &said(&ran),
    );
    checked.wants(
        "the merge is over",
        !ws.join(".rola/MERGING").exists(),
        &said(&ran),
    );
}

/// The Vaults a Workspace knows, and what a run that may not reach one does.
fn vaults(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["vault"]);
    checked.exits("the bound Vaults are listed", &ran, 0);
    checked.stdout_has(
        "a Workspace that knows none says so",
        &ran,
        "No Vault is bound",
    );

    let ran = rola.run(ws, &["vault", "bind", "origin", "rola://127.0.0.1:7999/"]);
    checked.exits("a Vault is bound by name", &ran, 0);
    checked.stdout_has("the binding names the Vault", &ran, "origin");

    let ran = rola.run(ws, &["vault"]);
    checked.stdout_has("the bound Vault is listed", &ran, "origin");
    checked.stdout_has("its address is listed", &ran, "127.0.0.1:7999");

    let ran = rola.run(ws, &["vault", "bind"]);
    checked.exits("binding without a name is refused", &ran, 70);

    let ran = rola.run(ws, &["vault", "set-default", "origin"]);
    checked.exits("a default Vault is chosen", &ran, 0);

    // `--offline` is one flag for every command, and what it means differs: a command whose work
    // is the exchange with the Vault is refused, and one that only works locally goes on.
    let ran = rola.run(ws, &["--offline", "vault", "handshake", "origin"]);
    checked.exits(
        "a command that reaches the Vault is refused offline",
        &ran,
        252,
    );
    checked.stderr_has("the refusal names the flag", &ran, "--offline");

    let ran = rola.run(ws, &["--offline", "layout", "fetch", "x@origin"]);
    checked.exits("fetching a Vault's Layout is refused offline", &ran, 252);

    let ran = rola.run(ws, &["--offline", "status"]);
    checked.exits("a local command goes on offline", &ran, 0);

    let ran = rola.run(ws, &["--offline", "storage", "ls-storaged"]);
    checked.exits("a store command goes on offline", &ran, 0);

    let ran = rola.run(ws, &["vault", "unbind", "nosuch"]);
    checked.exits("unbinding a name that is not bound is refused", &ran, 71);

    let ran = rola.run(ws, &["vault", "unbind", "origin"]);
    checked.exits("a bound Vault is unbound", &ran, 0);
}

/// Explaining a number the program left behind.
fn explain(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["explain", "exit-code", "82"]);
    checked.exits("a stated exit code is explained", &ran, 0);
    checked.stdout_has("the explanation names the code", &ran, "82");

    let ran = rola.run(ws, &["explain", "exit-code", "999"]);
    checked.exits("a code the program does not state is refused", &ran, 160);

    let ran = rola.run(ws, &["explain"]);
    checked.exits("`explain` without a command shows its help", &ran, 2);
}

/// How a result is drawn: as data, indented, or through a template.
fn output(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["layout", "ls", "--json"]);
    checked.exits("`--json` ends successfully", &ran, 0);
    checked.wants(
        "`--json` answers with one object",
        json(&ran).is_some(),
        &said(&ran),
    );

    let ran = rola.run(ws, &["layout", "ls", "--json-pretty"]);
    checked.exits("`--json-pretty` ends successfully", &ran, 0);
    checked.wants(
        "`--json-pretty` indents the answer",
        ran.stdout.contains("\n  "),
        &said(&ran),
    );

    let ran = rola.run(ws, &["layout", "ls", "--format", "{{ layouts.name }}"]);
    checked.exits("a result is drawn through a template", &ran, 0);
    checked.wants(
        "the template reads the listing",
        ran.stdout.trim() == "main",
        &said(&ran),
    );

    let ran = rola.run(ws, &["layout", "ls", "--format", "{{ nope }}"]);
    checked.exits(
        "a template naming what the data has not is refused",
        &ran,
        180,
    );

    // A structural renderer comes first, so a run that asked for both is answered with the data
    // and told that the template was left out.
    let ran = rola.run(ws, &["layout", "ls", "--json", "--format", "{{ nope }}"]);
    checked.exits("`--json` wins over `--format`", &ran, 0);
    checked.wants(
        "the answer is still the data",
        json(&ran).is_some(),
        &said(&ran),
    );
    checked.stderr_has("the ignored template is warned about", &ran, "ignored");
}

/// The language a run speaks, which is asked for by argument.
fn language(rola: &Rola, ws: &Path, checked: &mut Checked) {
    let ran = rola.run(ws, &["--lang", "en", "storage", "extract-file"]);
    checked.exits("a failure is the same failure in English", &ran, 84);
    checked.stderr_has("English is spoken when named", &ran, "No hash was named");

    let ran = rola.run(ws, &["--lang", "zh-CN", "storage", "extract-file"]);
    checked.exits("a failure is the same failure in Chinese", &ran, 84);
    checked.wants(
        "Chinese is spoken when named",
        ran.stderr.contains("未指定哈希"),
        &said(&ran),
    );

    let ran = rola.run(ws, &["-l", "zh-CN", "layout", "ls"]);
    checked.exits("the short language flag is taken", &ran, 0);
}

/// Moving and removing a path another account holds.
///
/// Neither is refused, and that is the whole of what is asked here: a file operation is about this
/// tree, and what the Vault's Layout names is changed by recording the change and sending it up, not
/// by a file being put somewhere else or taken away. Ownership was once read here — the fetched copy's
/// `owner` compared with the account the run acts as — and a move was refused for it, which made a
/// browser's drag stop on a file somebody else held while changing nothing the refusal protected.
///
/// The fixture is the shape a `layout fetch` leaves behind: the Workspace's own Layout names the paths,
/// a `TRACK` file names the Vault, and the Vault's fetched copy under the cache holds the `owner`. What
/// is not needed is a Vault: the copy is the Layout's own files, so the same three lookups are made
/// without one being served.
fn held_by_another(rola: &Rola, dir: &Path, checked: &mut Checked) {
    let ran = rola.run(dir, &["init"]);
    checked.exits("a Workspace is made for the held entries", &ran, 0);

    let ran = rola.run(dir, &["layout", "new", "main"]);
    checked.exits("a Layout is made to name them", &ran, 0);

    // Two entries, held by an account this run does not act as: one to move and one to remove.
    let moving = "00000000-0000-0000-0000-000000000001";
    let removing = "00000000-0000-0000-0000-000000000002";
    let version = "11".repeat(32);

    for (uuid, path) in [(moving, "hero.psd"), (removing, "solo.psd")] {
        let ran = rola.run(
            dir,
            &[
                "layout", "entry", "create", uuid, &version, "--owner", "alice",
            ],
        );
        checked.exits(
            &format!("an entry held by another account is written: `{path}`"),
            &ran,
            0,
        );

        let ran = rola.run(dir, &["layout", "path", "create", path, uuid]);
        checked.exits(
            &format!("the entry is named at a path: `{path}`"),
            &ran,
            0,
        );

        fs::write(dir.join(path), "art\n").expect("the file an entry is about");
    }

    // The Vault the Layout tracks, and the copy of its Layout that ownership is read from. The name is
    // written where `layout set-track` would write it rather than bound: a Vault named by a run is one
    // the run fetched from, and what is checked here is read from the copy alone.
    fs::write(dir.join(".rola/layouts/main/TRACK"), "vault\n").expect("the tracked Vault");
    copy_into(
        &dir.join(".rola/layouts/main"),
        &dir.join(".rola/cache/readonly-layouts/vault/truth"),
    );

    let ran = rola.run(dir, &["key", "generate", "--install", "carol"]);
    checked.exits("an account holding neither entry is installed", &ran, 0);

    let ran = rola.run(dir, &["account", "carol"]);
    checked.exits("the work acts as that account", &ran, 0);

    let ran = rola.run(dir, &["fs-ops", "mv", "hero.psd", "hero-moved.psd"]);
    checked.exits("a path another account holds is moved", &ran, 0);
    checked.wants(
        "the held path is at its new name and not the old one",
        dir.join("hero-moved.psd").is_file() && !dir.join("hero.psd").exists(),
        &said(&ran),
    );

    // The Layout is the Workspace's own record of the tree, and a move keeps it in step whichever
    // account holds the entry upstream.
    let ran = rola.run(dir, &["layout", "entries"]);
    checked.exits("the Layout is read back", &ran, 0);
    checked.stdout_has(
        "the Layout names the path the entry was moved to",
        &ran,
        "hero-moved.psd",
    );
    checked.wants(
        "the Layout no longer names the path it was moved from",
        !ran
            .stdout
            .lines()
            .any(|line| line.starts_with("hero.psd ")),
        &said(&ran),
    );

    let ran = rola.run(dir, &["fs-ops", "rm", "solo.psd"]);
    checked.exits("a path another account holds is removed", &ran, 0);
    checked.wants(
        "the held path is gone",
        !dir.join("solo.psd").exists(),
        &said(&ran),
    );
}

/// Taking ownership and letting it go, which the Desktop's ownership menu runs the same way.
///
/// What the Layout says about a Vault is what `hold` and `giveup` need, and a Workspace that tracks none is
/// refused for that. What is checked is that the paths are read at all: the run has to be refused for the
/// Vault it does not track rather than for the arguments it was given. It was refused for those, because the
/// flag was picked before the paths were — the picker reads a command's arguments by position — so a path
/// landed where the flag was looked for and the real argument was never named.
fn ownership(rola: &Rola, dir: &Path, checked: &mut Checked) {
    fs::create_dir_all(dir.join("art")).expect("a place to claim in");

    let ran = rola.run(dir, &["init"]);
    checked.exits("a Workspace is made to claim in", &ran, 0);

    let ran = rola.run(dir, &["layout", "new", "main"]);
    checked.exits("and a Layout to claim in", &ran, 0);

    fs::write(dir.join("art/hero.psd"), "art\n").expect("a file to claim");
    fs::write(dir.join("art/second.psd"), "art\n").expect("and another to claim beside it");

    for verb in ["hold", "giveup"] {
        let ran = rola.run(&dir.join("art"), &[verb, "hero.psd"]);
        checked.exits(
            &format!("`{verb}` reads a file it was named"),
            &ran,
            193,
        );
        checked.stderr_has(
            &format!("`{verb}` is refused for the Vault it tracks, and not for its arguments"),
            &ran,
            "tracks no Vault",
        );
    }

    let ran = rola.run(&dir.join("art"), &["hold", "--force", "hero.psd"]);
    checked.exits("and the forced form reads the same file", &ran, 193);

    // A menu over several chosen entries is one run of the same command: every path it was given, and the
    // flag that lets the ones that do pass be claimed while the ones that do not are said.
    let ran = rola.run(
        &dir.join("art"),
        &["hold", "hero.psd", "second.psd", "--allow-partial"],
    );
    checked.exits("several files are claimed in one run", &ran, 193);
    checked.stderr_has(
        "and that run is refused for the Vault as well",
        &ran,
        "tracks no Vault",
    );

    let ran = rola.run(dir, &["hold"]);
    checked.exits("while `hold` with nothing named is still refused", &ran, 191);
}

/// Copies a directory and everything under it, which is the shape a fetched Layout is laid down in.
fn copy_into(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the copy's own directory");

    for entry in fs::read_dir(from).expect("the directory to copy") {
        let entry = entry.expect("an entry of it");
        let under = to.join(entry.file_name());

        if entry.file_type().expect("what it is").is_dir() {
            copy_into(&entry.path(), &under);
        } else {
            fs::copy(entry.path(), under).expect("a copied file");
        }
    }
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
            .env("ROLA_HOME", &self.rola_home)
            // The language is pinned, so that what a check reads back does not depend on the
            // machine it runs on. A check about the language names its own.
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

/// The path as the argument a command is given.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// What a run printed, read as JSON, when it printed JSON at all.
fn json(ran: &Ran) -> Option<serde_json::Value> {
    serde_json::from_str(ran.stdout.trim()).ok()
}

/// The listing's item for the Layout named `name`.
fn layout_of<'a>(value: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    value
        .get("layouts")?
        .as_array()?
        .iter()
        .find(|item| item.get("name").and_then(serde_json::Value::as_str) == Some(name))
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
            self.wrong.push(format!("{question} — {why}"));
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
