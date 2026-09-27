//! `vcs-index-remote`: what `rola vcs-index ls-remote-*` lists and `read-remote` reads.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: it
//! makes a place to work in, keeps an account, binds a Vault, puts objects in the Vault's index,
//! serves the Vault, and then asks the two questions these commands exist for.
//!
//! What is checked is that a remote listing is *the other end's*, not the run's own, and that a
//! remote read answers with the object the Vault holds — a Version with the number the Vault traced
//! its chain to — while leaving both indexes as they were. The listing and the read are the two
//! halves of the same reaching, which is why a listing is hashes and the object is a read besides.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::storage::{Key, StorageBackend as _};
use librorolala::vault::{CONFIG_PATH, KEYS_DIR, Vault};
use librorolala::vcs::{Creator, Message, ROOT_VERSION, VCSIndex, VCSWrite as _, Version};
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
    let sandbox = Guard::new("vcs-index-remote");
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

    // The Vault's index holds a whole little chain: a creator, a message, the root version, a variant
    // based on the root, and the version made of that variant. The number of the version is 0, and the
    // root's is the root number — both are the Vault's to work out, and neither is written down.
    let vault_index = VCSIndex::create(root.join("index"));
    let creator = Creator::try_from(NAME).expect("a name");
    let message = Message::try_from("first").expect("a message");
    let root_version = Version::root();
    let variant = root_version.new_variant(
        [0x42; 32],
        *creator.hash().digest(),
        *message.hash().digest(),
    );
    let version = variant.new_version();

    let creator_key = vault_index.write(creator.clone()).await.expect("a creator");
    let message_key = vault_index.write(message.clone()).await.expect("a message");
    let root_key = vault_index.write(root_version).await.expect("a root");
    let variant_key = vault_index.write(variant.clone()).await.expect("a variant");
    let version_key = vault_index.write(version.clone()).await.expect("a version");

    let serving = serve_vault(&root, PORT);
    let mut checked = Checked::default();

    // A listing is the other end's, and it names hashes — one kind per command, so a listing of the
    // variants does not name the versions.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "ls-remote-variants", VAULT_NAME],
    ));
    let listed = keys(&said.stdout);

    checked.wants(
        "a listing of the other end's variants is answered",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "the Vault's variant is listed",
        listed.contains(&variant_key.hex()),
        &format!("it listed {:?}", said.stdout.trim()),
    );
    checked.wants(
        "a listing of the variants names no version",
        !listed.contains(&version_key.hex()),
        "a listing of the variants named a version",
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "ls-remote-versions", VAULT_NAME],
    ));
    let listed = keys(&said.stdout);

    checked.wants(
        "the Vault's version is listed",
        listed.contains(&version_key.hex()) && !listed.contains(&variant_key.hex()),
        &format!("it listed {:?}", said.stdout.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "ls-remote-str", VAULT_NAME],
    ));
    let string_hashes = keys(&said.stdout);

    checked.wants(
        "the Vault's text objects are listed",
        string_hashes.contains(&creator_key.hex()) && string_hashes.contains(&message_key.hex()),
        &format!("it listed {:?}", said.stdout.trim()),
    );

    // The listing, as a program reads it: `--json` answers with the hashes whole.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "ls-remote-variants", VAULT_NAME, "--json"],
    ));

    checked.wants(
        "a listing read as a program is written down whole",
        said.stdout.contains("\"string_hashes\"") && said.stdout.contains(&variant_key.hex()),
        &format!("it said {:?}", said.stdout.trim()),
    );

    // A remote read answers with the object the Vault holds. The variant is named with the version
    // it is based on and the storage entry it points at; a Creator is its text.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "read-remote", &variant_key.hex(), VAULT_NAME],
    ));

    checked.wants(
        "the Vault's variant is read",
        said.success()
            && said.stdout.contains(&variant_key.hex())
            && said.stdout.contains(&root_key.hex())
            && said.stdout.contains(
                &variant
                    .storage_hash()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            ),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // The version is numbered on the Vault, since the chain is there: the first version of a file is
    // numbered 0.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "read-remote", &version_key.hex(), VAULT_NAME],
    ));

    checked.wants(
        "the Vault traces the number of the version it reads",
        said.success()
            && said.stdout.contains(&format!("0:{}", version_key.hex()))
            && said.stdout.contains(&variant_key.hex()),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // The root version is the version before the first: its number is the root number, shown as it is
    // rather than made a special case of.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "read-remote", &root_key.hex(), VAULT_NAME],
    ));

    checked.wants(
        "the root version reads with the root number",
        said.success()
            && said
                .stdout
                .contains(&format!("{ROOT_VERSION}:{}", root_key.hex())),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "read-remote", &creator_key.hex(), VAULT_NAME],
    ));

    checked.wants(
        "the Vault's Creator is read as its text",
        said.stdout.trim() == NAME,
        &format!("it said {:?}", said.stdout.trim()),
    );

    // The composition the commands are made for: what one lists, the other reads.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "ls-remote-str", VAULT_NAME],
    ));
    let texts: Vec<String> = keys(&said.stdout)
        .iter()
        .filter_map(|hash| {
            let said = run(&mut client(
                &workspace,
                &data,
                &["vcs-index", "read-remote", hash, VAULT_NAME],
            ));
            said.success().then(|| said.stdout.trim().to_owned())
        })
        .collect();

    checked.wants(
        "what a listing names is what the read answers with",
        texts.contains(&NAME.to_owned()) && texts.contains(&"first".to_owned()),
        &format!("it read {texts:?}"),
    );

    // A hash the Vault does not hold is answered as nothing stored, not as a failed exchange.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "read-remote", &"ab".repeat(32), VAULT_NAME],
    ));

    checked.wants(
        "a hash the Vault does not hold is not found",
        said.code == Some(124),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A word that is not a hash is refused before anything is reached for.
    let said = run(&mut client(
        &workspace,
        &data,
        &["vcs-index", "read-remote", "not-a-hash", VAULT_NAME],
    ));

    checked.wants(
        "a word that is not a hash is refused",
        said.code == Some(123),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // Nothing any of it did changed either index: the Workspace's index never held the Vault's
    // objects, and reading did not bring them.
    let workspace_index = VCSIndex::create(workspace.join(".rola/index"));
    checked.wants(
        "reading the other end does not bring its objects here",
        !holds(
            &workspace_index,
            &[variant_key, version_key, creator_key, message_key],
        )
        .await,
        "a read brought the other end's objects into this index",
    );
    checked.wants(
        "listing the other end leaves its index as it was",
        holds(&vault_index, &[variant_key, version_key, root_key]).await,
        "a listing changed the other end's index",
    );

    serving.stop();
    checked.report();
}

/// The hashes a listing printed, one a line, without the colour a terminal would be shown.
fn keys(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(strip_colour)
        .flat_map(|line| {
            line.split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The text of `line` with any terminal escape sequences taken out.
///
/// Output that has been piped is not read by a terminal, so what it holds is the line itself — but a
/// check that compares against it should not depend on that. This takes the escapes out either way.
fn strip_colour(line: &str) -> String {
    let mut clean = String::with_capacity(line.len());
    let mut rest = line;

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
