//! `storage-sync`: what `rola storage sync-all` does to a Workspace's store and a Vault's.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: what
//! it does is what a person does with a terminal — make a place to work in, keep an account, bind a
//! Vault, put something in each store, serve the Vault, ask for a sync — and what it checks is what
//! the two stores hold afterwards.
//!
//! What is checked is that a sync is not an upload or a download: the Vault holds a key the
//! Workspace has never seen and the Workspace holds two the Vault has not, and one run leaves each
//! of them holding all three. One of the Workspace's is a content big enough to be kept as chunks,
//! so what crosses for it is a manifest and the chunks the manifest names — and the manifest that
//! arrives is the one that was sent, rather than the content being cut over again at the far end.
//! Another is a content larger than one value carries, which crosses as several.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::storage::{Codec, Key, RorolalaStorage, StorageBackend as _};
use librorolala::vault::{KEYS_DIR, Vault, locate_vault};
use librorolala::workspace::{Workspace, locate_workspace};
use rorolala_utils_sandbox::{Guard, Serving, command, run, serve};

/// The port the Vault serves on.
const PORT: u16 = 7931;

/// The namespace the Vault is registered under.
const NAMESPACE: &str = "vault";

/// The account the client acts as, which is a member of the Vault.
const NAME: &str = "rootuser";

/// The name the Workspace knows the Vault by.
const VAULT_NAME: &str = "origin";

/// How long the Vault is waited for before it is given up on.
const STARTUP: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() {
    let sandbox = Guard::new("storage-sync");
    // Where the program keeps what it keeps for itself, so that a run of it here touches nothing
    // the machine's own runs keep.
    let data = sandbox.join("data");
    let root = sandbox.join("root");
    let workspace = sandbox.join("ws");

    // The Vault, with the namespace it is reached under.
    serveable(&root);

    // The Workspace: a place to work in, the account it acts as, and the Vault it knows.
    Workspace::create(&workspace)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", workspace.display()));
    let keys = workspace.join(".rola").join("auth");
    pair(&keys, NAME);
    publish(&keys, &root, NAME);

    run(&mut client(&workspace, &data, &["account", NAME])).expect_success();
    run(&mut client(
        &workspace,
        &data,
        &["vault", "bind", VAULT_NAME, &address(PORT)],
    ))
    .expect_success();

    // What each store holds to begin with: the Vault one key the Workspace has never seen, the
    // Workspace two — one written whole, and one written as a file big enough to be cut.
    let vault_store = store_of_vault(&root);
    let workspace_store = store_of_workspace(&workspace);

    let only_in_the_vault = vault_store
        .write_object(b"what only the Vault wrote", Codec::Raw)
        .await
        .expect("the Vault's object");

    let plain_file = workspace.join("plain.txt");
    fs::write(&plain_file, b"what only the Workspace wrote").expect("the plain file");
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "write-file", &text(&plain_file)],
    ));
    let plain: Option<Key> = said.stdout.trim().parse().ok();

    let cut_file = workspace.join("cut.txt");
    fs::write(&cut_file, lines(400)).expect("the cut file");
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "write-file", &text(&cut_file)],
    ));
    let cut: Option<Key> = said
        .stdout
        .trim()
        .strip_prefix("manifest:")
        .and_then(|hex| hex.parse().ok());

    let mut checked = Checked::default();

    checked.wants(
        "a small file is written as a hash",
        plain.is_some(),
        &format!("it said {:?}", said.stdout.trim()),
    );
    checked.wants(
        "a file big enough to cut is written as a manifest",
        cut.is_some(),
        &format!("it said {:?}", said.stdout.trim()),
    );

    let (Some(plain), Some(cut)) = (plain, cut) else {
        checked.report();
        unreachable!("a run without the keys it needs has been reported");
    };

    // A run that is not confirmed changes nothing. There is no terminal here, so the question is
    // asked of nothing and answered with no, which is what a run without a person at it does.
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "sync-all", VAULT_NAME],
    ));

    checked.wants(
        "an unconfirmed sync says it was not confirmed",
        said.code == Some(3),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "an unconfirmed sync leaves the Vault's store as it was",
        !holds(&vault_store, &[plain, cut]).await,
        "a run that was not confirmed changed the Vault's store",
    );

    // The Vault is served, and the same run is made with the question answered in advance. It is made
    // with `--json` as well, which is how a run says it is being read by a program rather than watched
    // by a person: what it is doing while it does it is then written down a record at a time, which is
    // what the questions below read back.
    let serving = serve_vault(&data, &root, PORT);

    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "sync-all", VAULT_NAME, "--confirm", "--json"],
    ));

    checked.wants(
        "a confirmed sync is made",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "a watched sync says what it began and that it is over",
        said.stderr.contains("\"signal\":\"begin\"")
            && said.stderr.contains("\"signal\":\"finish\""),
        &format!("it said on stderr {:?}", said.stderr.trim()),
    );
    checked.wants(
        "a key says which way it crossed",
        said.stderr.contains("\"direction\":\"up\"")
            && said.stderr.contains("\"direction\":\"down\""),
        &format!("it said on stderr {:?}", said.stderr.trim()),
    );
    checked.wants(
        "the Workspace holds what only the Vault had",
        holds(&workspace_store, &[only_in_the_vault]).await,
        "the key only the Vault held is not on the Workspace",
    );
    checked.wants(
        "the Vault holds what only the Workspace had",
        holds(&vault_store, &[plain]).await,
        "the key only the Workspace held is not on the Vault",
    );
    checked.wants(
        "a content kept as chunks crosses whole",
        holds(&vault_store, &[cut]).await,
        "the Vault cannot put the cut content back together",
    );

    // Read once here and once after the run that follows, so that what crossed can be compared with
    // what the store that sent it keeps.
    let cut_on_the_workspace = workspace_store
        .manifest_of(&cut)
        .await
        .expect("the Workspace's manifest");
    let cut_on_the_vault = vault_store
        .manifest_of(&cut)
        .await
        .expect("the Vault's manifest");

    checked.wants(
        "the manifest crossed rather than being cut again",
        cut_on_the_vault.is_some() && cut_on_the_vault == cut_on_the_workspace,
        "the two stores cut the same content differently",
    );

    // Asking again is asking two stores that already agree — except for this: a content larger than
    // one frame, which is what a value that was too long to frame used to stop from crossing at all.
    let big: Vec<u8> = (0..20 * 1024 * 1024_u32)
        .map(|at| (at % 251) as u8)
        .collect();
    let big_key = vault_store
        .write_object(&big, Codec::Raw)
        .await
        .expect("the big object");

    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "sync-all", VAULT_NAME, "--confirm"],
    ));

    checked.wants(
        "a sync of two stores that agree is made without complaint",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let cut_on_the_vault_again = vault_store
        .manifest_of(&cut)
        .await
        .expect("the Vault's manifest");

    checked.wants(
        "the manifest that crossed is still the one it was",
        cut_on_the_vault_again == cut_on_the_vault,
        "a second sync cut the content differently",
    );
    checked.wants(
        "a content larger than one frame crosses",
        holds(&workspace_store, &[big_key]).await,
        "the big content is not on the Workspace",
    );
    checked.wants(
        "and crosses whole",
        workspace_store
            .read_object(&big_key)
            .await
            .is_ok_and(|read| read == big),
        "the big content did not read back",
    );

    serving.stop();
    checked.report();
}

/// A command that runs the client, working in `workspace` and keeping its own files under `data`.
///
/// A run of the program is a run of the program, so it is reached the way a person reaches it: the
/// directory it is run in is the Workspace it works in, and what it keeps for itself is kept
/// somewhere of this test's own rather than wherever the machine's runs keep theirs.
fn client(workspace: &Path, data: &Path, args: &[&str]) -> Command {
    let mut command = command("rola");

    command
        .current_dir(workspace)
        .env("XDG_DATA_HOME", data).env("ROLA_DATA_HOME", data)
        .args(args);

    command
}

/// The path as the argument a command is given.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A text long enough to be cut, with a line of its own for each of `count`:
///
/// A content of one repeated phrase is not cut, however long it is: what a cut looks for is a place
/// where one stretch of content stops looking like the one before it, and repetition is exactly what
/// there is none of.
fn lines(count: usize) -> String {
    use std::fmt::Write as _;

    let mut text = String::new();
    for at in 0..count {
        writeln!(
            text,
            "line {at} of something written, {}",
            "x".repeat(at % 37)
        )
        .expect("writing a line");
    }

    text
}

/// Whether `store` can produce every one of `keys`.
async fn holds(store: &RorolalaStorage, keys: &[Key]) -> bool {
    store
        .contains_keys(keys)
        .await
        .expect("the store answers")
        .iter()
        .all(|held| held)
}

/// The store the Workspace at `dir` works on.
fn store_of_workspace(dir: &Path) -> RorolalaStorage {
    locate_workspace(dir)
        .unwrap_or_else(|| panic!("{} is not a Workspace", dir.display()))
        .get_or_create_rola_storage()
}

/// The store the Vault at `dir` works on.
fn store_of_vault(dir: &Path) -> RorolalaStorage {
    locate_vault(dir)
        .unwrap_or_else(|| panic!("{} is not a Vault", dir.display()))
        .get_or_create_rola_storage()
}

/// The address a Vault serving on `port` is reached at.
fn address(port: u16) -> String {
    format!("127.0.0.1:{port}/{NAMESPACE}")
}

/// Makes `dir` a Vault that can be served: a Vault of its own.
fn serveable(dir: &Path) {
    Vault::create(dir).unwrap_or_else(|error| panic!("making {}: {error:?}", dir.display()));
}

/// Puts the public half of the pair `name` names, kept under `keys`, where `vault` admits it.
///
/// Only the public half: a member is a name a Vault admits, and the private half stays with whoever
/// acts as that member.
fn publish(keys: &Path, vault: &Path, name: &str) {
    let public = keys.join(format!("{name}.pub"));
    let admitted = vault.join(KEYS_DIR);

    fs::create_dir_all(&admitted).expect("the Vault's keys directory");
    fs::copy(&public, admitted.join(format!("{name}.pub")))
        .unwrap_or_else(|error| panic!("publishing {}: {error}", public.display()));
}

/// Writes the key pair `name` names under `dir`.
///
/// `openssl` makes it, which is what the program itself uses: a pair that is not one a caller could
/// have made would have this test checking something nobody can do.
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

/// Serves the Vault rooted at `dir` under the namespace, and waits until it is listening.
///
/// The daemon belongs to the machine, so it is told where it sits through the user's own data
/// directory: the sandbox's `ROLA_DATA_HOME` holds both the table and the account it proves
/// itself with — the variable a Windows machine reads, where `XDG_DATA_HOME` names nothing.
fn serve_vault(data: &Path, dir: &Path, port: u16) -> Serving {
    register(data, dir);

    serve(
        command("rola-daemon")
            .arg("--port")
            .arg(port.to_string())
            .env("XDG_DATA_HOME", data).env("ROLA_DATA_HOME", data),
        ("127.0.0.1", port),
        STARTUP,
    )
}

/// Writes the table naming `dir` as [`NAMESPACE`], and gives the daemon an account of its own.
fn register(data: &Path, dir: &Path) {
    let registry = data.join("rola").join("daemon").join("registry.toml");

    fs::create_dir_all(registry.parent().expect("the daemon's directory"))
        .expect("the daemon's directory");
    fs::write(&registry, format!("{NAMESPACE} = {dir:?}\n")).expect("the namespace table");

    let keys = data.join("rola").join("keys");
    if !keys.join("daemon.pem").is_file() {
        pair(&keys, "daemon");
    }
}

/// What was asked of the two stores, and how many of the questions were answered as they should be.
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
