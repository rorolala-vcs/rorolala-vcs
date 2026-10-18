//! The machine's daemon: it listens, proves who it is, and runs the action a
//! Workspace asks for against one of the Vaults registered for this user.
//!
//! A Vault does not reach out; it waits. Every connection is answered on its own, so one
//! Workspace that fails to prove itself, or asks for something the machine does not serve,
//! never holds up the ones behind it. What a connection needs that does not change — the
//! daemon's identity and the actions it serves — is resolved once, as the daemon starts, and
//! shared from there.
//!
//! Which Vault a connection is *served as*, though, changes with every connection: an address
//! names a namespace, and the registry says which Vault that namespace is. The namespace is
//! read per connection rather than once, so a Vault registered while the daemon runs is served
//! without it being restarted.
//!
//! # Identity
//!
//! The listener proves itself before it reads which namespace a caller wants, so there is one
//! identity for the machine rather than one per Vault: the account the user keeps in their own
//! key directory. A registered Vault's members are still checked against that Vault's own
//! configuration, which is what decides who may act on it.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rorolala_auth::{
    KeyLocateRule, SecureStream, SigningKey, env_keys_dir, find_member, global_keys_dir,
    is_valid_key_name, locate_accounts, user_keys_dir,
};
use rorolala_protocol::{ActionContext, ActionError, Socket};
use rorolala_utils_cli_theme::{err_line, help_line, warn_line};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_location::Locate;
use rorolala_vault::{
    CONFIG_PATH, KEYS_DIR, KeyDiscovery, RootVault, VAULT_DEFAULT_MAX_CONCURRENCY, VAULTS_DIR,
    Vault,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::{
    ActionEntry, CancelSignal, DaemonExit, Namespaces, build_action_registry, do_action_with, exit,
    wire,
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
/// itself with, and the table of Vaults it serves. A daemon that cannot resolve those cannot
/// serve at all, so it says so and stops rather than listening and refusing every connection.
/// Which Vault a namespace names is read from the table per connection.
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

    let Some(signing) = daemon_identity() else {
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

    let host = Arc::new(Host {
        signing,
        registry: registry.to_path_buf(),
        max_concurrency,
        // Built once: the list a caller's id is read against is the one this daemon runs.
        actions: build_action_registry(),
    });

    listen(listener, host, signal.rx).await;

    DaemonExit::default()
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

/// Everything the daemon needs to answer a connection.
///
/// The identity it proves and the table it reads are fixed for as long as it runs; which Vault
/// a request names is resolved per connection rather than kept here.
struct Host {
    /// The identity the daemon proves to every peer.
    signing: SigningKey,
    /// The file naming the Vaults this daemon serves.
    registry: PathBuf,
    /// The run's own bound on how much is carried at once, applied over every Vault's file.
    max_concurrency: Option<u16>,
    /// Every action this daemon serves, laid out by id.
    actions: Vec<Option<Box<dyn ActionEntry>>>,
}

/// Serves connections until `cancel` fires.
///
/// Each connection is a job of its own, so a slow or hostile peer holds up nothing but
/// itself. The loop itself does no work beyond accepting, which is what lets cancellation
/// take effect between two connections rather than after the current one.
async fn listen(listener: TcpListener, host: Arc<Host>, mut cancel: watch::Receiver<bool>) {
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
                    // The permit is taken before the connection is spawned, so a daemon serving
                    // its limit holds the next peer at the accept rather than admitting it to
                    // wait: what a caller meets is backpressure, not a connection that does
                    // nothing.
                    let Ok(permit) = Arc::clone(&serving).acquire_owned().await else {
                        break;
                    };
                    let host = Arc::clone(&host);
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(error) = serve(stream, host).await {
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
/// request names is read after it, since that is what the connection is served for; and the
/// name the Workspace sends is only a label, so the member that label finds must be the
/// identity that was proved before the action runs. A request that is not confirmed never
/// starts an action, so a Workspace that is refused finds out before it has anything to undo.
async fn serve<Stream>(stream: Stream, host: Arc<Host>) -> Result<(), SessionError>
where
    Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let socket: Box<dyn Socket> = Box::new(stream);
    let mut channel = SecureStream::accept(socket, &host.signing).await?;

    let request = wire::read_request(&mut channel)
        .await
        .map_err(SessionError::Exchange)?;

    // Which Vault the request names decides who is admitted to it, so it is resolved before the
    // caller is looked for: the same name is a member of one Vault and of no other.
    let Some(served) = host.serve_for(&request.sub).await else {
        return Err(SessionError::UnknownVault(request.sub));
    };

    // The name is joined onto a scope's directory to reach the member's key, so what could steer
    // that join is refused here as well as in the search: nothing off the wire ever reaches a
    // path, and the search is never asked with a name that is not one.
    if !is_valid_key_name(&request.account) {
        return Err(SessionError::UnknownMember(request.account));
    }

    let Some(member) = find_member(&request.account, &served.roots, &served.rule) else {
        return Err(SessionError::UnknownMember(request.account));
    };
    if member.get_key().map_err(SessionError::Key)? != *channel.peer() {
        return Err(SessionError::IdentityMismatch(request.account));
    }

    // Both sides now know they mean the same action, so both start it.
    wire::write_confirmation(&mut channel, request.id)
        .await
        .map_err(SessionError::Exchange)?;

    // The Vault being served and its root live for this connection, so the action reaches them,
    // and what either is configured with, by borrowing them for as long as the action runs.
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

/// The Vault one connection is served for, with what it is served with.
///
/// A Vault is one thing for the duration of a connection and a different one for the next, so
/// what a request names is resolved into this: the Vault itself, what its file is configured
/// with, the root above it, and where its members are looked for.
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

impl Host {
    /// The Vault a request naming `sub` is served for, if this daemon can serve one there.
    ///
    /// `sub` is a namespace, or a namespace and the Vault below it: the first segment picks the
    /// entry out of the namespace table, and what follows is resolved under the Vault that entry
    /// names — by the path it sits at or by the name it is held under. A namespace that is not
    /// registered, or a Vault that is no longer there, is a request this daemon cannot answer.
    ///
    /// What the Vault is configured with, and who it admits, are read here rather than once at
    /// startup: a Vault reconfigured while the daemon runs is served under the rules it has, and
    /// a Vault under a root that keeps the group's keys admits them without either having to be
    /// restarted.
    async fn serve_for(&self, sub: &str) -> Option<Served> {
        let (namespace, under) = split_namespace(sub);

        if namespace.is_empty() {
            return None;
        }

        let namespaces = match Namespaces::read_from(&self.registry) {
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

        let root_vault =
            RootVault::locate(vault.get_root()).unwrap_or_else(|| RootVault::from(vault.clone()));

        let config = match rorolala_vault::Config::read_from(&vault.config_path()) {
            Ok(read) => read,
            Err(error) => {
                eprintln!(
                    "{}",
                    warn_line!(
                        "The configuration of the Vault at {} could not be read: {error}",
                        (vault.get_root().display())
                    )
                );
                rorolala_vault::Config::default()
            }
        };

        let root_vault_config = match rorolala_vault::Config::read_from(&root_vault.config_path()) {
            Ok(read) => read,
            Err(error) => {
                eprintln!(
                    "{}",
                    warn_line!(
                        "The configuration of the root Vault at {} could not be read: {error}",
                        (root_vault.get_root().display())
                    )
                );
                rorolala_vault::Config::default()
            }
        };

        let (roots, rule) = member_scopes(&config, &vault, &root_vault);
        let config = bound(&config, self.max_concurrency);

        Some(Served {
            vault,
            root_vault,
            config,
            root_vault_config,
            roots,
            rule,
        })
    }
}

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
    /// The action itself failed.
    Action(ActionError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handshake(source) => write!(formatter, "the handshake failed: {source}"),
            Self::Exchange(source) => {
                write!(formatter, "the request could not be answered: {source}")
            }
            Self::Key(source) => write!(formatter, "a member's key could not be read: {source}"),
            Self::UnknownMember(name) => write!(formatter, "no member is named {name}"),
            Self::UnknownVault(name) => {
                write!(formatter, "no Vault is served under {name}")
            }
            Self::IdentityMismatch(name) => {
                write!(formatter, "the peer is not the member it named ({name})")
            }
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
