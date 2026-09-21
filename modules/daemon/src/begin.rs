//! The Vault side of the daemon: it listens, proves who it is, and runs the action a
//! Workspace asks it for.
//!
//! A Vault does not reach out; it waits. Every connection is answered on its own, so one
//! Workspace that fails to prove itself, or asks for something the Vault does not serve,
//! never holds up the ones behind it. What a connection needs that does not change — the
//! Vault's identity, where its keys sit, and the actions it serves — is resolved once, as
//! the daemon starts, and shared from there.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rorolala_auth::{KeyLocateRule, Member, SecureStream, SigningKey, find_member};
use rorolala_protocol::{ActionContext, ActionError, Socket};
use rorolala_utils_cli_theme::{err_line, warn_line};
use rorolala_vault::KEYS_DIR;
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
    /// The identity the Vault proves to every peer.
    ///
    /// It is resolved where the daemon is wired up, not here: the Vault side of a
    /// protocol knows peers as [`Member`]s, and the Vault's own private key is host
    /// setup rather than a peer. What comes through is the bare key, not the account it
    /// was read from.
    pub(crate) identity: SigningKey,
    /// The signal used to cancel the daemon.
    pub(crate) signal: CancelSignal,
}

/// Runs the daemon until it is cancelled.
///
/// What does not change while it runs is resolved once here: the directories a member is
/// looked for in and the actions the Vault serves. A daemon that cannot resolve them
/// cannot serve at all, so it says so and stops rather than listening and refusing every
/// connection.
///
/// # Standard Error
///
/// Writes an error log when the address cannot be bound or a connection fails.
pub(crate) async fn daemon(input: DaemonInput<'_>) -> DaemonExit {
    let DaemonInput {
        cwd,
        config,
        identity,
        signal,
    } = input;

    // The Vault's keys are the first scope a member is looked for in.
    let roots = vec![cwd.join(KEYS_DIR)];
    let rule = KeyLocateRule::new();

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
        // Built once: the list a caller's id is read against is the one this daemon runs.
        registry: build_action_registry(),
    });

    listen(listener, host, signal.rx).await;

    DaemonExit::default()
}

/// Everything a connection needs that does not change while the daemon runs.
struct Host {
    /// The identity the Vault proves to every peer.
    signing: SigningKey,
    /// The directories a member may be found in, the Vault's own keys first.
    roots: Vec<PathBuf>,
    /// Which scopes a member may be found in.
    rule: KeyLocateRule,
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
/// The order is what makes a name mean anything. The handshake proves a key; the name the
/// Workspace sends is only a label, so the member that label finds must be the identity
/// that was proved before the action runs. A request that is not confirmed never starts
/// an action, so a Workspace that is refused finds out before it has anything to undo.
async fn serve<Stream>(stream: Stream, host: Arc<Host>) -> Result<(), SessionError>
where
    Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let socket: Box<dyn Socket> = Box::new(stream);
    let mut channel = SecureStream::accept(socket, &host.signing).await?;

    let request = wire::read_request(&mut channel)
        .await
        .map_err(SessionError::Exchange)?;

    let member = host.member(&request.account)?;
    if member.get_key().map_err(SessionError::Key)? != *channel.peer() {
        return Err(SessionError::IdentityMismatch(request.account));
    }

    // Both sides now know they mean the same action, so both start it.
    wire::write_confirmation(&mut channel, request.id)
        .await
        .map_err(SessionError::Exchange)?;

    let ctx = ActionContext::new_vault_ctx(member).with_channel(channel);
    do_action_with(&host.registry, request.id, ctx)
        .await
        .map_err(SessionError::Action)?;

    Ok(())
}

impl Host {
    /// The member `name` refers to, if this Vault can find its key.
    fn member(&self, name: &str) -> Result<Member, SessionError> {
        find_member(name, &self.roots, &self.rule)
            .ok_or_else(|| SessionError::UnknownMember(name.to_string()))
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
    use rorolala_auth::{
        Account, KeyAlgorithm, KeyLocateRule, PublicKey, SecureStream, SigningKey, find_account,
    };
    use rorolala_protocol::{
        Action as _, ActionContext, ActionError, Channel, OnlyWorkspace, Socket,
    };
    use tokio::io::{AsyncWriteExt as _, DuplexStream, duplex};
    use tokio::net::TcpListener;

    use super::{Host, SessionError, serve};
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
            registry,
        }
    }

    /// A Workspace session with a Vault that proves `server`, acting as `identity`.
    async fn connect(io: DuplexStream, identity: &SigningKey, server: &PublicKey) -> Channel {
        SecureStream::connect(Box::new(io) as Box<dyn Socket>, identity, server)
            .await
            .unwrap()
    }

    /// Asks for the action `id` to run as `account`.
    async fn request(channel: &mut Channel, id: u32, account: &str) {
        wire::write_request(
            channel,
            &Request {
                id,
                account: account.to_string(),
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

        assert_eq!(output, "Hello, world ... Welcome!");
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

        let output =
            proc_action::<ActionHandshake>(&account, address.to_string(), "world".to_string())
                .await
                .unwrap();

        assert_eq!(output, "Hello, world ... Welcome!");
        serving.await.unwrap().unwrap();
    }

    /// An action that always fails, so the session's handling of that is exercised.
    struct Failing;

    impl crate::ActionEntry for Failing {
        fn run(&self, _ctx: ActionContext) -> crate::EntryFuture {
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
                SessionError::Action(ActionError::NoChannel),
                "the action failed: the action context has no channel",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[tokio::test]
    async fn a_vault_that_cannot_bind_stops() {
        // Hold the address the daemon will try to take, so binding it must fail.
        let taken = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = taken.local_addr().unwrap().port();
        let config: rorolala_vault::Config =
            serde_json::from_str(&format!("{{\"daemon_config\":{{\"prefer_port\":{port}}}}}"))
                .unwrap();

        let (_tx, rx) = tokio::sync::watch::channel(false);
        let cwd = scratch("bind");
        let daemon = super::daemon(super::DaemonInput {
            cwd: &cwd,
            config: &config,
            identity: signing(40),
            signal: crate::CancelSignal { rx },
        });

        // The daemon stops with the port taken rather than listening somewhere else; a
        // daemon that bound anyway would wait for a cancellation that never comes.
        tokio::time::timeout(std::time::Duration::from_secs(5), daemon)
            .await
            .expect("the daemon stops when the port is taken");
    }
}
