//! The Vault side of the daemon: it listens, proves who it is, and runs the action a
//! Workspace asks it for.
//!
//! A Vault does not reach out; it waits. Every connection is answered on its own, so one
//! Workspace that fails to prove itself, or asks for something the Vault does not serve,
//! never holds up the ones behind it. What a connection needs that does not change — the
//! Vault's identity, where it sits among the Vaults it belongs to, and the actions it serves —
//! is resolved once, as the daemon starts, and shared from there.
//!
//! Which Vault a connection is *served as*, though, changes with every connection: an address
//! reaches a daemon, and the daemon may hold several Vaults under the one it was started for, so
//! the request says which is meant. What is fixed is only what a Vault cannot be talked into —
//! the identity it proves itself with — and what is read per connection is the Vault the
//! request names, what it is configured with, and who it admits.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rorolala_auth::{
    KeyLocateRule, SecureStream, SigningKey, env_keys_dir, find_member, global_keys_dir,
    locate_accounts, user_keys_dir,
};
use rorolala_protocol::{ActionContext, ActionError, Socket};
use rorolala_utils_cli_theme::{err_line, help_line, warn_line};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_location::Locate;
use rorolala_vault::{KEYS_DIR, KeyDiscovery, RootVault, VAULTS_DIR, Vault};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::{ActionEntry, CancelSignal, DaemonExit, build_action_registry, do_action_with, wire};

/// Input provided to the daemon for its operation.
pub(crate) struct DaemonInput<'a> {
    /// The current working directory in which the daemon operates.
    pub(crate) cwd: &'a Path,
    /// The configuration used to run the daemon.
    pub(crate) config: &'a rorolala_vault::Config,
    /// The signal used to cancel the daemon.
    pub(crate) signal: CancelSignal,
}

/// Runs the daemon until it is cancelled.
///
/// What does not change while it runs is resolved once here: the Vault the daemon was started
/// for, the root that holds it, and the identity it proves itself with. A daemon that cannot
/// resolve those cannot serve at all, so it says so and stops rather than listening and refusing
/// every connection. Which Vault under the root a connection is served as is read from each
/// request, so a Vault made while the daemon runs is served without it being restarted.
///
/// # Standard Error
///
/// Writes an error log when the address cannot be bound or a connection fails.
pub(crate) async fn daemon(input: DaemonInput<'_>) -> DaemonExit {
    let DaemonInput {
        cwd,
        config,
        signal,
    } = input;

    // The Vault the daemon was started for, and the root that holds it. Both are resolved once
    // here and kept for as long as the daemon runs, so every connection is served against the
    // same pair rather than looking for them again. The root is a Vault of its own — the
    // outermost above `cwd` — so a Vault-side action that works on the Vaults below it, or on
    // the one holding it, is handed it directly.
    let Some(vault) = Vault::locate(cwd) else {
        eprintln!(
            "{}",
            err_line!("No Vault is at or above {} to serve.", (cwd.display()))
        );
        return DaemonExit::default();
    };
    // A Vault that was found has its root above it — itself, at the least — so the search that
    // found one finds the other.
    let Some(root_vault) = RootVault::locate(cwd) else {
        eprintln!(
            "{}",
            err_line!("No root Vault is at or above {} to serve.", (cwd.display()))
        );
        return DaemonExit::default();
    };

    // Where the members of the daemon's own Vault are looked for, which its configuration says:
    // a Vault that names no place admits nobody. A Vault below it is served under the places its
    // own configuration names, resolved per connection.
    let (roots, rule) = member_scopes(config, &vault, &root_vault);

    // The identity the Vault proves itself with comes from the same places a member is looked
    // for in: a Vault that keeps its own key pair is proved by it, and one under a root that
    // keeps the group's — a sub-vault — is proved by the root's.
    let Some(identity) = vault_identity(&roots, &rule, vault.get_root()) else {
        return DaemonExit::default();
    };

    // What the root above is configured with, which a Vault-side action that works on the
    // Vaults below it is handed. The file is the Vault's own when the Vault served is the root
    // itself.
    let root_vault_config = match rorolala_vault::Config::read_from(&root_vault.config_path()) {
        Ok(read) => read,
        Err(error) => {
            eprintln!(
                "{}",
                warn_line!("The root Vault's configuration could not be read: {error}")
            );
            rorolala_vault::Config::default()
        }
    };

    let address = SocketAddr::from(([0, 0, 0, 0], config.daemon_config().prefer_port()));
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "{}",
                err_line!("The daemon could not listen on {address}: {error}")
            );
            return DaemonExit::default();
        }
    };

    let host = Arc::new(Host {
        signing: identity,
        roots,
        rule,
        vault,
        root_vault,
        vault_config: config.clone(),
        root_vault_config,
        // Built once: the list a caller's id is read against is the one this daemon runs.
        registry: build_action_registry(),
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

    let rule = KeyLocateRule {
        find_global: false,
        find_local: true,
        find_user: false,
        find_env: false,
    };

    (roots, rule)
}

/// The identity the Vault proves, read from the first account the places hold.
///
/// Which key a Vault is, is host setup, not something the Vault *side* of a protocol touches:
/// there, a peer is a [`Member`] and nothing else. Resolving it here keeps that side free of
/// the Workspace's notion of an account, and hands on the bare key it needs.
///
/// # Standard Error
///
/// Writes an error log when no place holds an account, or its key cannot be read.
fn vault_identity(
    roots: &[PathBuf],
    rule: &KeyLocateRule,
    vault_root: &Path,
) -> Option<SigningKey> {
    let accounts = locate_accounts(roots, rule);

    let Some(account) = accounts.iter().next() else {
        // The directory is walked back into the path it names before it is spoken of: the
        // layout names it `./keys/`, and joining that onto a root leaves the `./` in the
        // middle of the path a reader is shown.
        let own = vault_root.join(KEYS_DIR).components().collect::<PathBuf>();

        eprintln!(
            "{}",
            err_line!(
                "The Vault holds no account to prove itself with, in its own keys at {} or in the places its configuration names.",
                (own.display())
            )
        );
        eprintln!(
            "{}",
            help_line!(
                "Please give the Vault a key pair of its own — `rola tool-keygen keys/vault` makes one in its keys directory"
            )
        );
        return None;
    };

    match account.get_key() {
        Ok(identity) => Some(identity),
        Err(error) => {
            eprintln!(
                "{}",
                err_line!("The Vault's own key could not be read: {error}")
            );
            None
        }
    }
}

/// Everything the daemon needs to answer a connection.
///
/// What a Vault cannot be talked into — the identity it proves itself with — is fixed here, and
/// so is the Vault the daemon was started for, with the scopes and configuration it runs under.
/// The root above it is fixed too, since which Vaults a request may name is read against it. What
/// a request names under the root is resolved per connection rather than kept here.
struct Host {
    /// The identity the Vault proves to every peer.
    signing: SigningKey,
    /// The directories a member of the daemon's own Vault may be found in, in the order it names
    /// them.
    roots: Vec<PathBuf>,
    /// The scopes a member may be found in, which is the directories above and no other.
    rule: KeyLocateRule,
    /// The Vault the daemon was started for, as a context hands it to an action.
    vault: Vault,
    /// The root above [`vault`](Self::vault), as a context hands it to an action.
    root_vault: RootVault,
    /// What the Vault the daemon was started for is configured with, as a context hands it to an
    /// action.
    vault_config: rorolala_vault::Config,
    /// What the root above it is configured with, as a context hands it to an action.
    root_vault_config: rorolala_vault::Config,
    /// Every action this daemon serves, laid out by id.
    registry: Vec<Option<Box<dyn ActionEntry>>>,
}

/// Serves connections until `cancel` fires.
///
/// Each connection is a job of its own, so a slow or hostile peer holds up nothing but
/// itself. The loop itself does no work beyond accepting, which is what lets cancellation
/// take effect between two connections rather than after the current one.
async fn listen(listener: TcpListener, host: Arc<Host>, mut cancel: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    let host = Arc::clone(&host);
                    tokio::spawn(async move {
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

    // The Vault being served and its root live for this connection — one of them is built for
    // it — so the action reaches them, and what either is configured with, by borrowing them for
    // as long as the action runs.
    let ctx = ActionContext::new_vault_ctx(member)
        .with_current_vault(&served.vault)
        .with_current_root_vault(&host.root_vault)
        .with_current_vault_config(&served.config)
        .with_current_root_vault_config(&host.root_vault_config)
        .with_channel(channel);
    do_action_with(&host.registry, request.id, ctx)
        .await
        .map_err(SessionError::Action)?;

    Ok(())
}

/// The Vault one connection is served for, with what it is served with.
///
/// A Vault is one thing for the duration of a connection and a different one for the next, so
/// what a request names is resolved into this: the Vault itself, what its file — or, for the
/// daemon's own, what was handed in — is configured with, and where its members are looked for.
struct Served {
    /// The Vault the action is run over.
    vault: Vault,
    /// What that Vault is configured with, as a context hands it to an action.
    config: rorolala_vault::Config,
    /// The directories a member of it may be found in, nearest first.
    roots: Vec<PathBuf>,
    /// The scopes a member may be found in, which is the directories above and no other.
    rule: KeyLocateRule,
}

impl Host {
    /// The Vault a request naming `sub` is served for, if this daemon can serve one there.
    ///
    /// Nothing names the Vault the daemon was started for; anything else names a Vault under the
    /// root it sits in, either by the path it sits at or by the name the root holds it under.
    /// A name that reaches no Vault is a request this daemon cannot answer.
    ///
    /// What the Vault is configured with, and who it admits, are read here rather than once at
    /// startup: a Vault made or reconfigured while the daemon runs is served under the rules it
    /// has, and a Vault under a root that keeps the group's keys admits them without either
    /// Vault having to be restarted.
    async fn serve_for(&self, sub: &str) -> Option<Served> {
        let vault = self.vault_named(sub).await?;
        let own = vault.get_root() == self.vault.get_root();

        // The daemon's own Vault is what it was started for, so what it is configured with is
        // what was handed in rather than what the file says: a configuration changed for this run
        // is not one to read back. A Vault below it is known here only by what its file says, so
        // the file is where it is read from — and a file that will not read leaves a Vault
        // configured as a fresh one rather than one that cannot be served at all.
        let config = if own {
            self.vault_config.clone()
        } else {
            match rorolala_vault::Config::read_from(&vault.config_path()) {
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
            }
        };

        // The daemon's own scopes were resolved once from the configuration it runs with, so they
        // are taken as they are; a Vault below it is looked for in the places its own
        // configuration names.
        let (roots, rule) = if own {
            (self.roots.clone(), self.rule.clone())
        } else {
            member_scopes(&config, &vault, &self.root_vault)
        };

        Some(Served {
            vault,
            config,
            roots,
            rule,
        })
    }

    /// The Vault `sub` names, if the root holds one there.
    ///
    /// Nothing, `.`, and a path that walks to where it started all name the Vault the daemon
    /// serves itself, whatever it is. Anything else goes through the root: as the path it sits at
    /// under the root — which is what an address written out in full says — or, when that names
    /// nothing, as the name the root holds it under, which is how a Vault is named when it is
    /// made and how it is reached without writing the path out.
    ///
    /// The path is the root's to read, so nothing here can climb out of the root on its own — see
    /// [`RootVault::get_vault_by_path`] — and a Vault the root reaches through a symlink is
    /// reached the same way here.
    async fn vault_named(&self, sub: &str) -> Option<Vault> {
        let sub = sub.trim_matches('/');

        if sub.is_empty() || sub == "." {
            return Some(self.vault.clone());
        }

        if let Some(named) = self.root_vault.get_vault_by_path(sub).await {
            return Some(named);
        }

        self.root_vault
            .get_vault_by_path(format!("{VAULTS_DIR}/{sub}"))
            .await
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
    /// The request names a Vault this daemon does not serve.
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
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ed25519_dalek::SigningKey as Ed25519SigningKey;
    use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
    use ed25519_dalek::pkcs8::{EncodePrivateKey as _, EncodePublicKey as _};
    use rorolala_auth::Error as AuthError;
    use rorolala_auth::user_keys_dir;
    use rorolala_auth::{
        Account, KeyAlgorithm, KeyLocateRule, PublicKey, SecureStream, SigningKey, find_account,
    };
    use rorolala_protocol::{
        Action as _, ActionContext, ActionError, Channel, OnlyWorkspace, Socket,
    };
    use rorolala_utils_location::Locate;
    use rorolala_vault::{KEYS_DIR, RootVault, VAULTS_DIR, Vault};
    use rorolala_workspace::Workspace;
    use tokio::io::{AsyncWriteExt as _, DuplexStream, duplex};
    use tokio::net::TcpListener;

    use super::{Host, SessionError, member_scopes, serve, vault_identity};
    use crate::{
        ActionHandshake, build_action_registry, proc_action,
        wire::{self, Request},
    };

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-daemon-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// An identity from a fixed seed, so a test names the same key twice.
    fn signing(seed: u8) -> SigningKey {
        SigningKey::from_bytes(KeyAlgorithm::Ed25519, [seed; 32]).unwrap()
    }

    /// Writes the public key of `identity` as the member `name`, where a search finds it.
    fn publish(keys: &Path, name: &str, identity: &Ed25519SigningKey) {
        let pem = identity
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        fs::write(keys.join(format!("{name}.pub")), pem).unwrap();
    }

    /// Writes the private key of `identity` as the account `name`, where a search finds it.
    fn enroll(keys: &Path, name: &str, identity: &Ed25519SigningKey) {
        let pem = identity.to_pkcs8_pem(LineEnding::LF).unwrap();
        fs::write(keys.join(format!("{name}.pem")), pem.as_str()).unwrap();
    }

    /// A rule that looks only at the Vault's own keys, so a test sees only what it made.
    fn local_only() -> KeyLocateRule {
        KeyLocateRule {
            find_global: false,
            find_local: true,
            find_user: false,
            find_env: false,
        }
    }

    /// A Vault that proves `identity`, knows the members under `keys`, and serves every
    /// action the daemon does.
    fn vault(keys: PathBuf, identity: SigningKey) -> Host {
        vault_serving(keys, identity, build_action_registry())
    }

    /// A Vault that serves `registry` rather than the actions the daemon ships.
    fn vault_serving(
        keys: PathBuf,
        identity: SigningKey,
        registry: Vec<Option<Box<dyn crate::ActionEntry>>>,
    ) -> Host {
        Host {
            signing: identity,
            roots: vec![keys],
            rule: local_only(),
            // The Vault a connection is served for is the daemon's to keep; a test that checks
            // what an action is handed over the wire does not reach for it.
            vault: Vault::default(),
            root_vault: RootVault::default(),
            vault_config: rorolala_vault::Config::default(),
            root_vault_config: rorolala_vault::Config::default(),
            registry,
        }
    }

    /// A Workspace session with a Vault that proves `server`, acting as `identity`.
    async fn connect(io: DuplexStream, identity: &SigningKey, server: &PublicKey) -> Channel {
        SecureStream::connect(Box::new(io) as Box<dyn Socket>, identity, server)
            .await
            .unwrap()
    }

    /// Asks for the action `id` to run as `account`, against the Vault the daemon serves itself.
    async fn request(channel: &mut Channel, id: u32, account: &str) {
        request_sub(channel, id, account, "").await;
    }

    /// Asks for the action `id` to run as `account`, against the Vault `sub` names.
    async fn request_sub(channel: &mut Channel, id: u32, account: &str, sub: &str) {
        wire::write_request(
            channel,
            &Request {
                id,
                account: account.to_string(),
                sub: sub.to_string(),
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn a_workspace_that_names_a_member_runs_the_action() {
        let keys = scratch("runs");
        let server = signing(1);
        let client = Ed25519SigningKey::from_bytes(&[2; 32]);
        publish(&keys, "client", &client);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(vault(keys, signing(1)))));

        let mut channel = connect(client_io, &signing(2), &server.public_key()).await;
        request(&mut channel, ActionHandshake::ID, "client").await;
        let confirmation = wire::read_confirmation(&mut channel).await.unwrap();
        assert_eq!(confirmation, wire::confirmation(ActionHandshake::ID));

        // The input is a value only the Workspace holds, so it is where the action
        // starts; both sides end up holding what comes back.
        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_channel(channel);

        let output = ActionHandshake::process(OnlyWorkspace::from(Some("world".to_string())), ctx)
            .await
            .unwrap();

        // The greeting is the Vault's own voice, so what it says about itself is what the Vault
        // it was handed says — the default here, since the Host under it was configured with
        // nothing.
        assert_eq!(output, "Hello, world, I'm unknown_vault.\n\nUnnamed Vault");
        serving.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_workspace_that_names_nobody_is_refused() {
        let keys = scratch("unknown");
        let server = signing(3);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(vault(keys, signing(3)))));

        let mut channel = connect(client_io, &signing(4), &server.public_key()).await;
        request(&mut channel, ActionHandshake::ID, "nobody").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::UnknownMember(name) if name == "nobody"));

        // No confirmation came, so the client finds the channel closed rather than a
        // go-ahead it must not have.
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn a_request_that_names_no_vault_the_daemon_serves_is_refused() {
        let keys = scratch("unknown-vault");
        let server = signing(31);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(vault(keys, signing(31)))));

        // The daemon's own Vault is the default, so a name under it that reaches nothing is a
        // Vault this daemon does not serve — refused before any member is looked for.
        let mut channel = connect(client_io, &signing(32), &server.public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "nowhere").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::UnknownVault(sub) if sub == "nowhere"));
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn an_action_runs_over_the_vault_the_request_names() {
        let root = scratch("served-root");
        let alpha = root.join(VAULTS_DIR).join("alpha");
        let server = signing(33);
        let client = Ed25519SigningKey::from_bytes(&[34; 32]);
        // Two Vaults, and a caller the one under the root admits while the root itself does not:
        // what comes back says which of them the request reached.
        Vault::create(&root).unwrap();
        Vault::create(&alpha).unwrap();
        publish(&alpha.join(KEYS_DIR), "client", &client);

        // The daemon is started for the root, so its own scopes hold no member: only a request
        // that names the Vault below reaches the caller, and it is reached under that Vault's own
        // rules.
        let host = Host {
            signing: signing(33),
            roots: Vec::new(),
            rule: local_only(),
            vault: Vault::locate(&root).unwrap(),
            root_vault: RootVault::locate(&root).unwrap(),
            vault_config: rorolala_vault::Config::default(),
            root_vault_config: rorolala_vault::Config::default(),
            registry: build_action_registry(),
        };

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host)));

        let mut channel = connect(client_io, &signing(34), &server.public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "alpha").await;
        let confirmation = wire::read_confirmation(&mut channel).await.unwrap();
        assert_eq!(confirmation, wire::confirmation(ActionHandshake::ID));

        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_channel(channel);
        let output = ActionHandshake::process(OnlyWorkspace::from(Some("world".to_string())), ctx)
            .await
            .unwrap();

        // The greeting is named after the Vault the request reached, which is the one under the
        // root rather than the one the daemon was started for.
        assert_eq!(output, "Hello, world, I'm Alpha.\n\nNew Rola Vault");
        serving.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_sub_is_the_path_a_vault_sits_at_or_the_name_it_is_held_under() {
        let root = scratch("named");
        let alpha = root.join(VAULTS_DIR).join("alpha");
        Vault::create(&root).unwrap();
        Vault::create(&alpha).unwrap();

        let host = Host {
            signing: signing(35),
            roots: Vec::new(),
            rule: local_only(),
            vault: Vault::locate(&root).unwrap(),
            root_vault: RootVault::locate(&root).unwrap(),
            vault_config: rorolala_vault::Config::default(),
            root_vault_config: rorolala_vault::Config::default(),
            registry: build_action_registry(),
        };

        // Nothing, `.`, and the path the served Vault sits at all name the Vault the daemon
        // serves itself.
        for own in ["", ".", "/"] {
            assert_eq!(
                host.vault_named(own).await.unwrap().get_root(),
                root.as_path()
            );
        }

        // A Vault the root holds is reached by the name it is held under, by the path it sits at,
        // and with either written with a trailing separator.
        for named in ["alpha", "vaults/alpha", "alpha/", "/vaults/alpha"] {
            assert_eq!(
                host.vault_named(named).await.unwrap().get_root(),
                alpha.as_path(),
                "{named}"
            );
        }

        // A name the root holds nothing under names no Vault.
        assert!(host.vault_named("nowhere").await.is_none());
    }

    #[tokio::test]
    async fn a_workspace_that_names_someone_else_is_refused() {
        let keys = scratch("mismatch");
        let server = signing(5);
        // The Vault knows "client" as one key, but the peer proves another.
        publish(&keys, "client", &Ed25519SigningKey::from_bytes(&[6; 32]));

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(vault(keys, signing(5)))));

        let mut channel = connect(client_io, &signing(7), &server.public_key()).await;
        request(&mut channel, ActionHandshake::ID, "client").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::IdentityMismatch(name) if name == "client"));
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn a_workspace_reaches_a_listening_vault_over_a_socket() {
        let keys = scratch("tcp-keys");
        let workspace = scratch("tcp-workspace");
        let client = Ed25519SigningKey::from_bytes(&[12; 32]);
        // The Vault knows the caller as a member; the workspace holds the key it acts as.
        publish(&keys, "client", &client);
        enroll(&workspace, "client", &client);

        let host = Arc::new(vault(keys, signing(11)));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let serving = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            serve(stream, host).await
        });

        // The account is found the way a client finds one: by name, in its own keys.
        let account =
            find_account("client", std::slice::from_ref(&workspace), &local_only()).unwrap();

        let output = proc_action::<ActionHandshake>(
            &Workspace::default(),
            &account,
            address.to_string(),
            "world".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(output, "Hello, world, I'm unknown_vault.\n\nUnnamed Vault");
        serving.await.unwrap().unwrap();
    }

    /// An action that always fails, so the session's handling of that is exercised.
    struct Failing;

    impl crate::ActionEntry for Failing {
        fn run<'a>(&self, _ctx: ActionContext<'a>) -> crate::EntryFuture<'a> {
            Box::pin(async { Err(ActionError::NoChannel) })
        }
    }

    #[tokio::test]
    async fn a_request_the_vault_cannot_read_fails_the_exchange() {
        let keys = scratch("exchange");
        let server = signing(20);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(vault(keys, signing(20)))));

        let mut channel = connect(client_io, &signing(21), &server.public_key()).await;
        // A name longer than any request may carry is not one the vault reads, so the
        // request never reaches the member lookup behind it.
        request(&mut channel, ActionHandshake::ID, &"a".repeat(5000)).await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(
            error,
            SessionError::Exchange(ActionError::ValueTooLarge)
        ));
    }

    #[tokio::test]
    async fn a_members_key_that_cannot_be_read_fails_the_session() {
        let keys = scratch("bad-key");
        let server = signing(22);
        // The member is found by name, but the file it names is not a key.
        fs::write(keys.join("client.pub"), "not a key").unwrap();

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(vault(keys, signing(22)))));

        let mut channel = connect(client_io, &signing(23), &server.public_key()).await;
        request(&mut channel, ActionHandshake::ID, "client").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::Key(_)));
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn an_action_that_fails_fails_the_session() {
        let keys = scratch("action");
        let server = signing(24);
        let client = Ed25519SigningKey::from_bytes(&[25; 32]);
        publish(&keys, "client", &client);

        // The member and the confirmation are both fine; it is the action itself that
        // cannot be carried out.
        let registry: Vec<Option<Box<dyn crate::ActionEntry>>> = vec![Some(Box::new(Failing))];
        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(vault_serving(keys, signing(24), registry)),
        ));

        let mut channel = connect(client_io, &signing(25), &server.public_key()).await;
        request(&mut channel, ActionHandshake::ID, "client").await;
        let confirmation = wire::read_confirmation(&mut channel).await.unwrap();
        assert_eq!(confirmation, wire::confirmation(ActionHandshake::ID));

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(
            error,
            SessionError::Action(ActionError::NoChannel)
        ));
    }

    #[tokio::test]
    async fn a_peer_that_does_not_handshake_fails_the_session() {
        let (mut client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(vault(scratch("handshake"), signing(26))),
        ));

        // A message of no length is not the offer a handshake opens with, so accepting
        // the peer fails and the connection is refused.
        client_io.write_all(&[0, 0, 0, 0]).await.unwrap();
        client_io.flush().await.unwrap();

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::Handshake(_)));
    }

    #[test]
    fn every_session_failure_says_what_went_wrong() {
        let cases = [
            (
                SessionError::Handshake(AuthError::Malformed),
                "the handshake failed: a key or signature was malformed",
            ),
            (
                SessionError::Exchange(ActionError::NoChannel),
                "the request could not be answered: the action context has no channel",
            ),
            (
                SessionError::Key(AuthError::Malformed),
                "a member's key could not be read: a key or signature was malformed",
            ),
            (
                SessionError::UnknownMember("alice".to_string()),
                "no member is named alice",
            ),
            (
                SessionError::IdentityMismatch("bob".to_string()),
                "the peer is not the member it named (bob)",
            ),
            (
                SessionError::UnknownVault("alpha".to_string()),
                "no Vault is served under alpha",
            ),
            (
                SessionError::Action(ActionError::NoChannel),
                "the action failed: the action context has no channel",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    /// The identity the daemon runs as, looked for in the Vault's own keys alone.
    fn identity_of(cwd: &Path) -> Option<SigningKey> {
        vault_identity(&[cwd.join(KEYS_DIR)], &local_only(), cwd)
    }

    #[test]
    fn a_vault_with_no_account_cannot_prove_itself() {
        let cwd = scratch("no-account");

        assert!(identity_of(&cwd).is_none());
    }

    #[test]
    fn a_vault_whose_key_cannot_be_read_cannot_prove_itself() {
        let cwd = scratch("bad-key");
        // The account is found by name, but the file it names is not a key.
        fs::create_dir_all(cwd.join(KEYS_DIR)).unwrap();
        fs::write(cwd.join(KEYS_DIR).join("vault.pem"), "not a key").unwrap();

        assert!(identity_of(&cwd).is_none());
    }

    #[test]
    fn a_vault_whose_key_can_be_read_proves_it() {
        let cwd = scratch("key");
        let signing = Ed25519SigningKey::from_bytes(&[30; 32]);
        fs::create_dir_all(cwd.join(KEYS_DIR)).unwrap();
        enroll(&cwd.join(KEYS_DIR), "vault", &signing);

        let identity = identity_of(&cwd).unwrap();

        assert_eq!(
            identity.public_key().as_bytes(),
            signing.verifying_key().to_bytes()
        );
    }

    #[test]
    fn a_member_is_looked_for_only_where_the_vault_names() {
        // One place named, one directory handed over, and the rule searches what it is handed
        // rather than naming any scope a second time.
        let config: rorolala_vault::Config =
            serde_json::from_str("{\"daemon_config\":{\"key_discovery\":[\"vault\"]}}").unwrap();
        let (roots, rule) = member_scopes(&config, &Vault::default(), &RootVault::default());

        assert_eq!(roots, [PathBuf::new().join(KEYS_DIR)]);
        assert!(rule.find_local);
        assert!(!rule.find_global && !rule.find_user && !rule.find_env);

        // The user's own store, when it is named, is the directory it is everywhere else.
        let config: rorolala_vault::Config =
            serde_json::from_str("{\"daemon_config\":{\"key_discovery\":[\"user\"]}}").unwrap();
        let (roots, _) = member_scopes(&config, &Vault::default(), &RootVault::default());

        assert_eq!(roots, [user_keys_dir().unwrap()]);
    }

    #[test]
    fn a_vault_that_names_no_place_looks_nowhere() {
        let config: rorolala_vault::Config =
            serde_json::from_str("{\"daemon_config\":{\"key_discovery\":[]}}").unwrap();
        let (roots, _) = member_scopes(&config, &Vault::default(), &RootVault::default());

        assert!(roots.is_empty());
    }

    #[tokio::test]
    async fn a_vault_that_cannot_bind_stops() {
        // Hold the address the daemon will try to take, so binding it must fail.
        let taken = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = taken.local_addr().unwrap().port();
        let config: rorolala_vault::Config = serde_json::from_str(&format!(
            "{{\"daemon_config\":{{\"prefer_port\":{port},\"key_discovery\":[\"vault\"]}}}}"
        ))
        .unwrap();

        let (_tx, rx) = tokio::sync::watch::channel(false);
        let cwd = scratch("bind");
        // The daemon serves a Vault and proves itself with an account of its own, so both are
        // made: what stops it here is the port, and nothing else.
        Vault::create(&cwd).unwrap();
        enroll(
            &cwd.join(KEYS_DIR),
            "vault",
            &Ed25519SigningKey::from_bytes(&[40; 32]),
        );

        let daemon = super::daemon(super::DaemonInput {
            cwd: &cwd,
            config: &config,
            signal: crate::CancelSignal { rx },
        });

        // The daemon stops with the port taken rather than listening somewhere else; a
        // daemon that bound anyway would wait for a cancellation that never comes.
        tokio::time::timeout(std::time::Duration::from_secs(5), daemon)
            .await
            .expect("the daemon stops when the port is taken");
    }
}
