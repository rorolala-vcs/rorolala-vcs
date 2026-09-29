//! `layout-remote`: fetching a Vault's Layout, and taking and letting go of an entry.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: it
//! makes a place to work in, keeps accounts, binds a Vault, puts an entry in the Vault's Layout,
//! serves the Vault, and then works the commands these exist for.
//!
//! What is checked is that a fetch brings the Vault's Layout here as a copy rather than moving it,
//! that the copy is what the reading commands answer from — including when there is no copy — and
//! that taking and letting go of an entry is written on the Vault and in the copy beside it.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::vault::{CONFIG_PATH, KEYS_DIR, LAYOUT_DIR, Vault};
use librorolala::workspace::Workspace;
use rorolala_utils_sandbox::{Guard, Serving, command, run, serve};
use uuid::Uuid;

/// The port the Vault serves on.
const PORT: u16 = 7941;

/// The account the work acts as first, and the one it is handed to after.
const ALICE: &str = "alice";
const BOB: &str = "bob";

/// The name the Workspace knows the Vault by.
const VAULT_NAME: &str = "origin";

/// The entry the Vault's Layout holds, and one it does not.
const ENTRY: Uuid = Uuid::from_u128(0x42);
const GONE: Uuid = Uuid::from_u128(0x99);

/// How long the Vault is waited for before it is given up on.
const STARTUP: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() {
    let sandbox = Guard::new("layout-remote");
    let data = sandbox.join("data");
    let root = sandbox.join("root");
    let workspace = sandbox.join("ws");

    serveable(&root, PORT);

    Workspace::create(&workspace)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", workspace.display()));
    let auth = workspace.join(".rola").join("auth");
    for name in [ALICE, BOB] {
        pair(&auth, name);
        publish(&auth, &root, name);
    }

    // The Vault's Layout holds one entry, held by nobody. It is written before the Vault is served,
    // so what a fetch brings is what was put there and not a Layout that happened to be empty.
    let vault_layout = Layout::open(root.join(LAYOUT_DIR)).expect("the Vault's Layout");
    vault_layout
        .create_entry(ENTRY, MutableData::new(None, [7; 32], "a file".to_owned()))
        .expect("an entry");
    vault_layout
        .create_path(&LayoutPath::new("a.psd").unwrap(), ENTRY)
        .expect("a path");

    run(&mut client(&workspace, &data, &["account", ALICE])).expect_success();
    run(&mut client(
        &workspace,
        &data,
        &["vault", "bind", VAULT_NAME, &address(PORT)],
    ))
    .expect_success();

    let copy = workspace
        .join(".rola/cache/readonly-layouts")
        .join(VAULT_NAME)
        .join("truth");
    let entry = ENTRY.to_string();
    let gone = GONE.to_string();
    let mut checked = Checked::default();

    // Before anything is fetched there is no copy, and the reading commands say so rather than
    // reaching for a Vault — the Vault is not even serving yet.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "reading with no copy is refused",
        said.code == Some(192) && said.stderr.contains("has not been fetched"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "ls-ownership", VAULT_NAME],
    ));
    checked.wants(
        "listing with no copy is refused",
        said.code == Some(192),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A Vault's Layout is named `NAME@VAULT`, and reading one that was never fetched is refused the
    // same way whether it is a listing or an entry query.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "entries", "--layout", "truth@origin"],
    ));
    checked.wants(
        "querying an unfetched Vault Layout is refused",
        said.code == Some(192),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // What is kept under a name is kept under a name: an address is not one.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "fetch", &address(PORT)],
    ));
    checked.wants(
        "fetching by address is refused",
        said.code == Some(71),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let serving = serve_vault(&root, PORT);

    // A fetch brings the Vault's Layout here. What comes back says so, and what is on disk is a
    // Layout of the Workspace's own that holds the entry the Vault holds.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "fetch", VAULT_NAME],
    ));
    checked.wants(
        "the Vault's Layout is fetched",
        said.success() && said.stdout.contains("readonly-layouts"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    let copied = Layout::open(&copy).expect("the fetched copy");
    checked.wants(
        "the fetched copy is a Layout holding what the Vault held",
        copied.entry(ENTRY).is_some()
            && copied.id_of(&LayoutPath::new("a.psd").unwrap()) == Some(ENTRY),
        "the copy did not hold the entry the Vault held",
    );

    // The copy is what a name written `NAME@VAULT` reads, so the query commands reach it too.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "entries", "--layout", "truth@origin"],
    ));
    checked.wants(
        "a Vault Layout is queried through NAME@VAULT",
        said.success() && clean(&said.stdout).contains("a.psd") && said.stdout.contains(&entry),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A copy is read, never worked in: a command that changes a Layout refuses one.
    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "path",
            "remove",
            "a.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "a fetched copy is refused by a command that writes",
        said.code == Some(190) && clean(&said.stderr).contains("cannot be changed"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // An entry nobody holds reads as nobody's, from the copy.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "an unheld entry reads as held by nobody",
        said.success() && clean(&said.stdout).contains("held by nobody"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // Taking it names this account, on the Vault and in the copy beside it.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "req-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "an unheld entry is taken",
        said.success() && clean(&said.stdout).contains(ALICE),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    checked.wants(
        "the Vault names the account that took the entry",
        vault_owner(&root) == Some(ALICE.to_owned()),
        "the Vault's Layout did not name the taker",
    );
    checked.wants(
        "the copy names the account that took the entry",
        Layout::open(&copy)
            .expect("the fetched copy")
            .entry(ENTRY)
            .and_then(|data| data.owner().map(str::to_owned))
            == Some(ALICE.to_owned()),
        "the copy did not follow the Vault",
    );

    // Another account cannot take what is held, and nothing changes when it tries.
    run(&mut client(&workspace, &data, &["account", BOB])).expect_success();
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "req-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "an entry another account holds cannot be taken",
        said.code == Some(193) && clean(&said.stderr).contains(ALICE),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "the holder is unchanged by a refusal",
        said.success() && clean(&said.stdout).contains(ALICE),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // What the copy lists is every entry the Vault's Layout names, with its holder.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "ls-ownership", VAULT_NAME, "--json"],
    ));
    checked.wants(
        "listing as a program names every entry and its holder",
        said.success() && said.stdout.contains(&entry) && said.stdout.contains(ALICE),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // Letting go is the holder's to do, and the Vault and the copy both end up naming nobody.
    run(&mut client(&workspace, &data, &["account", ALICE])).expect_success();
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "giveup-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "the holder lets the entry go",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    checked.wants(
        "the Vault names nobody after a give-up",
        vault_owner(&root).is_none(),
        "the Vault still named a holder",
    );
    checked.wants(
        "the copy names nobody after a give-up",
        Layout::open(&copy)
            .expect("the fetched copy")
            .entry(ENTRY)
            .and_then(|data| data.owner().map(str::to_owned))
            .is_none(),
        "the copy still named a holder",
    );

    // An entry the Vault does not hold is told apart from one it holds and nobody owns, and from
    // one the copy does not name.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "req-ownership", VAULT_NAME, &gone],
    ));
    checked.wants(
        "taking an entry the Vault does not hold is refused",
        said.code == Some(192) && clean(&said.stderr).contains("holds no entry"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &gone],
    ));
    checked.wants(
        "reading an entry the copy does not name is refused",
        said.code == Some(192) && clean(&said.stderr).contains("names no entry"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    serving.stop();
    checked.report();
}

/// The text of `said` with any terminal escape sequences taken out.
///
/// Output that has been piped is not read by a terminal, so what it holds is the line itself — but a
/// check that compares against it should not depend on that. This takes the escapes out either way.
fn clean(said: &str) -> String {
    let mut clean = String::with_capacity(said.len());
    let mut rest = said;

    while let Some(start) = rest.find('\u{1b}') {
        clean.push_str(&rest[..start]);

        // An escape sequence ends at `m`; anything after it is what follows.
        match rest[start..].find('m') {
            Some(end) => rest = &rest[start + end + 1..],
            None => return clean,
        }
    }
    clean.push_str(rest);

    clean
}

/// A command that runs the client, working in `workspace` and keeping its own files under `data`.
fn client(workspace: &Path, data: &Path, args: &[&str]) -> Command {
    let mut command = command("rola");

    command
        .current_dir(workspace)
        .env("XDG_DATA_HOME", data)
        // What the run says is checked, so it is said in one language rather than whichever one
        // the machine happens to be set to.
        .env("ROLA_LANG", "en")
        .args(args);

    command
}

/// Who holds [`ENTRY`] in the Vault's own Layout, read afresh.
///
/// The Vault is another process, so its Layout is opened again rather than kept: what this process
/// read before is what was true then, and only a fresh read sees what the Vault's daemon wrote.
fn vault_owner(root: &Path) -> Option<String> {
    Layout::open(root.join(LAYOUT_DIR))
        .expect("the Vault's Layout")
        .entry(ENTRY)
        .and_then(|data| data.owner().map(str::to_owned))
}

/// The address a Vault serving on `port` is reached at.
fn address(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// Makes `dir` a Vault that can be served: a Vault of its own, listening on `port`, holding the
/// account it proves itself with.
fn serveable(dir: &Path, port: u16) {
    Vault::create(dir).unwrap_or_else(|error| panic!("making {}: {error:?}", dir.display()));

    fs::write(
        dir.join(CONFIG_PATH),
        format!("[daemon_config]\nprefer_port = {port}\n"),
    )
    .expect("writing the Vault's configuration");

    pair(&dir.join(KEYS_DIR), "vault");
}

/// Puts the public half of the pair `name` names, kept under `keys`, where `vault` admits it.
fn publish(keys: &Path, vault: &Path, name: &str) {
    let public = keys.join(format!("{name}.pub"));
    let admitted = vault.join(KEYS_DIR);

    fs::create_dir_all(&admitted).expect("the Vault's keys directory");
    fs::copy(&public, admitted.join(format!("{name}.pub")))
        .unwrap_or_else(|error| panic!("publishing {}: {error}", public.display()));
}

/// Writes the key pair `name` names under `dir`.
fn pair(dir: &Path, name: &str) {
    fs::create_dir_all(dir).expect("the keys directory");
    let private = dir.join(format!("{name}.pem"));
    let public = dir.join(format!("{name}.pub"));

    run(Command::new("openssl")
        .args(["genpkey", "-algorithm", "ed25519", "-out"])
        .arg(&private))
    .expect_success();

    run(Command::new("openssl")
        .args(["pkey", "-in"])
        .arg(&private)
        .args(["-pubout", "-out"])
        .arg(&public))
    .expect_success();
}

/// Serves the Vault rooted at `dir`, and waits until it is listening on `port`.
fn serve_vault(dir: &Path, port: u16) -> Serving {
    serve(
        command("rola-daemon")
            .arg("--vault-dir")
            .arg(dir)
            .arg("listen"),
        ("127.0.0.1", port),
        STARTUP,
    )
}

/// What was asked of the commands, and how many of the questions were answered as they should be.
#[derive(Default)]
struct Checked {
    /// How many questions were asked.
    asked: usize,
    /// Which of them were answered with something other than what was wanted.
    wrong: Vec<String>,
}

impl Checked {
    /// Counts a question, and records it as wrong when it was not answered as it should be.
    fn wants(&mut self, question: &str, answered: bool, said: &str) {
        self.asked += 1;

        if !answered {
            self.wrong.push(format!("{question} — {said}"));
        }
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
