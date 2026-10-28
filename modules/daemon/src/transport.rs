//! Reaching the daemon an action is spoken to.
//!
//! An address says where a Vault is; this is what makes the stream a session runs over. What that
//! comes to is the address's own to say: a daemon is a TCP connection made here, while a Vault
//! behind an SSH session is a program started here, whose standard input and output carry the
//! exchange. The protocol that runs over either is the same one, which is why the choice is made
//! once, here, and nothing above this ever asks which it got.
//!
//! # Why it is not part of `protocol`
//!
//! `protocol` says what an address is and what travels over one. What *reaches* an address is a
//! client's business — this crate is both halves of the exchange — and it is here rather than
//! there for that reason, and because a protocol library that held a TCP client and a process
//! launcher would be one that had to be linked against a network stack and a shell to say what a
//! namespace is.
//!
//! # Why an SSH session is a process
//!
//! `ssh` is the only part of this that knows how to authenticate to a host, and it already does:
//! it reads the user's configuration, their `known_hosts`, their agent, and their keys. Rather
//! than teach this program to do any of that, one `ssh` is started per action and the two ends
//! talk over its standard streams — which the program has to keep clean, since they are the
//! protocol and nothing else.

use std::fmt;
use std::io;
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use std::time::Duration;

use rorolala_protocol::{ActionError, Link, Socket, VaultAddress};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::watch;

/// The program that serves a Vault on the far side of a session.
const REMOTE_PROGRAM: &str = "rola-daemon";

/// The subcommand that serves one connection over the far side's standard streams.
///
/// It is a contract across versions rather than an implementation detail: an older `rola-daemon`
/// answers a command it does not know with the code below, which is what lets a caller here say
/// that the far side has to be upgraded rather than that the path is wrong.
const REMOTE_COMMAND: &str = "serve";

/// The program a session is made with.
const SSH_PROGRAM: &str = "ssh";

/// The variable that names the program a session is made with, when the machine's own is not it.
///
/// It is what a machine whose `ssh` is somewhere unusual says, and what a suite points at a
/// stand-in program so that what is checked is the arguments this side builds rather than whether
/// the machine has a host to reach.
const SSH_ENV_VAR: &str = "ROLA_SSH";

/// What `ssh` itself exits with when it could not make the connection.
///
/// It is reserved by `ssh` for its own failures — a host that is not known, a key that is not
/// accepted, a name that does not resolve — as against whatever the remote command said.
const SSH_FAILURE: u8 = 255;

/// What a shell exits with when the command it was asked to run is not there.
const NOT_FOUND: u8 = 127;

/// What this program's own commands exit with when they do not know the command they were given.
///
/// It is `rola-daemon`'s number for that, and an older one that does not know `serve` answers
/// with it.
const UNKNOWN_COMMAND: u8 = 13;

/// How long the failing end of a session is waited for once the exchange has failed.
///
/// The program has already closed its output by then, since that is what a session fails on, so
/// this is the wait for the last few instructions of a process that is ending — not for one that
/// is still working.
const ENDED_WITHIN: Duration = Duration::from_secs(2);

/// Why the daemon an address names could not be reached.
///
/// What it carries is the leg that failed rather than a name for it: a caller that reports this
/// reports the sentence, and a reader is told which of the things between here and a Vault's
/// answer went wrong.
#[derive(Debug)]
pub(crate) enum ConnectError {
    /// The TCP connection to a daemon could not be made.
    Tcp(io::Error),
    /// The SSH session could not be started.
    Ssh {
        /// What happened, naming the program that was to be started.
        message: String,
    },
}

impl fmt::Display for ConnectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp(source) => write!(formatter, "the daemon could not be reached: {source}"),
            Self::Ssh { message } => formatter.write_str(message),
        }
    }
}

/// A stream to what an address names, and what was started to reach it.
pub(crate) struct Dialed {
    /// The stream the session runs over.
    pub(crate) socket: Box<dyn Socket>,
    /// What the program that carries the session ends with, where a program carries it.
    ///
    /// A daemon is a connection and has no such thing; an SSH session is a process, and its exit
    /// code is the only thing that can say *why* it ended — a path that is not a Vault, a remote
    /// program that is not installed, a remote daemon that refuses sessions at all.
    pub(crate) exit: Option<ExitWatch>,
}

/// Makes the stream a session with `address` runs over.
pub(crate) async fn dial(address: &VaultAddress) -> Result<Dialed, ConnectError> {
    match address.link() {
        Link::Daemon { port, .. } => dial_tcp(address, *port).await,
        Link::Ssh {
            user,
            host,
            port,
            path,
        } => dial_ssh(user, host, *port, path),
    }
}

/// Turns a failure the exchange met into what a caller is told about it.
///
/// A daemon link has no program to ask, and what the exchange itself said is the whole story. An
/// SSH session has one, and its exit code says the things the exchange cannot: the exchange only
/// ever sees its end of the stream close.
pub(crate) async fn explain(error: ActionError, exit: Option<&ExitWatch>) -> ActionError {
    // What already says which leg failed is not improved by asking a program about it.
    let Some(exit) = exit else {
        return error;
    };

    if matches!(error, ActionError::Transport(_)) {
        return error;
    }

    match exit.ended().await.map(|ended| ended.code) {
        // No program ended within the bound, so nothing is known about why it is over: what the
        // exchange said is reported.
        None => error,
        // A program that ended well ended after doing what it was asked. A session that still
        // failed before its first exchange is one where nothing spoke the protocol at all, which
        // is what a login script writing to the stream looks like from here.
        Some(Some(0)) => match error {
            ActionError::Auth(_) | ActionError::Io(_) => ActionError::Transport(
                "the session ended before the protocol did: the far side's standard output must \
                 carry nothing but the daemon's answer, so a login script that prints breaks it"
                    .to_owned(),
            ),
            other => other,
        },
        Some(Some(code)) => ActionError::Transport(ended_message(code)),
        // A program ended by a signal said nothing about why, and there is nothing better to
        // report than that it was stopped.
        Some(None) => ActionError::Transport(
            "the SSH program was stopped before the session finished".to_owned(),
        ),
    }
}

/// What a session program ending with `code` means to a caller here.
///
/// The codes are three kinds, and what they are is read off the number because that is what a
/// program's end is: `ssh`'s own, this program's, and a shell's.
fn ended_message(code: i32) -> String {
    let Ok(code) = u8::try_from(code) else {
        return format!("the session ended with an exit code no program uses ({code})");
    };

    match code {
        SSH_FAILURE => "the SSH connection could not be established: what `ssh` said is reported above"
            .to_owned(),
        crate::exit::NO_IDENTITY => {
            "the remote `rola-daemon` holds no account to prove itself with".to_owned()
        }
        crate::exit::NO_VAULT => {
            "the path the address names is not a Vault on the remote host".to_owned()
        }
        crate::exit::SESSION_FAILED => {
            "the remote `rola-daemon` failed the session: what it said is reported above"
                .to_owned()
        }
        crate::exit::SSH_DISABLED => "the remote `rola-daemon` refuses SSH sessions: set `ssh-connection = true` in its `preference.toml`".to_owned(),
        crate::exit::PREFERENCE_UNREADABLE => {
            "the remote `preference.toml` could not be read: fix or remove it on the remote host"
                .to_owned()
        }
        NOT_FOUND => "the remote host has no `rola-daemon` to run: install it there, or put it on the remote `PATH`".to_owned(),
        UNKNOWN_COMMAND => "the remote `rola-daemon` does not know the `serve` command: upgrade it on the remote host".to_owned(),
        _ => format!("the remote `rola-daemon` stopped with exit code {code}"),
    }
}

/// Makes the TCP connection a daemon at `address` answers on.
///
/// The target is the host as the link writes it and the port beside it, so an ipv6 address is
/// bracketed exactly where it is bracketed in what a configuration keeps.
async fn dial_tcp(address: &VaultAddress, port: u16) -> Result<Dialed, ConnectError> {
    let target = format!("{}:{port}", address.link().written_host());
    let stream = TcpStream::connect(target.as_str())
        .await
        .map_err(ConnectError::Tcp)?;

    // A request and its answer are small and alternate, and a sealed record is written as its own
    // buffer: left to Nagle, the second record of an exchange waits for the first to be
    // acknowledged, which is a round trip a small action spends on nothing. The whole session is
    // that shape, so the delay is turned off for the whole of it.
    stream.set_nodelay(true).map_err(ConnectError::Tcp)?;

    Ok(Dialed {
        socket: Box::new(stream),
        exit: None,
    })
}

/// Starts the SSH session that serves the Vault at `path` on the far side.
///
/// Starting a program is not an awaited thing: what is awaited is the exchange over it, which is
/// the caller's to do.
fn dial_ssh(user: &str, host: &str, port: Option<u16>, path: &str) -> Result<Dialed, ConnectError> {
    let program = ssh_program();
    let mut command = Command::new(&program);

    command
        .args(ssh_arguments(user, host, port, path))
        // The two streams are the protocol, and the third is not: what `ssh` says about a host
        // key, a refused key, or a name that does not resolve is what a person has to read, and
        // so is whatever the remote command writes — the audit line of a registration, or the
        // reason it would not serve.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        // A run that ends without taking the session down leaves a program holding a connection
        // open on both ends; the stream going out of scope is what stops it.
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|error| ConnectError::Ssh {
        message: unstarted(&program, &error),
    })?;

    // UNWRAP: both were asked for as pipes just above, so a program that started has them.
    let stdin = child
        .stdin
        .take()
        .expect("a piped standard input is there once the program starts");
    // UNWRAP: the same pipe, asked for in the same place.
    let stdout = child
        .stdout
        .take()
        .expect("a piped standard output is there once the program starts");

    let (ended, exit) = watch::channel(None);
    let (kill, stopped) = tokio::sync::oneshot::channel();

    // What the program ends with is watched from the moment it starts rather than from the moment
    // its output ends, since the two are not the same failure: a session whose first write cannot
    // be made — a program that ended before it read — never sees the end of an output it never
    // read, and that is exactly the case a caller has to be told about.
    tokio::spawn(watch(child, stopped, ended));

    Ok(Dialed {
        socket: Box::new(SshSocket {
            // The pipe to a program that ends before it reads is a failure to report, not a reason
            // to die silently — see `SigpipeGuard`.
            _signal: SigpipeGuard::set_aside(),
            stdin,
            stdout,
            kill: Some(kill),
        }),
        exit: Some(ExitWatch(exit)),
    })
}

/// The program a session is made with, as the machine or the run names it.
fn ssh_program() -> String {
    std::env::var(SSH_ENV_VAR)
        .ok()
        .filter(|program| !program.is_empty())
        .unwrap_or_else(|| SSH_PROGRAM.to_owned())
}

/// What is said about a program that could not be started at all.
fn unstarted(program: &str, error: &io::Error) -> String {
    // On Windows the SSH client is an optional feature rather than something the system has,
    // which is the one case where "it is not installed" is the usual reason and the one a reader
    // needs to be told how to fix.
    #[cfg(windows)]
    let hint = " (on Windows, add the OpenSSH Client optional feature)";
    #[cfg(not(windows))]
    let hint = "";

    format!("the SSH program `{program}` could not be started: {error}{hint}")
}

/// The arguments `ssh` is started with for a Vault at `path` on `host`.
///
/// Every one of them is here rather than written at the call site because what is an argument and
/// what is a word of a command is the whole of what keeps a path from being read as anything but
/// a path.
fn ssh_arguments(user: &str, host: &str, port: Option<u16>, path: &str) -> Vec<String> {
    let mut arguments = vec![
        // A terminal must never be allocated: what travels over the two streams is binary, and a
        // pseudo-terminal would translate the line endings of it.
        "-T".to_owned(),
        "-o".to_owned(),
        // Nothing may be asked of a person: the standard input is the protocol, so a program that
        // wanted a password or a passphrase would be waiting for what can never be sent. Failing
        // at once is what lets a caller be told why instead of being hung up on.
        "BatchMode=yes".to_owned(),
    ];

    // A port is written only when the address named one: what is left out is `ssh`'s own
    // configuration and its own default, which is the whole of what leaving it out means.
    if let Some(port) = port {
        arguments.push("-p".to_owned());
        arguments.push(port.to_string());
    }

    // The account travels as the value of an option rather than as part of a `user@host`, so it
    // is never read a second time by anything.
    arguments.push("-l".to_owned());
    arguments.push(user.to_owned());

    // Everything after this is the destination and the command, whatever it starts with: a host
    // may be written with a leading `-`, and so may nothing else that reaches `ssh` as an
    // option.
    arguments.push("--".to_owned());

    // The destination is the host as it was written. `ssh` takes an ipv6 address unwrapped, and
    // brackets it where a port would otherwise be read — which is not this position.
    arguments.push(host.to_owned());

    // The command is one argument: `ssh` joins what follows the destination with spaces and hands
    // the line to the far side's login shell, so quoting one argument here is the whole of what
    // keeps the path one word there.
    arguments.push(format!(
        "{REMOTE_PROGRAM} {REMOTE_COMMAND} {}",
        shell_quote(path)
    ));

    arguments
}

/// `word` written so that a POSIX shell hands it back as the one word it was.
///
/// Single quotes carry everything a shell would otherwise act on, including the `$`, the
/// backtick and the backslash. The one thing they cannot carry is a single quote, which is closed,
/// written escaped, and opened again — the standard reading, and the reason a path with a quote in
/// it still arrives as itself.
fn shell_quote(word: &str) -> String {
    let mut quoted = String::with_capacity(word.len() + 2);
    quoted.push('\'');

    for symbol in word.chars() {
        if symbol == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(symbol);
        }
    }

    quoted.push('\'');

    quoted
}

/// What an SSH program ended with, once it has ended.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ended {
    /// The code it ended with, or `None` when a signal ended it.
    pub(crate) code: Option<i32>,
}

/// The ending of the program a session was started with.
///
/// It is watched rather than waited for: what waits is a task of its own, so a caller that never
/// asks is not held up by a program that is still running, and a caller that does ask is answered
/// whether the program ended before or after it asked.
pub(crate) struct ExitWatch(watch::Receiver<Option<Ended>>);

impl ExitWatch {
    /// What the program ended with, waiting a bounded time for it to end at all.
    pub(crate) async fn ended(&self) -> Option<Ended> {
        let mut exit = self.0.clone();

        match tokio::time::timeout(ENDED_WITHIN, exit.wait_for(Option::is_some)).await {
            Ok(Ok(ended)) => *ended,
            // A program that has not ended within the bound, and one whose watcher is gone, have
            // nothing to say about the failure — what the exchange said is reported instead.
            Ok(Err(_)) | Err(_) => None,
        }
    }
}

/// Keeps a program that ends before it reads from taking this one down with it.
///
/// The stream to a session is a pipe, and writing to a pipe nothing reads raises `SIGPIPE`. This
/// program asks the kernel to make that fatal, so that `rola | head` dies the way a filter is
/// expected to die rather than carrying on writing into nothing — but a session is the one place
/// where the reader ending first is a *failure to report*: an `ssh` that cannot reach its host is
/// exactly that, and dying on a signal would say nothing about it.
///
/// So the disposition is set aside for as long as one session lives and given back when it goes.
/// A run talks over one session at a time by construction — an action is one connection — which is
/// why one guard is enough and no counter is kept.
struct SigpipeGuard {
    /// The disposition this process had before, which is restored when the guard goes.
    #[cfg(unix)]
    previous: libc::sighandler_t,
}

impl SigpipeGuard {
    /// Sets `SIGPIPE` aside, and hands back what set it aside.
    #[cfg(unix)]
    fn set_aside() -> Self {
        // SAFETY: `signal` replaces one disposition and returns the one it replaced. It touches no
        // memory of this program's and is safe to call from any thread; what is done with the
        // returned value is put back when the guard goes.
        let previous = unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };

        Self { previous }
    }

    /// Nothing to set aside: a write to a pipe whose reader has gone is an error on Windows, which
    /// is what is wanted there and here alike.
    #[cfg(not(unix))]
    const fn set_aside() -> Self {
        Self {}
    }
}

#[cfg(unix)]
impl Drop for SigpipeGuard {
    fn drop(&mut self) {
        // SAFETY: as in `set_aside`; what is put back is the value this process had before.
        unsafe {
            libc::signal(libc::SIGPIPE, self.previous);
        }
    }
}

/// Waits for the program a session runs over, and reports what it ended with.
///
/// The program is this task's from the moment it starts, which is what makes its exit code known
/// even when the session failed before it read a byte: nothing else can wait for a process, and a
/// caller that is told only "broken pipe" is not told what happened at the other end.
///
/// It ends when the program ends, or when the stream that spoke to it is dropped — a session that
/// went away leaves nothing here holding a program open on both sides.
async fn watch(mut child: Child, stopped: Stopped, ended: watch::Sender<Option<Ended>>) {
    let status = tokio::select! {
        status = child.wait() => status,
        // The other end went away: what is left is asking the program to stop, and waiting for it
        // so that what it ends with is still reported rather than left to the reaper.
        _ = stopped => {
            let _ = child.start_kill();
            child.wait().await
        }
    };

    // A wait that could not be made is a program whose ending is unknown, which is said the same
    // way a signal is: nothing about it is known but that it is over.
    let code = status.ok().and_then(|status| status.code());

    let _ = ended.send(Some(Ended { code }));
}

/// The end of a session asking the program it runs over to stop.
type Stopped = tokio::sync::oneshot::Receiver<()>;

/// The program a session runs over, as a stream the exchange is spoken through.
struct SshSocket {
    /// What is written to it.
    stdin: ChildStdin,
    /// What is read from it.
    stdout: ChildStdout,
    /// What asks the program to stop, taken out when this goes.
    kill: Option<tokio::sync::oneshot::Sender<()>>,
    /// What keeps the pipe's end from taking this process down, for as long as the session lives.
    ///
    /// It is held rather than read: what it does, it does when it is dropped. The name is what the
    /// field is for rather than what it is read as, hence the underscore.
    _signal: SigpipeGuard,
}

impl AsyncRead for SshSocket {
    /// Reads what the program wrote.
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stdout).poll_read(context, buffer)
    }
}

impl AsyncWrite for SshSocket {
    /// Writes to what the program reads.
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().stdin).poll_write(context, buffer)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stdin).poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stdin).poll_shutdown(context)
    }
}

impl Drop for SshSocket {
    /// The program goes with the stream.
    ///
    /// A session that ended on this side — a run that was cancelled, or one that failed before the
    /// far side said anything — would otherwise leave a program on this machine holding a
    /// connection open on both. It is asked to stop rather than waited for, since dropping may not
    /// wait: what waits is the task that was watching it all along.
    fn drop(&mut self) {
        if let Some(kill) = self.kill.take() {
            let _ = kill.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{shell_quote, ssh_arguments};

    #[test]
    fn a_path_is_quoted_so_that_a_shell_reads_it_as_one_word() {
        assert_eq!(shell_quote("/srv/vaults/alpha"), "'/srv/vaults/alpha'");
        assert_eq!(shell_quote("/srv/a vault"), "'/srv/a vault'");

        // The one thing single quotes cannot carry: closed, written escaped, and opened again.
        assert_eq!(
            shell_quote("/srv/a vault's path"),
            "'/srv/a vault'\\''s path'"
        );

        // Everything a shell would otherwise act on travels as it was written.
        assert_eq!(shell_quote("/srv/$(whoami)"), "'/srv/$(whoami)'");
        assert_eq!(shell_quote("/srv/a;b&c|d>e<f*g"), "'/srv/a;b&c|d>e<f*g'");
        assert_eq!(shell_quote("/srv/a\"b'c"), "'/srv/a\"b'\\''c'");
    }

    #[cfg(unix)]
    #[test]
    fn a_quoted_path_comes_back_from_a_shell_as_the_word_it_was() {
        // What is checked is what a shell makes of the quoting, rather than what the quoting
        // looks like: the far side of a session is a login shell, and it is the thing that has
        // to read a path with a space or a quote in it as one path.
        for word in [
            "/srv/vaults/alpha",
            "/srv/a vault's path",
            "/srv/$(whoami)",
            "/srv/a;b",
            "/srv/a*b",
        ] {
            let line = format!("printf %s {}", shell_quote(word));
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&line)
                .output()
                .unwrap();

            assert_eq!(String::from_utf8_lossy(&output.stdout), word);
        }
    }

    #[test]
    fn what_ssh_is_started_with_is_an_option_list_and_one_command() {
        let arguments = ssh_arguments("alice", "vault.example.com", None, "/srv/vaults/alpha");

        assert_eq!(
            arguments,
            [
                "-T",
                "-o",
                "BatchMode=yes",
                "-l",
                "alice",
                "--",
                "vault.example.com",
                "rola-daemon serve '/srv/vaults/alpha'",
            ]
        );

        // A port that was written is one, and one that was not is nothing: what is left out is
        // what the machine's own SSH configuration says, rather than a number guessed at here.
        let ported = ssh_arguments("alice", "host", Some(2222), "/srv/alpha");
        assert_eq!(
            ported,
            [
                "-T",
                "-o",
                "BatchMode=yes",
                "-p",
                "2222",
                "-l",
                "alice",
                "--",
                "host",
                "rola-daemon serve '/srv/alpha'",
            ]
        );
    }

    #[test]
    fn a_host_that_starts_with_a_dash_still_reads_as_the_destination() {
        // The separator is the whole of what keeps these two apart: everything before it is an
        // option, and everything after it is where to go.
        let arguments = ssh_arguments("alice", "-oProxyCommand=evil", None, "/srv/alpha");

        let dashes = arguments
            .iter()
            .position(|argument| argument == "--")
            .expect("the separator");
        assert_eq!(arguments[dashes + 1], "-oProxyCommand=evil");
    }
}
