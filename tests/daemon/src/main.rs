//! `daemon`: what `rola-daemon`'s registry commands do to the machine's table.
//!
//! A program rather than a set of tests cargo runs, because what it does is what a person does
//! with a terminal: name a Vault, list what is served, check it, take it away, and complain
//! when what was said is not what was expected. The machine's own table is never touched: every
//! run is given a sandbox's `XDG_DATA_HOME`, which is where the daemon looks for it.

use std::fs;
use std::path::Path;
use std::process::Command;

use rorolala_utils_sandbox::{Guard, command, run};

/// The namespace the served Vault is registered under.
const NAMESPACE: &str = "alpha";

fn main() {
    let sandbox = Guard::new("daemon");
    let data = sandbox.join("data");
    let vault = sandbox.join("vaults/alpha");
    let plain = sandbox.join("vaults/plain");

    // One directory that is a Vault and one that is not, so registering can be checked both
    // ways before anything is served.
    make_vault(&vault);
    fs::create_dir_all(&plain).expect("the plain directory");

    // A table nothing has been registered in is empty rather than missing.
    let said = run(&mut daemon(&data, &["list"]));
    said.expect_success();
    assert_eq!(said.stdout.trim(), "", "an empty table lists nothing");

    // A directory that holds no Vault is refused.
    let said = run(&mut daemon(&data, &["register", "plain", &text(&plain)]));
    assert!(
        !said.success(),
        "registering a directory that is not a Vault should fail, got {:?}",
        said.code
    );

    // A Vault is registered under the namespace it is given.
    let said = run(&mut daemon(&data, &["register", NAMESPACE, &text(&vault)]));
    said.expect_success();
    assert!(
        said.stdout.contains(NAMESPACE),
        "the registered namespace was expected, got {:?}",
        said.stdout
    );

    // The same name may not be registered twice.
    let said = run(&mut daemon(&data, &["register", NAMESPACE, &text(&vault)]));
    assert_eq!(
        said.code,
        Some(10),
        "a name that is taken should be refused, got {:?}\n{}",
        said.code,
        said.stderr
    );

    // A name that is not one segment of an address is refused.
    let said = run(&mut daemon(&data, &["register", "a/b", &text(&vault)]));
    assert!(
        !said.success(),
        "a namespace with a separator should be refused, got {:?}",
        said.code
    );

    // What is listed is what was registered, one a line by default.
    let said = run(&mut daemon(&data, &["list"]));
    said.expect_success();
    assert_eq!(said.stdout.trim(), NAMESPACE);

    // `--json` writes the same row as data.
    let said = run(&mut daemon(&data, &["list", "--json"]));
    said.expect_success();
    assert!(
        said.stdout.contains("\"alpha\""),
        "the namespace was expected in the JSON, got {:?}",
        said.stdout
    );
    assert!(
        said.stdout.contains("\"path\""),
        "the path was expected in the JSON, got {:?}",
        said.stdout
    );

    // A table whose rows all point at Vaults checks clean, in the shape the help describes.
    let said = run(&mut daemon(&data, &["check"]));
    said.expect_success();
    assert!(
        said.stdout.contains("[OK] alpha ("),
        "an OK row was expected, got {:?}",
        said.stdout
    );

    // `--format` draws the same rows through a template.
    let said = run(&mut daemon(
        &data,
        &["check", "--format", "{{ checks.namespace }}"],
    ));
    said.expect_success();
    assert_eq!(said.stdout.trim(), NAMESPACE);

    // A row whose Vault is gone fails, and takes the run's status with it.
    fs::remove_file(vault.join("vault.toml")).expect("removing the Vault's configuration");
    let said = run(&mut daemon(&data, &["check"]));
    assert_eq!(said.code, Some(36), "a failed check should end non-zero");
    assert!(
        said.stdout.contains("[FAIL] alpha ("),
        "a FAIL row was expected, got {:?}",
        said.stdout
    );

    // Removing takes the row, and a row that is not there is refused.
    let said = run(&mut daemon(&data, &["remove", NAMESPACE]));
    said.expect_success();
    let said = run(&mut daemon(&data, &["remove", NAMESPACE]));
    assert_eq!(
        said.code,
        Some(12),
        "removing a name that is not registered should fail, got {:?}",
        said.code
    );

    let said = run(&mut daemon(&data, &["list"]));
    said.expect_success();
    assert_eq!(said.stdout.trim(), "", "the removed row is not listed");

    // A path may be left out when the run is inside the Vault it means: the nearest Vault above
    // the working directory is the one registered.
    let auto = sandbox.join("vaults/auto");
    make_vault(&auto);
    fs::create_dir_all(auto.join("nested")).expect("a directory inside the Vault");

    let said = run(&mut daemon_in(
        &data,
        &auto.join("nested"),
        &["register", "auto"],
    ));
    said.expect_success();

    let said = run(&mut daemon(&data, &["list", "--json"]));
    said.expect_success();
    assert!(
        said.stdout.contains("\"auto\""),
        "the namespace was expected in the JSON, got {:?}",
        said.stdout
    );
    assert!(
        said.stdout.contains(&text(&auto)),
        "the Vault the run was inside was expected, got {:?}",
        said.stdout
    );

    // A run that is inside no Vault has nothing to look up, and says so rather than guessing.
    let said = run(&mut daemon_in(&data, &plain, &["register", "elsewhere"]));
    assert!(
        !said.success(),
        "registering with no path outside a Vault should fail, got {:?}",
        said.code
    );
}

/// A directory that is a Vault, as far as the registry is concerned.
fn make_vault(dir: &Path) {
    fs::create_dir_all(dir).expect("the Vault's directory");
    fs::write(
        dir.join("vault.toml"),
        "[vault_config]\nvault_name = \"Alpha\"\n",
    )
    .expect("the Vault's configuration");
}

/// A command running the daemon against the sandbox's own data directory.
fn daemon(data: &Path, args: &[&str]) -> Command {
    let mut command = command("rola-daemon");
    command.env("XDG_DATA_HOME", data).args(args);
    command
}

/// As [`daemon`], working in `dir`, which is what a run with no path looks from.
fn daemon_in(data: &Path, dir: &Path, args: &[&str]) -> Command {
    let mut command = daemon(data, args);
    command.current_dir(dir);
    command
}

/// The path as the argument a command is given.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
