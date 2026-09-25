//! `vault-auth`: what a client finds when it reaches different Vaults.
//!
//! A program rather than a set of tests cargo runs, because what it does is what a person does
//! with a terminal: make a place to work in, put keys in it in one place or another, serve it,
//! reach it, and complain when what came back is not what was expected.
//!
//! There are two Vaults, a root one and one under it — `root/vaults/alpha` — which is what sets
//! the key search walking outward. What is checked is who each of them admits: a member whose
//! public key is kept in the root is a member of everything below it, one kept only below it is
//! a member of that Vault and of nothing above, and one kept nowhere is a member of nothing at
//! all. Each of those is asked of both Vaults, so the answers are what tells the rule apart
//! from "everything is admitted" and from "nothing is".

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::auth::{Account, KeyLocateRule, find_account};
use librorolala::daemon::action_handshake;
use librorolala::vault::{CONFIG_PATH, KEYS_DIR, VAULTS_DIR, Vault};
use librorolala::workspace::{Workspace, locate_workspace};
use rorolala_utils_sandbox::{Guard, Serving, command, run, serve};

/// The port the root Vault serves on.
const ROOT_PORT: u16 = 7911;

/// The port the Vault under it serves on.
const ALPHA_PORT: u16 = 7912;

/// How long a Vault is waited for before it is given up on.
const STARTUP: Duration = Duration::from_secs(10);

fn main() {
    let sandbox = Guard::new("vault-auth");
    let root = sandbox.join("root");
    let alpha = root.join(VAULTS_DIR).join("alpha");
    let workspace = sandbox.join("ws");

    // The two Vaults, each with its own identity and with members kept in one place or the
    // other: the root keeps `rootuser`, the Vault under it keeps `subuser`, and `nobody` is a
    // name neither has ever heard of.
    serveable(&root, ROOT_PORT);
    serveable(&alpha, ALPHA_PORT);

    // The client's own side: the pairs it can act as, and a place to keep them. Its accounts are
    // looked up the way the program looks them up, from the Workspace's keys.
    let keys = workspace.join(".rola").join("auth");
    fs::create_dir_all(&keys).expect("the Workspace's keys");
    pair(&keys, "rootuser");
    pair(&keys, "subuser");
    pair(&keys, "nobody");

    // Who each Vault admits: the root's keys hold `rootuser`, the ones below hold `subuser`, and
    // `nobody` is a name neither has heard of. Only the public half is given to a Vault — the
    // private half is the member's own to keep, which is what makes one account a member of one
    // Vault and of no other.
    publish(&keys, &root, "rootuser");
    publish(&keys, &alpha, "subuser");

    let mut checked = Checked::default();

    // What the root Vault admits. Its own keys hold `rootuser` and nothing else, and there is
    // no Vault above it to look in.
    {
        let serving = serve_vault(&root, ROOT_PORT);
        let address = address(ROOT_PORT);

        checked.wants_admitted(&workspace, "rootuser", &address, "the root holds it");
        checked.wants_refused(
            &workspace,
            "subuser",
            &address,
            "only the Vault below holds it",
        );
        checked.wants_refused(&workspace, "nobody", &address, "nothing holds it");

        // The same daemon serves the Vault it holds when the request names one, so a link to it
        // is answered by the Vault below — under that Vault's own rules, which reach up into the
        // root's keys and no further.
        let alpha_link = link(ROOT_PORT, "alpha");

        checked.wants_admitted(
            &workspace,
            "rootuser",
            &alpha_link,
            "the root above holds it",
        );
        checked.wants_admitted(&workspace, "subuser", &alpha_link, "it holds it itself");
        checked.wants_refused(&workspace, "nobody", &alpha_link, "nothing holds it");

        // A name under the root that is no Vault is refused rather than answered by the root.
        checked.wants_refused(
            &workspace,
            "rootuser",
            &link(ROOT_PORT, "nowhere"),
            "no Vault is served there",
        );

        serving.stop();
    }

    // What the Vault under it admits. Its own keys hold `subuser`, and the search goes outward
    // from there into the root's, so both names are known to it.
    {
        let serving = serve_vault(&alpha, ALPHA_PORT);
        let address = address(ALPHA_PORT);

        checked.wants_admitted(&workspace, "rootuser", &address, "the root above holds it");
        checked.wants_admitted(&workspace, "subuser", &address, "it holds it itself");
        checked.wants_refused(&workspace, "nobody", &address, "nothing holds it");

        serving.stop();
    }

    checked.report();
}

/// What was asked of the Vaults, and how many of them answered as they should have.
#[derive(Default)]
struct Checked {
    /// How many questions were asked.
    asked: usize,
    /// Which of them were answered with something other than what was wanted.
    wrong: Vec<String>,
}

impl Checked {
    /// Asks for `name` to reach `address`, and counts it wrong unless it is admitted.
    fn wants_admitted(&mut self, workspace: &Path, name: &str, address: &str, why: &str) {
        self.asked += 1;

        if let Err(said) = reach(workspace, name, address) {
            self.wrong.push(format!(
                "{name} should have been admitted to {address} — {why}, but: {said}"
            ));
        }
    }

    /// Asks for `name` to reach `address`, and counts it wrong unless it is turned away.
    fn wants_refused(&mut self, workspace: &Path, name: &str, address: &str, why: &str) {
        self.asked += 1;

        if let Ok(said) = reach(workspace, name, address) {
            self.wrong.push(format!(
                "{name} should have been turned away from {address} — {why}, but was told: {said}"
            ));
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

/// Reaches `address` as `name`, from `workspace`, saying what the Vault answered or why it
/// could not be reached.
fn reach(workspace: &Path, name: &str, address: &str) -> Result<String, String> {
    let keys = vec![workspace.join(".rola").join("auth")];
    let account: Account = find_account(name, &keys, &local_only())
        .unwrap_or_else(|| panic!("the client holds no account named {name}"));

    // What the action is taken from is the Workspace the client works in, which is the one
    // the account's key was found in.
    let held: Workspace = locate_workspace(workspace)
        .unwrap_or_else(|| panic!("{} is not a Workspace", workspace.display()));

    action_handshake(&held, &account, address.to_string(), name.to_string())
        .map_err(|error| error.to_string())
}

/// A rule that looks only where the client's own keys are, so that what is checked is what the
/// test put there and not what the machine the test runs on happens to hold.
fn local_only() -> KeyLocateRule {
    KeyLocateRule {
        find_global: false,
        find_local: true,
        find_user: false,
        find_env: false,
    }
}

/// The address a Vault serving on `port` is reached at.
fn address(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// The link a Vault serving on `port` is reached at, naming the Vault `sub` holds.
fn link(port: u16, sub: &str) -> String {
    format!("rola://127.0.0.1:{port}/{sub}")
}

/// Makes `dir` a Vault that can be served: a Vault of its own, listening on `port`, holding
/// the account it proves itself with.
fn serveable(dir: &Path, port: u16) {
    Vault::create(dir).unwrap_or_else(|error| panic!("making {}: {error:?}", dir.display()));

    // The port is the one thing a Vault cannot be talked into, so it is written down before it
    // is served rather than asked for afterwards.
    fs::write(
        dir.join(CONFIG_PATH),
        format!("[daemon_config]\nprefer_port = {port}\n"),
    )
    .expect("writing the Vault's configuration");

    pair(&dir.join(KEYS_DIR), "vault");
}

/// Puts the public half of the pair `name` names, kept under `keys`, where `vault` admits it.
///
/// Only the public half: a member is a name a Vault admits, and the private half stays with
/// whoever acts as that member — which is what makes one account a member of one Vault and of
/// no other.
fn publish(keys: &Path, vault: &Path, name: &str) {
    let public = keys.join(format!("{name}.pub"));
    let admitted = vault.join(KEYS_DIR);

    fs::create_dir_all(&admitted).expect("the Vault's keys directory");
    fs::copy(&public, admitted.join(format!("{name}.pub")))
        .unwrap_or_else(|error| panic!("publishing {}: {error}", public.display()));
}

/// Writes the key pair `name` names under `dir`.
///
/// `openssl` makes it, which is what the program itself uses: a pair that is not one a caller
/// could have made would have this test checking something nobody can do.
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
