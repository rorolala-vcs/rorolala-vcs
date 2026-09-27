//! `storage-remote`: what `ls-remote-*` list and what `sync-hashes` carries.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: it
//! makes a place to work in, keeps an account, binds a Vault, puts something in each store, serves
//! the Vault, and then asks the two questions these commands exist for.
//!
//! What is checked is two things. A listing is *the other end's*, not the run's own — the keys the
//! Vault holds are named, and the ones only the Workspace holds are not — and it changes nothing. And
//! a named sync carries only what it is told to: a key only the Workspace holds crosses to the Vault,
//! a key only the Vault holds crosses back, a key that was not named stays where it was, and a
//! content kept as chunks crosses whole, manifest and all.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vault::{CONFIG_PATH, KEYS_DIR, Vault, locate_vault};
use librorolala::workspace::{Workspace, locate_workspace};
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
    let sandbox = Guard::new("storage-remote");
    let data = sandbox.join("data");
    let root = sandbox.join("root");
    let workspace = sandbox.join("ws");

    serveable(&root, PORT);

    Workspace::create(&workspace)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", workspace.display()));
    let auth = workspace.join(".rola").join("auth");
    pair(&auth, NAME);
    publish(&auth, &root, NAME);

    run(&mut client(&workspace, &data, &["account", NAME])).expect_success();
    run(&mut client(
        &workspace,
        &data,
        &["vault", "bind", VAULT_NAME, &address(PORT)],
    ))
    .expect_success();

    let vault_store = store_of_vault(&root);
    let workspace_store = store_of_workspace(&workspace);

    // What the Vault holds and the Workspace does not: one object written whole, and one content big
    // enough to be kept as chunks — so the Vault has both an object and a manifest to list.
    let only_in_the_vault = vault_store
        .write_object(
            b"what only the Vault wrote",
            librorolala::storage::Codec::Raw,
        )
        .await
        .expect("the Vault's object");

    let vault_cut_file = root.join("vaultcut.txt");
    fs::write(&vault_cut_file, lines(400, "the Vault's")).expect("the Vault's cut file");
    let said = run(&mut client(
        &root,
        &data,
        &["storage", "write-file", &text(&vault_cut_file)],
    ));
    let vault_cut: Option<Key> = manifest_of(&said.stdout);

    // What the Workspace holds and the Vault does not: one object written whole, and one kept as
    // chunks. The plain one is what a named sync is told to carry; the cut one is what stays behind.
    let plain_file = workspace.join("plain.txt");
    fs::write(&plain_file, b"what only the Workspace wrote").expect("the plain file");
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "write-file", &text(&plain_file)],
    ));
    let plain: Option<Key> = said.stdout.trim().parse().ok();

    let cut_file = workspace.join("cut.txt");
    fs::write(&cut_file, lines(400, "the Workspace's")).expect("the cut file");
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "write-file", &text(&cut_file)],
    ));
    let cut: Option<Key> = manifest_of(&said.stdout);

    let mut checked = Checked::default();

    checked.wants(
        "the Vault has a content kept as chunks",
        vault_cut.is_some(),
        &format!("it said {:?}", said.stdout.trim()),
    );
    checked.wants(
        "the Workspace has an object and a content kept as chunks",
        plain.is_some() && cut.is_some(),
        &format!("it said {:?}", said.stdout.trim()),
    );

    let (Some(vault_cut), Some(plain), Some(cut)) = (vault_cut, plain, cut) else {
        checked.report();
        unreachable!("a run without the keys it needs has been reported");
    };

    let serving = serve_vault(&root, PORT);

    // A listing is the other end's, and changes nothing. The run is made without `--json`, so what it
    // prints is the lines a person would read and hand back.
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "ls-remote-storaged", VAULT_NAME],
    ));
    let listed = keys(&said.stdout);

    checked.wants(
        "a listing of the other end's store is answered",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "the Vault's own object is listed",
        listed.contains(&only_in_the_vault),
        &format!("it listed {:?}", said.stdout.trim()),
    );
    checked.wants(
        "what only the Workspace holds is not listed",
        !listed.contains(&plain) && !listed.contains(&cut),
        "the run's own store was listed instead of the other end's",
    );
    checked.wants(
        "listing the other end changes nothing",
        !holds(&vault_store, &[plain, cut]).await
            && !holds(&workspace_store, &[only_in_the_vault]).await,
        "a listing moved something",
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "ls-remote-manifests", VAULT_NAME],
    ));
    let manifests = keys(&said.stdout);

    checked.wants(
        "the other end's manifests are listed",
        manifests.contains(&vault_cut) && !manifests.contains(&cut),
        &format!("it listed {:?}", said.stdout.trim()),
    );

    // A named sync carries only what it is told to. The plain object is named and crosses; the cut
    // content is not, and stays behind.
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "sync-hashes", &plain.hex(), VAULT_NAME],
    ));

    checked.wants(
        "a named sync is made without being asked to confirm",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "the key that was named crosses to the other end",
        holds(&vault_store, &[plain]).await,
        "the named key did not cross",
    );
    checked.wants(
        "a key that was not named stays where it was",
        !holds(&vault_store, &[cut]).await,
        "a key that was not named crossed too",
    );
    checked.wants(
        "and nothing crossed back that was not named",
        !holds(&workspace_store, &[only_in_the_vault]).await,
        "a key that was not named crossed the other way",
    );

    // The cut content, named as a manifest, crosses whole — the manifest and the chunks it names —
    // and crosses back the way it was kept rather than being cut over again.
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "sync-hashes", &cut.hex(), VAULT_NAME],
    ));

    checked.wants(
        "a manifest that was named crosses",
        said.success() && holds(&vault_store, &[cut]).await,
        &format!(
            "it ended with {:?}: {} | the Workspace still holds it: {}",
            said.code,
            said.stderr.trim(),
            holds(&workspace_store, &[cut]).await
        ),
    );

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

    // The manifest crossing is not the content crossing: what a key promises is its content, and a
    // cut content is its chunks. Each chunk the manifest names must be at the Vault too, or the key
    // is one the Vault cannot put back together however right the manifest beside it reads.
    let chunks: Vec<Key> = cut_on_the_workspace
        .as_ref()
        .map(|manifest| manifest.chunks().iter().map(|chunk| chunk.key()).collect())
        .unwrap_or_default();
    checked.wants(
        "every chunk the manifest names crossed with it",
        !chunks.is_empty() && holds(&vault_store, &chunks).await,
        "the manifest crossed without the chunks it names",
    );

    // A key only the Vault holds is named, and crosses back.
    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "storage",
            "sync-hashes",
            &only_in_the_vault.hex(),
            VAULT_NAME,
        ],
    ));

    checked.wants(
        "a key only the other end holds crosses back",
        said.success() && holds(&workspace_store, &[only_in_the_vault]).await,
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A hash neither end holds is something the exchange cannot carry, and is reported as such
    // rather than passed over: what was named is not there to move.
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "sync-hashes", &"ab".repeat(32), VAULT_NAME],
    ));

    checked.wants(
        "a key neither end holds is reported rather than passed over",
        said.code == Some(149),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A word that is not a hash is refused rather than taken for one.
    let said = run(&mut client(
        &workspace,
        &data,
        &["storage", "sync-hashes", "not-a-hash", VAULT_NAME],
    ));

    checked.wants(
        "a word that is not a hash is refused",
        said.code == Some(86),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    serving.stop();
    checked.report();
}

/// The key a `write-file` named as a manifest, from what it printed.
fn manifest_of(stdout: &str) -> Option<Key> {
    stdout.trim().strip_prefix("manifest:")?.trim().parse().ok()
}

/// The keys a listing printed, one a line.
fn keys(stdout: &str) -> Vec<Key> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.parse().ok())
        .collect()
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

/// The path as the argument a command is given.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A text long enough to be cut, with a line of its own for each of `count`.
///
/// `tag` says whose text it is, so that two files made here are not the same content: the same
/// content is the same key, and a test that meant two keys would be asking about one.
fn lines(count: usize, tag: &str) -> String {
    use std::fmt::Write as _;

    let mut text = String::new();
    for at in 0..count {
        writeln!(
            text,
            "{tag} line {at} of something written, {}",
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
