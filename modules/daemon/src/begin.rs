//! The machine's daemon: it listens, proves who it is, and runs the action a Workspace asks for
//! against one of the Vaults it serves.
//!
//! A Vault does not reach out; it waits. Every connection is answered on its own, so one
//! Workspace that fails to prove itself, or asks for something the machine does not serve,
//! never holds up the ones behind it. What a connection needs that does not change — the
//! daemon's identity and the actions it serves — is resolved once, as the daemon starts, and
//! shared from there.
//!
//! # Two ways to be reached
//!
//! A daemon listens on a port, and a connection to it is a stream like any other. It may also be
//! reached over a session whose standard streams *are* the connection — what `rola-daemon serve
//! <path>` is — which needs no port at all and needs nothing of the network beyond `ssh` itself.
//! The two share everything an ear and a mouth share: the identity, the actions, the order in
//! which a caller is proved and admitted. What they do not share is where the Vault comes from.
//!
//! A listening daemon is told which Vault a request names, and looks that name up in the table of
//! namespaces. A session was given a path on its command line, and serves that Vault and no other:
//! the request it reads may not name one, since the address that started it already did. Nothing
//! that arrives over the connection ever becomes a path in either case — a namespace is looked up
//! in a table, and a path was decided before the connection existed.
//!
//! # Identity
//!
//! The daemon proves itself before it reads which Vault a caller wants, so there is one identity
//! for the machine rather than one per Vault: the account the user keeps in their own key
//! directory. A Vault's members are still checked against that Vault's own configuration, which is
//! what decides who may act on it — and, for a session, what may be written into its keys when a
//! name is there for the first time.

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use std::time::SystemTime;

use rorolala_auth::{
    KeyLocateRule, Member, PublicKey, SecureStream, SigningKey, env_keys_dir, find_member,
    global_keys_dir, is_valid_key_name, locate_accounts, user_keys_dir,
};
use rorolala_protocol::{ActionContext, ActionError, Socket};
use rorolala_utils_cli_theme::{err_line, help_line, warn_line};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_constants::{PUBLIC_KEY_EXTENSION, ROOT_SUB_VAULT};
use rorolala_utils_location::Locate;
use rorolala_vault::{
    CONFIG_PATH, KEYS_DIR, KeyDiscovery, RootVault, VAULT_DEFAULT_MAX_CONCURRENCY, VAULTS_DIR,
    Vault,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::{
    ActionEntry, CancelSignal, DaemonExit, Namespaces, build_action_registry, do_action_with, exit,
    preference::Preference, preference_path, wire,
};

/// Input provided to the daemon for its operation.
pub(crate) struct DaemonInput<'a> {
    /// The file naming the Vaults this daemon serves, by the namespace each is reached under.
    pub(crate) registry: &'a Path,
    /// The port to listen on.
    pub(crate) port: u16,
    /// The most transfers the daemon carries at once, when the run named one: `None` keeps what a
    /// machine-wide daemon serves with.
    pub(crate) max_concurrency: Option<u16>,
    /// The signal used to cancel the daemon.
    pub(crate) signal: CancelSignal,
}

/// Runs the daemon until it is cancelled.
///
/// What does not change while it runs is resolved once here: the identity the daemon proves
/// itself with, and the actions it serves. A daemon that cannot resolve those cannot serve at
/// all, so it says so and stops rather than listening and refusing every connection. Which Vault
/// a namespace names is read from the table per connection.
///
/// # Standard Error
///
/// Writes an error log when there is no identity, the table cannot be read, or the address
/// cannot be bound.
pub(crate) async fn daemon(input: DaemonInput<'_>) -> DaemonExit {
    let DaemonInput {
        registry,
        port,
        max_concurrency,
        signal,
    } = input;

    let Some(host) = host(max_concurrency) else {
        return DaemonExit::with_code(exit::NO_IDENTITY);
    };

    // Read once here so a table that will not parse stops the daemon at the start rather than
    // on the first connection. What it holds is read again per connection, since a namespace
    // added while the daemon runs is meant to be served without a restart.
    if let Err(error) = Namespaces::read_from(registry) {
        eprintln!(
            "{}",
            err_line!(
                "The namespace table at {} could not be read: {error}",
                (registry.display())
            )
        );
        return DaemonExit::with_code(exit::REGISTRY_UNREADABLE);
    }

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "{}",
                err_line!("The daemon could not listen on {address}: {error}")
            );
            return DaemonExit::with_code(exit::PORT_TAKEN);
        }
    };

    let registry = Arc::new(Registry {
        path: registry.to_path_buf(),
        // Nothing has been asked for yet, so nothing has been resolved.
        served: StdMutex::new(HashMap::new()),
    });

    listen(listener, Arc::new(host), registry, signal.rx).await;

    DaemonExit::default()
}

/// Everything a connection needs that does not change with the Vault it is served for.
///
/// The identity it proves and the actions it serves are fixed for as long as it runs, and what a
/// Vault is served under is fixed for as long as one connection is.
pub(crate) struct Host {
    /// The identity the daemon proves to every peer.
    signing: SigningKey,
    /// The run's own bound on how much is carried at once, applied over every Vault's file.
    max_concurrency: Option<u16>,
    /// Every action this daemon serves, laid out by id.
    actions: Vec<Option<Box<dyn ActionEntry>>>,
}

/// The Vaults a listening daemon serves, and what a name was resolved to last time.
///
/// A session has no table: it was given one path and serves it. So the table and its cache live
/// here rather than beside the identity, since what is read from them changes with every
/// connection and what is read from the identity does not.
struct Registry {
    /// The file naming the Vaults this daemon serves.
    path: PathBuf,
    /// The Vaults requests have named, as they were resolved, so a table and two configurations are
    /// not read again for every connection. A resolution whose sources have changed is made again
    /// rather than served from here.
    served: StdMutex<HashMap<String, KeptServed>>,
}

/// How a connection is served: where its Vault comes from, and what may be done for it.
enum Resolution {
    /// A connection to a listening daemon: the request names a namespace, and the table says
    /// which Vault that is. Nothing is registered for one — a port is not a door that proves a
    /// person, so it is not one that may name itself a member.
    Namespaces(Arc<Registry>),
    /// A connection over a session: the command named the Vault, and the machine's preferences
    /// say what may be done for a caller that arrived through a shell.
    Session {
        /// The Vault the command named.
        served: Arc<Served>,
        /// What this machine allows a session to do.
        preference: Arc<Preference>,
    },
}

/// Builds what does not change with a connection, or says why it cannot.
///
/// # Standard Error
///
/// Writes an error log when no place holds an account, or its key cannot be read.
fn host(max_concurrency: Option<u16>) -> Option<Host> {
    Some(Host {
        signing: daemon_identity()?,
        max_concurrency,
        // Built once: the list a caller's id is read against is the one this daemon runs.
        actions: build_action_registry(),
    })
}

/// Serves one connection over the standard streams of this process, until it ends.
///
/// What the far side of a session asked for is answered by a Vault named on the command line
/// rather than by the table of namespaces, so the path is read once here and the connection is
/// served for it alone. The process ends when its standard input does: a session serves one
/// connection and no more, which is why there is nothing to wait for afterwards.
///
/// # Standard Error
///
/// Writes an error log when the machine's preferences cannot be read or refuse SSH sessions, when
/// the path does not hold a Vault, and when the session fails.
pub(crate) async fn serve_vault(path: &Path, max_concurrency: Option<u16>) -> DaemonExit {
    let preference = match Preference::read() {
        Ok(preference) => preference,
        Err(error) => {
            eprintln!("{}", err_line!("{error}"));
            return DaemonExit::with_code(exit::PREFERENCE_UNREADABLE);
        }
    };

    // What the switch is for is a machine that would rather not serve a session at all, and a
    // refusal is said where whoever started the session can read it: this is the far side's
    // standard error, which `ssh` forwards to the person who asked.
    if !preference.security().ssh_connection() && in_ssh_session() {
        eprintln!(
            "{}",
            err_line!("This machine does not allow its daemon to be reached over SSH.")
        );
        // Where the switch is, said as a place to go: a refusal a caller cannot act on is one
        // that reads as a fault in this program.
        let where_to_set = preference_path().map_or_else(
            || "this machine's preference file".to_owned(),
            |path| path.display().to_string(),
        );
        eprintln!(
            "{}",
            help_line!("Set `ssh-connection = true` under `[security]` in {where_to_set}")
        );
        return DaemonExit::with_code(exit::SSH_DISABLED);
    }

    let Some(host) = host(max_concurrency) else {
        return DaemonExit::with_code(exit::NO_IDENTITY);
    };

    let Some(served) = Served::from_path(path, max_concurrency) else {
        let named = path.display();
        eprintln!(
            "{}",
            err_line!("{named} holds no Vault to serve: no {CONFIG_PATH} is there")
        );
        return DaemonExit::with_code(exit::NO_VAULT);
    };

    let resolution = Resolution::Session {
        served: Arc::new(served),
        preference: Arc::new(preference),
    };

    match serve(Stdio::new(), Arc::new(host), resolution).await {
        Ok(()) => DaemonExit::default(),
        Err(error) => {
            eprintln!("{}", err_line!("The session failed: {error}"));
            DaemonExit::with_code(exit::SESSION_FAILED)
        }
    }
}

/// Whether this process was started by an SSH session.
///
/// It asks the environment, which is a policy switch and not a security boundary: a program
/// started on this machine can set the same words. What actually decides whether a session may be
/// served is who may run this program as this user at all, and what is being switched here is
/// whether this machine wants to offer that at all.
fn in_ssh_session() -> bool {
    ["SSH_CONNECTION", "SSH_USER_AUTH"]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

/// The two streams this process was handed, as one connection.
///
/// A session is the same session the listening daemon serves a socket over — the two differ in
/// where the stream comes from and in nothing else, which is why there is one `serve` and not a
/// second implementation of the exchange for a shell.
struct Stdio {
    /// What a request is read from.
    input: tokio::io::Stdin,
    /// What an answer is written to.
    output: tokio::io::Stdout,
}

impl Stdio {
    /// This process's standard streams.
    fn new() -> Self {
        Self {
            input: tokio::io::stdin(),
            output: tokio::io::stdout(),
        }
    }
}

impl AsyncRead for Stdio {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().input).poll_read(context, buffer)
    }
}

impl AsyncWrite for Stdio {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.get_mut().output).poll_write(context, buffer)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().output).poll_flush(context)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().output).poll_shutdown(context)
    }
}

/// The directories a member is looked for in, and the rule they are searched under.
///
/// The Vault's configuration names the places — see [`KeyDiscovery`] — and each one is a
/// directory handed to the search, so the rule is the one that searches what it is handed and
/// nothing else: a scope turned on in the rule as well would name its directory a second time.
/// A place with no directory yet is still named, since a scope that is not set up is not an
/// error.
fn member_scopes(
    config: &rorolala_vault::Config,
    vault: &Vault,
    root_vault: &RootVault,
) -> (Vec<PathBuf>, KeyLocateRule) {
    let mut roots = Vec::new();

    for place in config.daemon_config().key_discovery() {
        match place {
            KeyDiscovery::System => roots.extend(global_keys_dir()),
            KeyDiscovery::User => roots.extend(user_keys_dir()),
            KeyDiscovery::Env => roots.extend(env_keys_dir()),
            KeyDiscovery::Vault => roots.push(vault.get_root().join(KEYS_DIR)),
            KeyDiscovery::RootVault => roots.push(root_vault.get_root().join(KEYS_DIR)),
        }
    }

    (roots, local_rule())
}

/// A rule that searches the directories it is handed and no scope of its own.
const fn local_rule() -> KeyLocateRule {
    KeyLocateRule {
        find_global: false,
        find_local: true,
        find_user: false,
        find_env: false,
    }
}

/// The account the daemon proves itself with: the user's own, looked for where the user keeps
/// keys.
///
/// A machine-wide daemon speaks before it knows which Vault a caller wants, so it cannot prove
/// itself with a Vault's key. What it proves instead is that it runs as this user.
///
/// # Standard Error
///
/// Writes an error log when no place holds an account, or its key cannot be read.
fn daemon_identity() -> Option<SigningKey> {
    let mut roots = Vec::new();
    roots.extend(user_keys_dir());
    roots.extend(env_keys_dir());

    identity_in(&roots)
}

/// The first account `roots` holds, by the rule the daemon always searches under.
///
/// # Standard Error
///
/// Writes an error log when no place holds an account, or its key cannot be read.
fn identity_in(roots: &[PathBuf]) -> Option<SigningKey> {
    let accounts = locate_accounts(roots, &local_rule());

    let Some(account) = accounts.iter().next() else {
        let places = roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        eprintln!(
            "{}",
            err_line!(
                "The daemon holds no account to prove itself with, in the user's keys at {places}."
            )
        );
        eprintln!(
            "{}",
            help_line!(
                "Please give the user an account — `rola key generate <name> --install` writes one into the user's key directory"
            )
        );
        return None;
    };

    match account.get_key() {
        Ok(identity) => Some(identity),
        Err(error) => {
            eprintln!(
                "{}",
                err_line!("The daemon's own key could not be read: {error}")
            );
            None
        }
    }
}

/// A Vault a request named, and what says the resolution still holds.
struct KeptServed {
    /// The files the resolution was read from, as they looked when it read them.
    sources: [Source; 3],
    /// The Vault, and what it is served with.
    served: Arc<Served>,
}

/// One file a resolution was read from, and how it looked when it was.
///
/// It is the length and the modification time rather than either alone: a configuration rewritten to
/// say something else is a file whose time moved, and the length agreeing with it is one more thing
/// that has to match for two readings to be the same reading.
struct Source {
    /// Where the file is.
    path: PathBuf,
    /// How long it was.
    len: u64,
    /// When it was last written, where the filesystem tells it.
    modified: Option<SystemTime>,
}

impl Source {
    /// How the file at `path` looks now, which is how it looked when it is not there at all.
    fn of(path: impl Into<PathBuf>) -> Self {
        let path = path.into();

        let (len, modified) = std::fs::metadata(&path).map_or((0, None), |metadata| {
            (metadata.len(), metadata.modified().ok())
        });

        Self {
            path,
            len,
            modified,
        }
    }

    /// Whether the file is still the file the resolution read.
    fn holds(&self) -> bool {
        let now = Self::of(self.path.clone());

        now.len == self.len && now.modified == self.modified
    }
}

/// Serves connections until `cancel` fires.
///
/// Each connection is a job of its own, so a slow or hostile peer holds up nothing but
/// itself. The loop itself does no work beyond accepting, which is what lets cancellation
/// take effect between two connections rather than after the current one.
async fn listen(
    listener: TcpListener,
    host: Arc<Host>,
    registry: Arc<Registry>,
    mut cancel: watch::Receiver<bool>,
) {
    // How many connections are served at once. A client asks for a number of lanes and each lane
    // is a connection, so this is what holds every caller together to what the machine is set to
    // serve: one run cannot spend the machine, and neither can many.
    let serving = Arc::new(tokio::sync::Semaphore::new(
        usize::from(
            host.max_concurrency
                .unwrap_or(VAULT_DEFAULT_MAX_CONCURRENCY),
        )
        .max(1),
    ));

    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    // A request and its answer are small and alternate, and a sealed record is
                    // written as its own buffer: left to Nagle, the second record of an exchange
                    // waits for the first to be acknowledged, which is a round trip a small action
                    // spends on nothing. Everything this connection carries is that shape, so the
                    // delay is turned off for the whole of it.
                    if let Err(error) = stream.set_nodelay(true) {
                        eprintln!(
                            "{}",
                            warn_line!("A connection from {peer} could not be tuned: {error}")
                        );
                    }

                    // The permit is taken before the connection is spawned, so a daemon serving
                    // its limit holds the next peer at the accept rather than admitting it to
                    // wait: what a caller meets is backpressure, not a connection that does
                    // nothing.
                    let Ok(permit) = Arc::clone(&serving).acquire_owned().await else {
                        break;
                    };
                    let host = Arc::clone(&host);
                    let registry = Arc::clone(&registry);
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(error) =
                            serve(stream, host, Resolution::Namespaces(registry)).await
                        {
                            eprintln!(
                                "{}",
                                err_line!("A connection from {peer} failed: {error}")
                            );
                        }
                    });
                }
                // One connection that cannot be accepted is not a reason to stop: the
                // daemon is still listening, and the next peer may fare better.
                Err(error) => {
                    eprintln!(
                        "{}",
                        warn_line!("A connection could not be accepted: {error}")
                    );
                }
            },
            // The signal fires once. Whether it says so or the sender is gone, the
            // daemon has nothing left to wait for.
            _ = cancel.changed() => break,
        }
    }
}

/// Serves one connection: handshakes, checks the caller, and runs the action.
///
/// The order is what makes a name mean anything. The handshake proves a key; which Vault the
/// connection is served for is read after it, since that is what the connection is for; and the
/// name the Workspace sends is only a label, so the member that label finds must be the identity
/// that was proved before the action runs. A request that is not confirmed never starts an action,
/// so a Workspace that is refused finds out before it has anything to undo.
async fn serve<Stream>(
    stream: Stream,
    host: Arc<Host>,
    resolution: Resolution,
) -> Result<(), SessionError>
where
    Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let socket: Box<dyn Socket> = Box::new(stream);
    let mut channel = SecureStream::accept(socket, &host.signing).await?;

    let request = wire::read_request(&mut channel)
        .await
        .map_err(SessionError::Exchange)?;

    // Which Vault the connection is served for decides who is admitted to it, so it is resolved
    // before the caller is looked for: the same name is a member of one Vault and of no other. A
    // session serves the Vault it was started for and no other, so a request that names one is a
    // request it cannot answer.
    let served = match &resolution {
        Resolution::Namespaces(registry) => registry.serve_for(&host, &request.sub).await,
        Resolution::Session { served, .. } => {
            (request.sub == ROOT_SUB_VAULT).then(|| Arc::clone(served))
        }
    };

    let Some(served) = served else {
        return Err(SessionError::UnknownVault(request.sub));
    };

    // The name is joined onto a scope's directory to reach the member's key, so what could steer
    // that join is refused here as well as in the search: nothing off the wire ever reaches a
    // path, and a name that is not one is never a key file's name.
    if !is_valid_key_name(&request.account) {
        return Err(SessionError::UnknownMember(request.account));
    }

    let member = member_for(&served, &request.account, channel.peer(), &resolution)?;

    if member.get_key().map_err(SessionError::Key)? != *channel.peer() {
        return Err(SessionError::IdentityMismatch(request.account));
    }

    // Both sides now know they mean the same action, so both start it.
    wire::write_confirmation(&mut channel, request.id)
        .await
        .map_err(SessionError::Exchange)?;

    // The Vault being served and its root live for this connection, so the action reaches them,
    // and what either is configured with, by borrowing them for as long as the action runs. The
    // resolution is held here rather than borrowed from the daemon's memory, so what a connection is
    // served with is one whole even if the next request resolves a Vault and replaces it.
    let ctx = ActionContext::new_vault_ctx(member)
        .with_current_vault(&served.vault)
        .with_current_root_vault(&served.root_vault)
        .with_current_vault_config(&served.config)
        .with_current_root_vault_config(&served.root_vault_config)
        .with_channel(channel);
    do_action_with(&host.actions, request.id, ctx)
        .await
        .map_err(SessionError::Action)?;

    Ok(())
}

/// The member `name` is served as, registering the key the peer proved where a session may.
///
/// A name that is known is answered from what the Vault admits, and nothing is written. A name
/// that is not is the first connection of somebody the Vault has never seen, and what may be done
/// about that is what the session resolution and the machine's preferences say: a listening daemon
/// registers nothing at all, since a port proves no person.
///
/// Registering is safe because of what the handshake already did: the key is one the far side
/// proved it holds, so a name cannot be claimed for a key nobody has. What it is *not* is
/// revocation-aware — a deleted key is written again on the next connection — which is why the
/// switch defaults to off and why the rules below are read before anything is written.
fn member_for(
    served: &Served,
    name: &str,
    peer: &PublicKey,
    resolution: &Resolution,
) -> Result<Member, SessionError> {
    if let Some(member) = find_member(name, &served.roots, &served.rule) {
        return Ok(member);
    }

    let Resolution::Session { preference, .. } = resolution else {
        return Err(SessionError::UnknownMember(name.to_owned()));
    };

    if !preference.security().auto_register_by_ssh() {
        return Err(SessionError::UnknownMember(name.to_owned()));
    }

    // An administrator's name may move any file in the Vault, so a name with no key yet is the
    // last one a connection may claim: whoever can reach the port would otherwise become one by
    // saying the word.
    if served.config.auth_config().is_admin(name) {
        return Err(SessionError::RegisterAdmin(name.to_owned()));
    }

    // A key written where nothing looks for it is not a registration, so saying one was made
    // would be telling a caller something it can act on and should not.
    if !served
        .config
        .daemon_config()
        .key_discovery()
        .contains(&KeyDiscovery::Vault)
    {
        return Err(SessionError::RegisterNotDiscovered(name.to_owned()));
    }

    register_member_key(&served.vault.get_root().join(KEYS_DIR), name, peer)?;

    find_member(name, &served.roots, &served.rule)
        .ok_or_else(|| SessionError::UnknownMember(name.to_owned()))
}

/// How many registrations this process has staged, which is what tells one staging file from the next.
///
/// A daemon serves one connection, but a test or an embedder may serve several at once through the
/// same process, and two of them writing one scratch name would race over it.
static STAGED: AtomicUsize = AtomicUsize::new(0);

/// Writes `key` as the member `name` of the Vault whose keys are at `keys`.
///
/// What is written is never a file that is already there: a name registered to one key is not
/// registered again to another, and a connection that arrives while another is registering the
/// same name either finds the same key already written or is refused. That is what makes the
/// first-come rule the only rule — nothing here overwrites.
fn register_member_key(keys: &Path, name: &str, key: &PublicKey) -> Result<(), SessionError> {
    let target = keys.join(format!("{name}.{PUBLIC_KEY_EXTENSION}"));

    std::fs::create_dir_all(keys).map_err(|error| SessionError::register(&target, &error))?;

    let pem = key.to_pem().map_err(|error| SessionError::Register {
        path: target.clone(),
        reason: error.to_string(),
    })?;

    // What is written is written beside the name and then given to it, so that the name is either
    // absent or whole: a reader that arrives mid-write sees no file rather than half of one. The
    // number is what keeps two registrations in one process — a daemon serves one connection, but a
    // test or an embedder may serve several — from writing the same scratch name.
    let staged = keys.join(format!(
        ".{name}.{PUBLIC_KEY_EXTENSION}.{}.{}.tmp",
        std::process::id(),
        STAGED.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&staged, pem.as_bytes())
        .map_err(|error| SessionError::register(&target, &error))?;

    let made = match std::fs::hard_link(&staged, &target) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        // A filesystem that cannot link two names to one file — FAT, and some network shares — is
        // one that cannot make the name atomically either, so the fallback makes the name
        // exclusively instead. It is the closest thing such a filesystem has, and its one known
        // gap is recorded: what a concurrent reader could see in between is a name that exists and
        // is still being written.
        Err(_) => match exclusive_write(&target, pem.as_bytes()) {
            Ok(made) => made,
            Err(error) => {
                let _ = std::fs::remove_file(&staged);
                return Err(SessionError::register(&target, &error));
            }
        },
    };

    let _ = std::fs::remove_file(&staged);

    // What the name holds now is what decides: a key that is the same is a registration another
    // connection made a moment before this one, and a key that is not is a name that is taken.
    let held = std::fs::read_to_string(&target)
        .map_err(|error| SessionError::register(&target, &error))
        .and_then(|pem| {
            PublicKey::from_pem(&pem).map_err(|error| SessionError::Register {
                path: target.clone(),
                reason: error.to_string(),
            })
        })?;

    if held != *key {
        return Err(SessionError::IdentityMismatch(name.to_owned()));
    }

    if made {
        announce(name, key);
    }

    Ok(())
}

/// Writes `bytes` to `target` only if nothing is there, answering whether it did.
fn exclusive_write(target: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    use std::io::Write as _;

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
    {
        Ok(mut file) => {
            file.write_all(bytes)?;
            file.flush()?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

/// Says, on the standard error the caller's session forwards, that a member was registered.
///
/// It goes where a person running the command can read it, because a registration is something
/// that happened to their Vault and nothing else they run will say so.
fn announce(name: &str, key: &PublicKey) {
    let session = std::env::var("SSH_CONNECTION").map_or_else(
        |_| "a local session".to_owned(),
        |connection| format!("an SSH session from {connection}"),
    );

    let fingerprint = key.fingerprint();

    // Plain rather than through one of the theme's lines: this is a record of something that
    // happened, neither an error nor advice, and it is read in a log as much as on a terminal.
    eprintln!(
        "Registered `{name}` as a member of this Vault, by the key it proved ({fingerprint}), over {session}"
    );
}

/// The Vault one connection is served for, with what it is served with.
///
/// A request names a Vault, and what that comes to is this: the Vault itself, what its file is
/// configured with, the root above it, and where its members are looked for. It is kept by the daemon
/// beside the files it was read from — see [`KeptServed`] — and held by a connection for as long as the
/// action it serves runs, so what that action is served with is one whole however many requests arrive
/// after it.
struct Served {
    /// The Vault the action is run over.
    vault: Vault,
    /// The outermost Vault above it, as a context hands it to an action.
    root_vault: RootVault,
    /// What the Vault is configured with, as a context hands it to an action.
    config: rorolala_vault::Config,
    /// What the root above it is configured with, as a context hands it to an action.
    root_vault_config: rorolala_vault::Config,
    /// The directories a member of it may be found in, nearest first.
    roots: Vec<PathBuf>,
    /// The scopes a member may be found in, which is the directories above and no other.
    rule: KeyLocateRule,
}

impl Served {
    /// The Vault at `path`, if a Vault is what is there.
    ///
    /// The path names the root of a Vault exactly, the way registering one does: a directory that
    /// merely lies inside a Vault is not one, so what is served is what was named rather than what
    /// was nearest. Nothing here reads the table of namespaces — a session is told where its Vault
    /// is — which is what lets a machine with nothing registered serve one.
    fn from_path(path: &Path, max_concurrency: Option<u16>) -> Option<Self> {
        let absolute = std::path::absolute(path).ok()?;

        if !absolute.join(CONFIG_PATH).is_file() {
            return None;
        }

        Some(Self::of(Vault::at(absolute), max_concurrency))
    }

    /// `vault`, read as a connection is served it.
    ///
    /// What the Vault is configured with, and who it admits, are read here rather than once at
    /// startup: a Vault reconfigured while the daemon runs is served under the rules it has, and
    /// a Vault under a root that keeps the group's keys admits them without either having to be
    /// restarted.
    fn of(vault: Vault, max_concurrency: Option<u16>) -> Self {
        let root_vault =
            RootVault::locate(vault.get_root()).unwrap_or_else(|| RootVault::from(vault.clone()));

        let config = read_config(&vault.config_path());
        let root_vault_config = read_config(&root_vault.config_path());
        let (roots, rule) = member_scopes(&config, &vault, &root_vault);
        let config = bound(&config, max_concurrency);

        Self {
            vault,
            root_vault,
            config,
            root_vault_config,
            roots,
            rule,
        }
    }
}

/// The configuration at `path`, or what a Vault whose file will not read is served under.
///
/// # Standard Error
///
/// Writes a warning naming the file, since a Vault served under defaults is one whose callers
/// meet rules nobody configured.
fn read_config(path: &Path) -> rorolala_vault::Config {
    match rorolala_vault::Config::read_from(path) {
        Ok(read) => read,
        Err(error) => {
            eprintln!(
                "{}",
                warn_line!(
                    "The configuration of the Vault at {} could not be read: {error}",
                    (path.display())
                )
            );
            rorolala_vault::Config::default()
        }
    }
}

impl Registry {
    /// The Vault a request naming `sub` is served for, if this daemon can serve one there.
    ///
    /// What it comes to is kept, so the next connection naming the same Vault reads a table, two
    /// configurations and a walk to the root only when one of them has changed since.
    async fn serve_for(&self, host: &Host, sub: &str) -> Option<Arc<Served>> {
        if let Some(kept) = self.kept(sub) {
            return Some(kept);
        }

        let served = Arc::new(self.resolve(host, sub).await?);
        self.keep(sub, Arc::clone(&served));

        Some(served)
    }

    /// The Vault `sub` names, as it was resolved, when nothing it was resolved from has changed.
    fn kept(&self, sub: &str) -> Option<Arc<Served>> {
        let kept = self
            .served
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(sub)
            .map(|kept| {
                (
                    kept.sources.each_ref().map(Source::holds),
                    Arc::clone(&kept.served),
                )
            })?;

        let (holds, served) = kept;

        holds.iter().all(|holds| *holds).then_some(served)
    }

    /// Keeps how `sub` was resolved, with what it was resolved from.
    fn keep(&self, sub: &str, served: Arc<Served>) {
        let sources = [
            Source::of(&self.path),
            Source::of(served.vault.config_path()),
            Source::of(served.root_vault.config_path()),
        ];

        let mut kept = self.served.lock().unwrap_or_else(PoisonError::into_inner);

        // A peer may name a Vault it has made up every time it connects, so what is kept is bounded:
        // the daemon serves the namespaces it was given, and a cache that grows with a stranger's
        // requests is one a stranger can spend the machine with.
        if kept.len() >= KEPT_SERVED && !kept.contains_key(sub) {
            kept.clear();
        }

        kept.insert(sub.to_owned(), KeptServed { sources, served });
    }

    /// Resolves the Vault a request naming `sub` is served for, reading everything it comes to.
    ///
    /// `sub` is a namespace, or a namespace and the Vault below it: the first segment picks the
    /// entry out of the namespace table, and what follows is resolved under the Vault that entry
    /// names — by the path it sits at or by the name it is held under. A namespace that is not
    /// registered, or a Vault that is no longer there, is a request this daemon cannot answer.
    async fn resolve(&self, host: &Host, sub: &str) -> Option<Served> {
        let (namespace, under) = split_namespace(sub);

        if namespace.is_empty() {
            return None;
        }

        let namespaces = match Namespaces::read_from(&self.path) {
            Ok(namespaces) => namespaces,
            Err(error) => {
                eprintln!(
                    "{}",
                    warn_line!("The namespace table could not be read: {error}")
                );
                return None;
            }
        };

        let root = namespaces.get(namespace)?;

        if !root.join(CONFIG_PATH).is_file() {
            return None;
        }

        let entry = Vault::at(root);
        let vault = if under.is_empty() {
            entry
        } else {
            let held = RootVault::from(entry);
            match held.get_vault_by_path(under).await {
                Some(vault) => vault,
                None => {
                    held.get_vault_by_path(format!("{VAULTS_DIR}/{under}"))
                        .await?
                }
            }
        };

        Some(Served::of(vault, host.max_concurrency))
    }
}

/// How many resolved Vaults the daemon keeps.
///
/// A request names a Vault and a daemon is made to serve the namespaces it was given, so what is kept
/// is a handful. The bound is what keeps a peer that names a different Vault every time from growing
/// the daemon's memory for as long as it is left running.
const KEPT_SERVED: usize = 64;

/// The configuration a Vault is served under: what its file says, with the run's own bound on
/// how much is carried at once applied over it.
fn bound(config: &rorolala_vault::Config, max_concurrency: Option<u16>) -> rorolala_vault::Config {
    let mut bound = config.clone();

    if let Some(max_concurrency) = max_concurrency {
        bound
            .daemon_config_mut()
            .set_max_concurrency(max_concurrency);
    }

    bound
}

/// The namespace a request names, and the Vault below it when one is named.
///
/// The first segment is the namespace; everything after it is the path within that namespace.
/// Separators at either end are not part of a name, so `alpha/` and `/alpha` name the same
/// entry and `alpha/` names no Vault below it.
fn split_namespace(sub: &str) -> (&str, &str) {
    let sub = sub.trim_matches('/');

    match sub.split_once('/') {
        Some((namespace, under)) => (namespace, under.trim_matches('/')),
        None => (sub, ""),
    }
}

/// What went wrong while serving one connection.
///
/// A failure here is one peer's, so the daemon logs it and carries on: nothing in this
/// reaches a caller, which is why it is not an exported error.
#[derive(Debug)]
enum SessionError {
    /// The peer could not be handshaken with.
    Handshake(rorolala_auth::Error),
    /// The request could not be read, or the confirmation written.
    Exchange(ActionError),
    /// A member's public key could not be read from the file it names.
    Key(rorolala_auth::Error),
    /// No member this Vault knows is named as the caller.
    UnknownMember(String),
    /// The request names a namespace this daemon does not serve.
    UnknownVault(String),
    /// The member the caller named is not the identity it proved.
    IdentityMismatch(String),
    /// A registration could not be written.
    Register {
        /// The file that was to be written.
        path: PathBuf,
        /// What the filesystem said about it.
        reason: String,
    },
    /// A registration was refused because the Vault does not look for members in its own keys.
    RegisterNotDiscovered(String),
    /// A registration was refused because the name is an administrator of the Vault.
    RegisterAdmin(String),
    /// The action itself failed.
    Action(ActionError),
}

impl SessionError {
    /// A registration at `path` that failed for `source`.
    ///
    /// What is kept of the failure is its words, so the error is borrowed rather than taken.
    fn register(path: &Path, source: &std::io::Error) -> Self {
        Self::Register {
            path: path.to_path_buf(),
            reason: source.to_string(),
        }
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handshake(source) => write!(formatter, "the handshake failed: {source}"),
            Self::Exchange(source) => {
                write!(formatter, "the request could not be answered: {source}")
            }
            Self::Key(source) => write!(formatter, "a member's key could not be read: {source}"),
            Self::UnknownMember(name) => write!(
                formatter,
                "no member is named {name}: the Vault's keys do not hold a key for it. Ask whoever                  runs the Vault to put your public key in its keys, or allow `auto-register-by-ssh`                  in its machine's preference file"
            ),
            Self::UnknownVault(name) => {
                write!(formatter, "no Vault is served under {name}")
            }
            Self::IdentityMismatch(name) => {
                write!(formatter, "the peer is not the member it named ({name})")
            }
            Self::Register { path, reason } => write!(
                formatter,
                "the member's key could not be written to {}: {reason}",
                path.display()
            ),
            Self::RegisterNotDiscovered(name) => write!(
                formatter,
                "{name} is not a member and cannot be registered: the Vault does not look for                  members in its own keys (`key_discovery` has no `vault`)"
            ),
            Self::RegisterAdmin(name) => write!(
                formatter,
                "{name} is an administrator of the Vault and is not registered by a connection:                  an administrator has to be named by hand"
            ),
            Self::Action(source) => write!(formatter, "the action failed: {source}"),
        }
    }
}

impl From<rorolala_auth::Error> for SessionError {
    fn from(source: rorolala_auth::Error) -> Self {
        Self::Handshake(source)
    }
}

#[cfg(test)]
include!("begin_tests.rs");
