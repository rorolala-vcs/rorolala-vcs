//! `sync-chain`: a version chain crosses whole.
//!
//! What a run tells the Vault it holds is a whole chain of versions, and a version is only a meaning
//! once what it stored is in the store that names it. So a push has to carry the content of *every*
//! version in the chain it moves, not just the version at the top — otherwise the chain crosses, the
//! content below it does not, and a later run that moves a pointer back finds a name nothing can
//! produce.
//!
//! The shape is the one a person meets: one workspace makes two versions of a file and syncs them;
//! a second workspace, which holds none of it, checks the file in and moves its pointer back one
//! version. That last step needs the content of the version below the top, and it can only work if
//! the push carried it.
//!
//! A program rather than a `#[test]` for the same reason the other suites are: it runs the built
//! programs the way a caller meets them, over a real socket, with a real daemon serving a real Vault.

use std::fs;
use std::path::Path;
use std::process::Command;

use std::time::Duration;

use librorolala::vault::{KEYS_DIR, Vault};
use librorolala::workspace::Workspace;
use rorolala_utils_sandbox::{Guard, Serving, command, run, serve};

/// The port the Vault serves on.
const PORT: u16 = 7943;

/// The namespace the Vault is registered under.
const NAMESPACE: &str = "chain";

/// The account the client acts as, which is a member of the Vault.
const NAME: &str = "rootuser";

/// The name the Workspaces know the Vault by.
const VAULT_NAME: &str = "origin";

/// How long the Vault is waited for before it is given up on.
const STARTUP: Duration = Duration::from_secs(10);

/// Where the file sits in a Layout: what a checkin names it by.
const REMOTE_PATH: &str = "@/new/hero.psd";

/// Where a checkin puts it on disk, below the Workspace it is checked into.
const LOCAL_PATH: &str = "new/hero.psd";

fn main() {
    let sandbox = Guard::new("sync-chain");
    let data = sandbox.join("data");
    let root = sandbox.join("root");
    let first = sandbox.join("ws1");
    let second = sandbox.join("ws2");

    let mut checked = Checked::default();

    // A Vault that can be served, and one identity admitted to it.
    Vault::create(&root).unwrap_or_else(|error| panic!("making {}: {error:?}", root.display()));
    Workspace::create(&first)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", first.display()));
    let auth = first.join(".rola").join("auth");
    pair(&auth, NAME);
    publish(&auth, &root, NAME);

    run(&mut client(&first, &data, &["account", NAME])).expect_success();
    run(&mut client(&first, &data, &["bind", &link()])).expect_success();

    // Two versions of one file, both cut into chunks: a content kept as chunks is the case a chain
    // below the top is lost for, since what the top names is one manifest and what the version below
    // names is another.
    let file = first.join("hero.psd");
    let older = psd(1);
    let newer = psd(2);

    fs::write(&file, &older).expect("the first save");
    let said = run(&mut client(&first, &data, &["track", ".", "--message", "1"]));
    checked.wants(
        "the first version is recorded",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    fs::write(&file, &newer).expect("the second save");
    let said = run(&mut client(&first, &data, &["track", ".", "--message", "2"]));
    checked.wants(
        "the second version is recorded",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // One version of the file is one content, so a chain of two is two manifests in the Vault — and
    // a run that carried only the top of the chain leaves one.
    let serving = serve_vault(&data, &root);

    let said = run(&mut client(&first, &data, &["sync"]));
    checked.wants(
        "the first workspace syncs",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &first,
        &data,
        &["storage", "ls-remote-manifests", VAULT_NAME],
    ));
    let manifests = said.stdout.lines().filter(|line| !line.trim().is_empty()).count();
    checked.wants(
        "the Vault holds a manifest for every version of the chain",
        said.success() && manifests == 2,
        &format!(
            "it listed {manifests} manifest(s) ({:?}): {}",
            said.code,
            said.stderr.trim()
        ),
    );

    // A second workspace, holding none of what the first made. It acts as the same account: an
    // account is the machine's, and two workspaces that made a pair each would be two members with
    // one name, which the Vault is right to refuse.
    Workspace::create(&second)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", second.display()));
    copy_identity(&first.join(".rola").join("auth"), &second.join(".rola").join("auth"));

    run(&mut client(&second, &data, &["account", NAME])).expect_success();
    run(&mut client(&second, &data, &["bind", &link()])).expect_success();
    let said = run(&mut client(&second, &data, &["fetch"]));
    checked.wants(
        "the second workspace fetches the Vault's Layout",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(&second, &data, &["checkin", REMOTE_PATH, "."]));
    checked.wants(
        "the file is checked in as the Vault holds it",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let checked_in = second.join(LOCAL_PATH);
    checked.wants(
        "the content the checkin needed was brought whole",
        fs::read(&checked_in).is_ok_and(|bytes| bytes == newer),
        "the file at the Vault's path is not what the Vault holds",
    );

    // Moving the pointer back one version needs the version below the top — the content a run that
    // carried only the top of the chain never sent.
    let said = run(&mut client(&second, &data, &["retrack", LOCAL_PATH]));
    checked.wants(
        "the pointer moves back to the version below the top",
        said.success(),
        &format!(
            "it ended with {:?}: {} / {}",
            said.code,
            said.stdout.trim(),
            said.stderr.trim()
        ),
    );
    checked.wants(
        "the version moved back to is the content that version stored",
        fs::read(&checked_in).is_ok_and(|bytes| bytes == older),
        "the file the pointer was moved to is not what the version below the top stored",
    );

    // A run that cannot be carried out must say so — which is what the confirmation at the end of a
    // direction is for. It cannot be made to fail from outside: the Vault makes its own directories
    // as it opens, so taking one away is not taking away its ability to write. What the detector
    // itself does with content that is gone is checked in the store's own tests, where a chunk can
    // be taken away directly.

    serving.stop();
    checked.report();
}

/// A Photoshop document, cut into chunks by the store's own policy for the format.
///
/// It is a real document as far as the policy reads one — a header, a colour mode section, an image
/// resources section, layer records and their channels, and a composite image — so what the store
/// keeps is a manifest naming chunks, which is the case a version below the top is lost for.
fn psd(seed: u8) -> Vec<u8> {
    let layers: Vec<(String, Vec<Vec<u8>>)> = (0..20)
        .map(|at| {
            let mut data = vec![seed.wrapping_add(u8::try_from(at).unwrap_or(0)); 4000];
            data[0] = u8::try_from(at).unwrap_or(0);
            (format!("layer {at}"), vec![channel(&data), channel(&data)])
        })
        .collect();
    let layers: Vec<(&str, Vec<Vec<u8>>)> = layers
        .iter()
        .map(|(name, data)| (name.as_str(), data.clone()))
        .collect();

    let mut image = vec![seed; 9000];
    image[0] = 7;

    document(&layers, &image)
}

/// A channel's stored bytes: the compression word and what follows it.
fn channel(bytes: &[u8]) -> Vec<u8> {
    let mut data = vec![0, 0];
    data.extend_from_slice(bytes);

    data
}

/// A layer's extra data: no mask, no blending ranges, and a name padded to four bytes.
fn extra(name: &str) -> Vec<u8> {
    let mut extra = Vec::new();
    extra.extend_from_slice(&0_u32.to_be_bytes());
    extra.extend_from_slice(&0_u32.to_be_bytes());

    let name = name.as_bytes();
    extra.push(u8::try_from(name.len()).unwrap_or(0));
    extra.extend_from_slice(name);
    while (extra.len() - 8) % 4 != 0 {
        extra.push(0);
    }

    extra
}

/// One layer record: its rectangle is zero and every channel is numbered as a PSD numbers one.
fn record(name: &str, channels: &[Vec<u8>]) -> Vec<u8> {
    let mut record = vec![0_u8; 16];
    record.extend_from_slice(&u16::try_from(channels.len()).unwrap_or(0).to_be_bytes());

    for (index, data) in channels.iter().enumerate() {
        let id = if index == 0 {
            -1
        } else {
            i16::try_from(index - 1).unwrap_or(0)
        };
        record.extend_from_slice(&id.to_be_bytes());
        record.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
    }

    record.extend_from_slice(b"8BIMnorm");
    record.extend_from_slice(&[255, 0, 0, 0]);

    let extra = extra(name);
    record.extend_from_slice(&u32::try_from(extra.len()).unwrap_or(0).to_be_bytes());
    record.extend_from_slice(&extra);

    record
}

/// Every layer record, and then every layer's channel data, as a PSD lays them down.
fn layer_block(layers: &[(&str, Vec<Vec<u8>>)]) -> Vec<u8> {
    let mut records = Vec::new();
    let mut channels = Vec::new();

    for (name, data) in layers {
        records.extend_from_slice(&record(name, data));
        for channel in data {
            channels.extend_from_slice(channel);
        }
    }

    let mut info = Vec::new();
    info.extend_from_slice(&i16::try_from(layers.len()).unwrap_or(0).to_be_bytes());
    info.extend_from_slice(&records);
    info.extend_from_slice(&channels);

    info
}

/// A PSD holding `layers` and a composite of `image`.
fn document(layers: &[(&str, Vec<Vec<u8>>)], image: &[u8]) -> Vec<u8> {
    let mut content = Vec::new();
    content.extend_from_slice(b"8BPS");
    content.extend_from_slice(&1_u16.to_be_bytes());
    content.extend_from_slice(&[0; 6]);
    content.extend_from_slice(&3_u16.to_be_bytes());
    content.extend_from_slice(&1_u32.to_be_bytes());
    content.extend_from_slice(&1_u32.to_be_bytes());
    content.extend_from_slice(&8_u16.to_be_bytes());
    content.extend_from_slice(&3_u16.to_be_bytes());

    content.extend_from_slice(&0_u32.to_be_bytes());

    let resources = b"resources";
    content.extend_from_slice(&u32::try_from(resources.len()).unwrap_or(0).to_be_bytes());
    content.extend_from_slice(resources);

    let info = layer_block(layers);
    let length = 4 + info.len() + 4;
    content.extend_from_slice(&u32::try_from(length).unwrap_or(0).to_be_bytes());
    content.extend_from_slice(&u32::try_from(info.len()).unwrap_or(0).to_be_bytes());
    content.extend_from_slice(&info);
    content.extend_from_slice(&0_u32.to_be_bytes());

    content.extend_from_slice(&0_u16.to_be_bytes());
    content.extend_from_slice(image);

    content
}

/// A command that runs the client, working in `workspace` and keeping its own files under `data`.
fn client(workspace: &Path, data: &Path, args: &[&str]) -> Command {
    let mut command = command("rola");

    command
        .current_dir(workspace)
        .env("XDG_DATA_HOME", data)
        .env("ROLA_DATA_HOME", data)
        .args(args);

    command
}

/// The link the Vault serving on [`PORT`] is bound by.
///
/// Binding is also what gives a Workspace a Layout: the name it takes is the name the Layout is
/// under, and a run that has no Layout has nothing to check in or sync.
fn link() -> String {
    format!("rola://127.0.0.1:{PORT}/{NAMESPACE}")
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

/// Copies the pair an account is, so a second Workspace acts as the same member.
fn copy_identity(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the keys directory");
    for name in [format!("{NAME}.pem"), format!("{NAME}.pub")] {
        fs::copy(from.join(&name), to.join(&name))
            .unwrap_or_else(|error| panic!("copying {}: {error}", from.join(&name).display()));
    }
}

/// Puts the public half of the pair `name` names, kept under `keys`, where `vault` admits it.
fn publish(keys: &Path, vault: &Path, name: &str) {
    let public = keys.join(format!("{name}.pub"));
    let admitted = vault.join(KEYS_DIR);

    fs::create_dir_all(&admitted).expect("the Vault's keys directory");
    fs::copy(&public, admitted.join(format!("{name}.pub")))
        .unwrap_or_else(|error| panic!("publishing {}: {error}", public.display()));
}

/// Serves the Vault rooted at `dir`, and waits until it is listening.
fn serve_vault(data: &Path, dir: &Path) -> Serving {
    register(data, dir);

    serve(
        command("rola-daemon")
            .arg("--port")
            .arg(PORT.to_string())
            .env("XDG_DATA_HOME", data)
            .env("ROLA_DATA_HOME", data),
        ("127.0.0.1", PORT),
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

/// What was asked of the run, and which of the answers were not what was wanted.
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
            println!("{} questions, every one answered as it should be", self.asked);

            return;
        }

        panic!(
            "{} of {} questions were not answered as they should be",
            self.wrong.len(),
            self.asked
        );
    }
}
