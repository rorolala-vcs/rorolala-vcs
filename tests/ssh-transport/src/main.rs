//! `ssh-transport`: a Vault reached over an SSH session, end to end.
//!
//! What is checked is the whole leg a caller meets: an address written `user@host:path`, a
//! Workspace bound to it, `rola` starting `ssh` for the exchange, the far side running
//! `rola-daemon serve <path>`, and the answer coming back through the two standard streams — no
//! port bound anywhere. The stand-in for `ssh` is this program itself: started with the arguments
//! `ssh` is started with, it records them and hands the far side's command to a shell, which is
//! what proves the quoting the client writes arrives as one word.
//!
//! The negative cases are here because they are the ones a caller actually meets: no `ssh` on this
//! machine, no `rola-daemon` on that one, a daemon too old to know `serve`, a path that is not a
//! Vault, a machine that refuses sessions, and a first connection that may or may not be allowed
//! to register the key it proved.
//!
//! A program rather than a `#[test]` for the same reason the other suites are: it runs the built
//! programs the way a caller meets them.

// A Windows machine runs no session of its own here, so what the suite uses only on Unix — the
// report, the sandbox helpers — is allowed to sit unused there rather than be gated twice.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode};

use librorolala::vault::{CONFIG_PATH, Config, KEYS_DIR, Vault};
use librorolala::workspace::Workspace;
use rorolala_utils_configure::Configure as _;
use rorolala_utils_sandbox::{Guard, Ran, bin_dir, command, run};

/// The variable that asks the stand-in `ssh` to behave as a particular failure.
const MODE_VAR: &str = "ROLA_SSH_MODE";

/// The variable that names the file each invocation of the stand-in records its arguments in.
const RECORD_VAR: &str = "ROLA_SSH_RECORD";

/// The account the client acts as, which the Vault admits from the start.
const CLIENT: &str = "rootuser";

/// The name the Workspace knows the Vault by.
const VAULT_NAME: &str = "alpha";

/// The exit code of an action that could not reach its Vault.
const TRANSPORT_FAILED: i32 = 152;

fn main() -> ExitCode {
    // The same program stands in for `ssh`, so it is what `ssh` is started with that says which
    // of its two lives it is living: only a session is started with `-T`. A Windows machine starts
    // no session of its own, so the stand-in has no second life to live there and is not compiled.
    #[cfg(unix)]
    {
        if std::env::args().nth(1).as_deref() == Some("-T") {
            return stand_in_ssh();
        }
    }

    // The far side of a session is a login shell, which is what a Windows machine running this
    // suite does not have. The server side of the feature is Unix-only for now, and the suite says
    // so rather than failing on what it was never meant to check.
    #[cfg(not(unix))]
    {
        println!("ssh-transport/ skipped: a session's far side is a Unix shell");
        return ExitCode::SUCCESS;
    }

    #[cfg(unix)]
    {
        checks();
        ExitCode::SUCCESS
    }
}

#[cfg(unix)]
fn checks() {
    let sandbox = Guard::new("ssh-transport");
    let data = sandbox.join("data");
    let root = sandbox.join("vaults").join(VAULT_NAME);
    let workspace = sandbox.join("workspace");
    let record = sandbox.join("ssh-arguments");
    let mut checked = Checked::default();

    // One Vault, one Workspace, and two identities: the one the work acts as, which the Vault
    // admits, and the daemon's own, which is what proves the far side.
    Vault::create(&root).unwrap_or_else(|error| panic!("making {}: {error:?}", root.display()));
    Workspace::create(&workspace)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", workspace.display()));
    let auth = workspace.join(".rola").join("auth");
    pair(&auth, CLIENT);
    publish(&auth, &root, CLIENT);
    pair(&data.join("rola").join("keys"), "daemon");

    run(&mut client(
        &workspace,
        &data,
        &record,
        &["account", CLIENT],
    ))
    .expect_success();

    // The address is written the way a person writes an scp destination: the account, the host,
    // and the path of the Vault on that host.
    let link = format!("{CLIENT}@localhost:{}", root.display());
    let said = run(&mut client(
        &workspace,
        &data,
        &record,
        &["bind", &link, VAULT_NAME],
    ));
    checked.wants(
        "a Vault behind SSH is bound by an scp-style address",
        said.success(),
        &said.failed(),
    );

    // A file, so that a second action has something of the Vault's to ask for.
    fs::write(workspace.join("hero.psd"), b"a picture").expect("the file");
    run(&mut client(
        &workspace,
        &data,
        &record,
        &["track", ".", "--message", "first"],
    ))
    .expect_success();

    let said = run(&mut client(
        &workspace,
        &data,
        &record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "the handshake reaches the Vault through the session",
        said.success() && said.stdout.contains("I'm Alpha"),
        &said.failed(),
    );

    // Nothing had written this machine's preferences yet, and the session asked them what it may do:
    // what a machine that has said nothing means is now the file it can be changed in.
    let preferences = data.join("rola").join("daemon").join("preference.toml");
    checked.wants(
        "a machine that has said nothing has its preferences written down by the first session",
        fs::read_to_string(&preferences).is_ok_and(|said| {
            said.contains("[security]")
                && said.contains("ssh-connection = true")
                && said.contains("auto-register-by-ssh = false")
        }),
        &format!("{}", preferences.display()),
    );

    // From here on each check turns the switches it needs, so what differs between two of them is the
    // switch rather than whatever happened to be on disk.
    preference(&data, true, false);

    // Every action opens a session of its own, so the second one is a second program rather than a
    // channel being reused: what is checked is that it works at all, which is the whole of what
    // reusing a connection would be for.
    let said = run(&mut client(
        &workspace,
        &data,
        &record,
        &["storage", "ls-remote-manifests", VAULT_NAME],
    ));
    checked.wants(
        "a second action opens a second session",
        said.success(),
        &said.failed(),
    );

    // What the client asked `ssh` for: the account as an option's value, no terminal, nothing
    // asked of a person, and the far side's command as one argument.
    let recorded = fs::read_to_string(&record).unwrap_or_default();
    let last = recorded
        .lines()
        .last()
        .unwrap_or_default()
        .replace(char::from(31), " ");
    checked.wants(
        "ssh is started without a terminal and without asking anything",
        last.contains("-T") && last.contains("-o BatchMode=yes"),
        &last,
    );
    checked.wants(
        "the account travels as an option value and the far side's command as one argument",
        last.contains("-l rootuser")
            && last.contains(&format!("rola-daemon serve '{}'", root.display())),
        &last,
    );

    first_connection(&workspace, &data, &record, &root, &mut checked);
    failures(&workspace, &data, &record, &root, &mut checked);

    checked.report();
}

/// What a first connection does about a name the Vault has never seen.
#[cfg(unix)]
fn first_connection(
    workspace: &Path,
    data: &Path,
    record: &Path,
    root: &Path,
    checked: &mut Checked,
) {
    let auth = workspace.join(".rola").join("auth");
    let newcomer = "newcomer";
    pair(&auth, newcomer);
    run(&mut client(workspace, data, record, &["account", newcomer])).expect_success();

    let keys = root.join(KEYS_DIR);
    let written = keys.join(format!("{newcomer}.pub"));

    // A name that has never been seen is refused while the machine registers nobody...
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "a name the Vault has never seen is refused when nothing may be registered",
        said.code == Some(TRANSPORT_FAILED) && !written.exists(),
        &said.failed(),
    );

    // ...and is registered by the connection that proves its key, once the machine allows it.
    preference(data, true, true);
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "a name the Vault has never seen is registered when the machine allows it",
        said.success() && said.stdout.contains("I'm Alpha"),
        &said.failed(),
    );
    checked.wants(
        "what is registered is the key the connection proved",
        fs::read_to_string(&written).is_ok_and(|registered| {
            registered.contains("BEGIN PUBLIC KEY")
                && registered
                    == fs::read_to_string(auth.join(format!("{newcomer}.pub"))).unwrap_or_default()
        }),
        &format!("{}", written.display()),
    );

    // The switch is turned back off: the name now exists, so the next connection needs nothing
    // registered and is served from the key that is already there.
    preference(data, true, false);
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "a registered name is served with the switch off",
        said.success(),
        &said.failed(),
    );

    // Deleting the key with the switch on is not a revocation: the next connection is the first
    // one again, and writes it back. That is what the documentation has to say, and this is where
    // it is held to it.
    preference(data, true, true);
    fs::remove_file(&written).expect("taking the key away");
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "a deleted key is registered again while the machine registers connections",
        said.success() && written.is_file(),
        &said.failed(),
    );

    // With the switch off, the same deletion is a revocation.
    preference(data, true, false);
    fs::remove_file(&written).expect("taking the key away");
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "a deleted key is a revocation while the machine registers nothing",
        said.code == Some(TRANSPORT_FAILED) && !written.exists(),
        &said.failed(),
    );

    // An administrator's name is never registered by a connection, however much the machine
    // allows: an administrator can move any file in the Vault.
    // The Vault made by the suite names no administrator; one is named here, as a person names the
    // first one by hand, so that a connection claiming the name has a Vault that would admit it as
    // an administrator if it were admitted at all.
    let settings = root.join(CONFIG_PATH);
    let mut configured = Config::read_from(&settings).expect("the Vault's configuration");
    configured.auth_config_mut().add_admin(newcomer);
    configured
        .write_to(&settings)
        .expect("the Vault's configuration");

    preference(data, true, true);
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "an administrator's name is not registered by a connection",
        said.code == Some(TRANSPORT_FAILED) && !written.exists(),
        &said.failed(),
    );
}

/// Every way the leg itself can fail, and what the caller is told about each.
#[cfg(unix)]
fn failures(workspace: &Path, data: &Path, record: &Path, root: &Path, checked: &mut Checked) {
    run(&mut client(workspace, data, record, &["account", CLIENT])).expect_success();
    preference(data, true, false);

    // A machine that does not allow sessions refuses them before any Vault is looked at.
    preference(data, false, false);
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "a machine that refuses sessions says so",
        said.code == Some(TRANSPORT_FAILED) && said.stderr.contains("refuses SSH sessions"),
        &said.failed(),
    );
    preference(data, true, false);

    // No `ssh` here at all.
    let said = run(&mut client_with_ssh(
        workspace,
        data,
        record,
        &["vault", "handshake"],
        Some("/nonexistent/ssh"),
    ));
    checked.wants(
        "no ssh on this machine is said to be no ssh on this machine",
        said.code == Some(TRANSPORT_FAILED) && said.stderr.contains("could not be started"),
        &said.failed(),
    );

    // `ssh` itself failing, which is what an unknown host or a refused key looks like.
    for (mode, expected) in [
        ("fail", "could not be established"),
        ("garbage", "ended before the protocol"),
        ("quiet", "ended before the protocol"),
        ("no-daemon", "has no"),
        ("old-daemon", "does not know"),
    ] {
        let said = run(&mut client_mode(
            workspace,
            data,
            record,
            &["vault", "handshake"],
            mode,
        ));
        checked.wants(
            &format!("a session that failed with {mode} says so"),
            said.code == Some(TRANSPORT_FAILED) && said.stderr.contains(expected),
            &said.failed(),
        );
    }

    // A path that is not a Vault on the far side: what the daemon there ends with is what says so.
    let plain = workspace.parent().unwrap_or(workspace).join("not-a-vault");
    fs::create_dir_all(&plain).expect("a directory that is not a Vault");
    let link = format!("{CLIENT}@localhost:{}", plain.display());
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["bind", &link, "plain"],
    ));
    checked.wants(
        "a path that is not a Vault is bound and then refused by the far side",
        said.success(),
        &said.failed(),
    );
    let said = run(&mut client(
        workspace,
        data,
        record,
        &["vault", "handshake"],
    ));
    checked.wants(
        "a path that is not a Vault is said to be no Vault",
        said.code == Some(TRANSPORT_FAILED)
            && said.stderr.contains("not a Vault on the remote host"),
        &said.failed(),
    );

    // The Vault is bound again, so the checks after this one reach the one that is there.
    let link = format!("{CLIENT}@localhost:{}", root.display());
    run(&mut client(
        workspace,
        data,
        record,
        &["bind", &link, VAULT_NAME],
    ))
    .expect_success();
}

/// A command that runs the client, working in `workspace` and keeping its own files under `data`.
#[cfg(unix)]
fn client(workspace: &Path, data: &Path, record: &Path, args: &[&str]) -> Command {
    client_with_ssh(workspace, data, record, args, None)
}

/// As [`client`], with a particular mode asked of the stand-in `ssh`.
#[cfg(unix)]
fn client_mode(workspace: &Path, data: &Path, record: &Path, args: &[&str], mode: &str) -> Command {
    let mut command = client(workspace, data, record, args);
    command.env(MODE_VAR, mode);

    command
}

/// As [`client`], with the program started in place of `ssh` named.
#[cfg(unix)]
fn client_with_ssh(
    workspace: &Path,
    data: &Path,
    record: &Path,
    args: &[&str],
    ssh: Option<&str>,
) -> Command {
    let mut built = command("rola");

    // The far side of a session is a shell, which finds `rola-daemon` the way a person's shell
    // would: on the program path the suite was given.
    let path = format!(
        "{}:{}",
        bin_dir().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    built
        .current_dir(workspace)
        .env("XDG_DATA_HOME", data)
        .env("ROLA_DATA_HOME", data)
        .env("PATH", path)
        .env(RECORD_VAR, record)
        .env(
            "ROLA_SSH",
            ssh.map_or_else(
                || {
                    std::env::current_exe()
                        .expect("the program running the suite")
                        .display()
                        .to_string()
                },
                str::to_owned,
            ),
        )
        .args(args);

    built
}

/// Writes this machine's preferences: what a session may be served and what it may register.
#[cfg(unix)]
fn preference(data: &Path, ssh_connection: bool, auto_register: bool) {
    let directory = data.join("rola").join("daemon");
    fs::create_dir_all(&directory).expect("the daemon's directory");

    fs::write(
        directory.join("preference.toml"),
        format!(
            "[security]\nssh-connection = {ssh_connection}\nauto-register-by-ssh = {auto_register}\n"
        ),
    )
    .expect("this machine's preferences");
}

/// Writes the key pair `name` names under `dir`.
#[cfg(unix)]
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

/// Puts the public half of the pair `name` names, kept under `keys`, where `vault` admits it.
#[cfg(unix)]
fn publish(keys: &Path, vault: &Path, name: &str) {
    let public = keys.join(format!("{name}.pub"));
    let admitted = vault.join(KEYS_DIR);

    fs::create_dir_all(&admitted).expect("the Vault's keys directory");
    fs::copy(&public, admitted.join(format!("{name}.pub")))
        .unwrap_or_else(|error| panic!("publishing {}: {error}", public.display()));
}

/// The stand-in for `ssh`: records what it was asked, then runs the far side's command.
#[cfg(unix)]
fn stand_in_ssh() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    if let Some(path) = std::env::var_os(RECORD_VAR) {
        let mut record = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("the argument record");
        // One line per invocation, with a character no argument carries between the arguments:
        // what a caller built is read back as it was written, not as it was joined for reading.
        writeln!(record, "{}", arguments.join(&char::from(31).to_string()))
            .expect("the argument record");
    }

    // What the run asked this stand-in to be: a machine that cannot be reached, one that answers
    // with something that is not the protocol, one with no daemon, one whose daemon is too old.
    match std::env::var(MODE_VAR).as_deref().unwrap_or_default() {
        "fail" => return ExitCode::from(255),
        "garbage" => {
            println!("this is not a rola daemon");
            return ExitCode::SUCCESS;
        }
        "quiet" => return ExitCode::SUCCESS,
        "no-daemon" => return ExitCode::from(127),
        "old-daemon" => return ExitCode::from(13),
        _ => {}
    }

    let Some((_, far_side)) = destination_and_command(&arguments) else {
        eprintln!("the stand-in ssh was given arguments it does not read: {arguments:?}");

        return ExitCode::from(2);
    };

    // The far side of a session is a login shell, which is exactly what has to read the quoting:
    // what arrives there has to be the path as one word, whatever it carries.
    let status = Command::new("sh")
        .arg("-c")
        .arg(&far_side)
        .env("SSH_CONNECTION", "127.0.0.1 4444 127.0.0.1 22")
        .status()
        .expect("the far side's shell");

    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::SUCCESS, ExitCode::from)
}

/// Where the far side is and what is to run there, out of the arguments `ssh` reads.
#[cfg(unix)]
fn destination_and_command(arguments: &[String]) -> Option<(String, String)> {
    // Everything before the separator is an option; the two after it are where to go and what to
    // run there.
    let separator = arguments.iter().position(|argument| argument == "--")?;

    Some((
        arguments.get(separator + 1)?.clone(),
        arguments.get(separator + 2)?.clone(),
    ))
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
            println!(
                "{} questions, every one answered as it should be",
                self.asked
            );

            return;
        }

        panic!(
            "{} of {} questions were answered wrongly",
            self.wrong.len(),
            self.asked
        );
    }
}

/// What a run said, as one line to report a failure with.
trait Failed {
    /// The exit code and both streams, which is all a failed run can be read from.
    fn failed(&self) -> String;
}

impl Failed for Ran {
    fn failed(&self) -> String {
        format!(
            "it ended with {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code,
            self.stdout.trim(),
            self.stderr.trim()
        )
    }
}
