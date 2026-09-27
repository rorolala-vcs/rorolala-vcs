//! `vcs-index-sync`: what `rola vcs-index sync-all` does to a Workspace's index and a Vault's.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: what
//! it does is what a person does with a terminal — make a place to work in, keep an account, bind a
//! Vault, put an object in each index, serve the Vault, ask for a sync — and what it checks is what
//! the two indexes hold afterwards.
//!
//! What is checked is that a sync is not an upload or a download: the Vault holds an object the
//! Workspace's index has never seen and the Workspace holds one the Vault has not, and one run
//! leaves each of them holding both. The store is left alone by all of it, which is the point of an
//! index sync being its own action rather than the store's.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::storage::{Key, StorageBackend as _};
use librorolala::vault::{CONFIG_PATH, KEYS_DIR, Vault};
use librorolala::vcs::{VCSIndex, VCSIndexObject, Variant, Version};
use librorolala::workspace::Workspace;
use rorolala_utils_sandbox::{Guard, Serving, command, run, serve};

/// The port the Vault serves on.
const PORT: u16 = 7932;

/// The account the client acts as, which is a member of the Vault.
const NAME: &str = "rootuser";

/// The name the Workspace knows the Vault by.
const VAULT_NAME: &str = "origin";

/// How long the Vault is waited for before it is given up on.
const STARTUP: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() {
    let sandbox = Guard::new("vcs-index-sync");
    // Where the program keeps what it keeps for itself, so that a run of it here touches nothing
    // the machine's own runs keep.
    let data = sandbox.join("data");
    let root = sandbox.join("root");
    let workspace = sandbox.join("ws");

    // The Vault, with the port it listens on and a key of its own to prove itself with.
    serveable(&root, PORT);

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

    // What each index holds to begin with: the Vault one object the Workspace has never seen, the
    // Workspace one the Vault has not.
    let workspace_index = VCSIndex::create(workspace.join(".rola/index"));
    let vault_index = VCSIndex::create(root.join("index"));

    let workspace_only =
        Variant::new_bare_variant([0x11; 32], [0x12; 32], None, [0x13; 32], [0x14; 32], 0);
    let workspace_key = workspace_index
        .write(workspace_only.clone())
        .await
        .expect("the Workspace's object");

    let vault_only = Version::new_bare_version([0x21; 32], 0);
    let vault_key = vault_index
        .write(vault_only)
        .await
        .expect("the Vault's object");

    let mut checked = Checked::default();

    // A run that is not confirmed changes nothing. There is no terminal here, so the question is
    // asked of nothing and answered with no, which is what a run without a person at it does.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "sync-all", VAULT_NAME],
    ));

    checked.wants(
        "an unconfirmed sync says it was not confirmed",
        said.code == Some(3),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "an unconfirmed sync leaves the Vault's index as it was",
        !holds(&vault_index, &[workspace_key]).await,
        "a run that was not confirmed changed the Vault's index",
    );

    // The Vault is served, and the same run is made with the question answered in advance. It is made
    // with `--json` as well, which is how a run says it is being read by a program rather than watched
    // by a person: what it is doing while it does it is then written down a record at a time.
    let serving = serve_vault(&root, PORT);

    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "sync-all", VAULT_NAME, "--confirm", "--json"],
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
        holds(&workspace_index, &[vault_key]).await,
        "the object only the Vault held is not on the Workspace",
    );
    checked.wants(
        "the Vault holds what only the Workspace had",
        holds(&vault_index, &[workspace_key]).await,
        "the object only the Workspace held is not on the Vault",
    );

    // What crossed is the object itself, not a re-encoding of it: a variant read at the Vault is the
    // one the Workspace wrote.
    let read = vault_index
        .read(workspace_key)
        .await
        .expect("the variant is read at the Vault");
    checked.wants(
        "the object that crossed reads back as it was written",
        read == VCSIndexObject::Variant(workspace_only),
        "the object changed on the way across",
    );

    // Asking again is asking two indexes that already agree.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "sync-all", VAULT_NAME, "--confirm"],
    ));

    checked.wants(
        "a sync of two indexes that agree is made without complaint",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    serving.stop();
    checked.report();
}

/// A command that runs the client, working in `workspace` and keeping its own files under `data`.
fn client(workspace: &Path, data: &Path, args: &[&str]) -> Command {
    let mut command = command("rola");

    command
        .current_dir(workspace)
        .env("XDG_DATA_HOME", data)
        .args(args);

    command
}

/// Whether `index` can produce every one of `keys`.
async fn holds(index: &VCSIndex, keys: &[Key]) -> bool {
    index
        .contains_keys(keys)
        .await
        .expect("the index answers")
        .iter()
        .all(|held| held)
}

/// The address a Vault serving on `port` is reached at.
fn address(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// Makes `dir` a Vault that can be served: a Vault of its own, listening on `port`, holding the
/// account it proves itself with.
fn serveable(dir: &Path, port: u16) {
    Vault::create(dir).unwrap_or_else(|error| panic!("making {}: {error:?}", dir.display()));

    // The port is the one thing a Vault cannot be talked into, so it is written down before it is
    // served rather than asked for afterwards.
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

/// What was asked of the two indexes, and how many of the questions were answered as they should be.
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
