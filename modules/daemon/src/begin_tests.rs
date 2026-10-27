
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ed25519_dalek::SigningKey as Ed25519SigningKey;
    use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
    use ed25519_dalek::pkcs8::{EncodePrivateKey as _, EncodePublicKey as _};
    use rorolala_auth::Error as AuthError;
    use rorolala_auth::{
        Account, KeyAlgorithm, KeyLocateRule, PublicKey, SecureStream, SigningKey, find_account,
    };
    use rorolala_utils_location::Locate;
    use rorolala_protocol::{
        Action as _, ActionContext, ActionError, Channel, OnlyWorkspace, Socket,
    };
    use rorolala_utils_progress::Progress;
    use rorolala_vault::{KEYS_DIR, RootVault, VAULTS_DIR, Vault};
    use rorolala_workspace::Workspace;
    use tokio::io::{AsyncWriteExt as _, DuplexStream, duplex};
    use tokio::net::TcpListener;

    use super::{Host, SessionError, identity_in, member_scopes, serve, split_namespace};
    use crate::{
        ActionHandshake, Namespaces, build_action_registry, proc_action,
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

    /// A Vault named `name`, admitting members from its own keys alone.
    fn vault_named(dir: &Path, name: &str) {
        Vault::create(dir).unwrap();
        fs::write(
            dir.join(rorolala_vault::CONFIG_PATH),
            format!(
                "[vault_config]\nvault_name = \"{name}\"\nvault_description = \"A test Vault\"\n\n\
                 [daemon_config]\nkey_discovery = [\"vault\"]\n"
            ),
        )
        .unwrap();
    }

    /// The table naming each of `entries` under the namespace beside it.
    fn registry(dir: &Path, entries: &[(&str, &Path)]) -> PathBuf {
        let file = dir.join("registry.toml");
        let mut namespaces = Namespaces::default();

        for (namespace, path) in entries {
            namespaces.register(namespace, *path).unwrap();
        }

        namespaces.write_to(&file).unwrap();
        file
    }

    /// A daemon serving `registry`, proving `identity`.
    fn host(registry: PathBuf, identity: SigningKey) -> Host {
        Host {
            signing: identity,
            registry,
            max_concurrency: None,
            actions: build_action_registry(),
            served: StdMutex::new(HashMap::new()),
        }
    }

    /// A Workspace session with a Vault that proves `server`, acting as `identity`.
    async fn connect(io: DuplexStream, identity: &SigningKey, server: &PublicKey) -> Channel {
        SecureStream::connect(Box::new(io) as Box<dyn Socket>, identity, server)
            .await
            .unwrap()
    }

    /// Asks for the action `id` to run as `account`, against `sub`.
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

    #[test]
    fn a_namespace_is_the_first_segment_and_the_rest_is_a_vault_below_it() {
        assert_eq!(split_namespace(""), ("", ""));
        assert_eq!(split_namespace("/"), ("", ""));
        assert_eq!(split_namespace("alpha"), ("alpha", ""));
        assert_eq!(split_namespace("alpha/"), ("alpha", ""));
        assert_eq!(split_namespace("/alpha"), ("alpha", ""));
        assert_eq!(split_namespace("alpha/beta"), ("alpha", "beta"));
        assert_eq!(split_namespace("alpha/vaults/beta/"), ("alpha", "vaults/beta"));
    }

    #[test]
    fn an_identity_is_the_first_account_the_daemon_can_find() {
        let empty = scratch("no-account");
        assert!(identity_in(std::slice::from_ref(&empty)).is_none());

        let bad = scratch("bad-key");
        fs::write(bad.join("vault.pem"), "not a key").unwrap();
        assert!(identity_in(std::slice::from_ref(&bad)).is_none());

        let good = scratch("key");
        let signing = Ed25519SigningKey::from_bytes(&[30; 32]);
        enroll(&good, "vault", &signing);

        let identity = identity_in(std::slice::from_ref(&good)).unwrap();

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

        assert_eq!(roots, [rorolala_auth::user_keys_dir().unwrap()]);
    }

    #[test]
    fn a_vault_that_names_no_place_looks_nowhere() {
        let config: rorolala_vault::Config =
            serde_json::from_str("{\"daemon_config\":{\"key_discovery\":[]}}").unwrap();
        let (roots, _) = member_scopes(&config, &Vault::default(), &RootVault::default());

        assert_eq!(roots, Vec::<PathBuf>::new());
    }

    #[tokio::test]
    async fn a_workspace_that_names_a_member_runs_the_action() {
        let dir = scratch("runs");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let client = Ed25519SigningKey::from_bytes(&[2; 32]);
        publish(&vault.join(KEYS_DIR), "client", &client);

        let registry = registry(&scratch("runs-registry"), &[("own", &vault)]);
        let server = signing(1);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host(registry, signing(1)))));

        let mut channel = connect(client_io, &signing(2), &server.public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "own").await;
        let confirmation = wire::read_confirmation(&mut channel).await.unwrap();
        assert_eq!(confirmation, wire::confirmation(ActionHandshake::ID));

        // The input is a value only the Workspace holds, so it is where the action
        // starts; both sides end up holding what comes back.
        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_channel(channel);

        let output = ActionHandshake::process(OnlyWorkspace::from(Some("world".to_string())), ctx)
            .await
            .unwrap();

        // What it says about itself is what the Vault the namespace named says.
        assert_eq!(output, "Hello, world, I'm Own Vault.\n\nA test Vault");
        serving.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_workspace_that_names_nobody_is_refused() {
        let dir = scratch("unknown");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let registry = registry(&scratch("unknown-registry"), &[("own", &vault)]);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host(registry, signing(3)))));

        let mut channel = connect(client_io, &signing(4), &signing(3).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "nobody", "own").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::UnknownMember(name) if name == "nobody"));

        // No confirmation came, so the client finds the channel closed rather than a
        // go-ahead it must not have.
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn a_request_that_names_no_served_namespace_is_refused() {
        let dir = scratch("unknown_vault");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let registry = registry(&scratch("unknown-vault-registry"), &[("own", &vault)]);

        for sub in ["", "nowhere"] {
            let (client_io, server_io) = duplex(64 * 1024);
            let serving = tokio::spawn(serve(server_io, Arc::new(host(registry.clone(), signing(31)))));

            let mut channel = connect(client_io, &signing(32), &signing(31).public_key()).await;
            request_sub(&mut channel, ActionHandshake::ID, "client", sub).await;

            let error = serving.await.unwrap().unwrap_err();
            assert!(matches!(error, SessionError::UnknownVault(name) if name == sub));
            assert!(wire::read_confirmation(&mut channel).await.is_err());
        }
    }

    #[tokio::test]
    async fn an_action_runs_over_the_vault_the_namespace_holds() {
        let root = scratch("served-root");
        let alpha = root.join(VAULTS_DIR).join("alpha");
        vault_named(&root, "Root Vault");
        vault_named(&alpha, "Alpha");
        let client = Ed25519SigningKey::from_bytes(&[34; 32]);
        // Two Vaults, and a caller the one under the root admits while the root itself does not:
        // what comes back says which of them the request reached.
        publish(&alpha.join(KEYS_DIR), "client", &client);

        let registry = registry(&scratch("served-registry"), &[("root", &root)]);
        let host = host(registry, signing(33));

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host)));

        let mut channel = connect(client_io, &signing(34), &signing(33).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "root/alpha").await;
        let confirmation = wire::read_confirmation(&mut channel).await.unwrap();
        assert_eq!(confirmation, wire::confirmation(ActionHandshake::ID));

        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_channel(channel);
        let output = ActionHandshake::process(OnlyWorkspace::from(Some("world".to_string())), ctx)
            .await
            .unwrap();

        // The greeting is named after the Vault the request reached, which is the one under the
        // namespace rather than the one the namespace names.
        assert_eq!(output, "Hello, world, I'm Alpha.\n\nA test Vault");
        serving.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_sub_is_the_path_a_vault_sits_at_or_the_name_it_is_held_under() {
        let root = scratch("named");
        let alpha = root.join(VAULTS_DIR).join("alpha");
        vault_named(&root, "Root Vault");
        vault_named(&alpha, "Alpha");

        let registry = registry(&scratch("named-registry"), &[("ns", &root)]);
        let host = host(registry, signing(35));

        // Nothing, `.`, and the path the namespace names all name the Vault the namespace
        // registered itself.
        for own in ["ns", "ns/.", "ns/"] {
            assert_eq!(
                host.serve_for(own).await.unwrap().vault.get_root(),
                root.as_path()
            );
        }

        // A Vault the root holds is reached by the name it is held under, by the path it sits at,
        // and with either written with a trailing separator.
        for named in [
            "ns/alpha",
            "ns/vaults/alpha",
            "ns/alpha/",
            "ns//vaults/alpha",
        ] {
            assert_eq!(
                host.serve_for(named).await.unwrap().vault.get_root(),
                alpha.as_path(),
                "{named}"
            );
        }

        // A name the root holds nothing under names no Vault.
        assert!(host.serve_for("ns/nowhere").await.is_none());
    }

    #[tokio::test]
    async fn a_workspace_that_names_someone_else_is_refused() {
        let dir = scratch("mismatch");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        // The Vault knows "client" as one key, but the peer proves another.
        publish(&vault.join(KEYS_DIR), "client", &Ed25519SigningKey::from_bytes(&[6; 32]));

        let registry = registry(&scratch("mismatch-registry"), &[("own", &vault)]);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host(registry, signing(5)))));

        let mut channel = connect(client_io, &signing(7), &signing(5).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "own").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::IdentityMismatch(name) if name == "client"));
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn a_workspace_that_names_a_path_is_refused() {
        let parent = scratch("named-path");
        let vault = parent.join("own");
        vault_named(&vault, "Own Vault");
        // A member's key kept beside the scope rather than in it: a name that climbed would reach
        // it, and must not.
        publish(&parent, "client", &Ed25519SigningKey::from_bytes(&[10; 32]));

        let registry = registry(&scratch("named-path-registry"), &[("own", &vault)]);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host(registry, signing(9)))));

        let mut channel = connect(client_io, &signing(10), &signing(9).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "../client", "own").await;

        // The file beside the scope holds the very key the peer proves, so only the name keeps the
        // request out.
        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::UnknownMember(name) if name == "../client"));
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn a_workspace_reaches_a_listening_vault_over_a_socket() {
        let dir = scratch("tcp");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let workspace = scratch("tcp-workspace");
        let client = Ed25519SigningKey::from_bytes(&[12; 32]);
        // The Vault knows the caller as a member; the workspace holds the key it acts as.
        publish(&vault.join(KEYS_DIR), "client", &client);
        enroll(&workspace, "client", &client);

        let registry = registry(&scratch("tcp-registry"), &[("own", &vault)]);
        let host = Arc::new(host(registry, signing(11)));
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
            format!("{address}/own"),
            "world".to_string(),
            Progress::silent(),
        )
        .await
        .unwrap();

        assert_eq!(output, "Hello, world, I'm Own Vault.\n\nA test Vault");
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
        let dir = scratch("exchange");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let registry = registry(&scratch("exchange-registry"), &[("own", &vault)]);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host(registry, signing(20)))));

        let mut channel = connect(client_io, &signing(21), &signing(20).public_key()).await;
        // A name longer than any request may carry is not one the vault reads, so the
        // request never reaches the member lookup behind it.
        request_sub(&mut channel, ActionHandshake::ID, &"a".repeat(5000), "own").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(
            error,
            SessionError::Exchange(ActionError::ValueTooLarge)
        ));
    }

    #[tokio::test]
    async fn a_members_key_that_cannot_be_read_fails_the_session() {
        let dir = scratch("bad-key");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        // The member is found by name, but the file it names is not a key.
        fs::write(vault.join(KEYS_DIR).join("client.pub"), "not a key").unwrap();
        let registry = registry(&scratch("bad-key-registry"), &[("own", &vault)]);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host(registry, signing(22)))));

        let mut channel = connect(client_io, &signing(23), &signing(22).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "own").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::Key(_)));
        assert!(wire::read_confirmation(&mut channel).await.is_err());
    }

    #[tokio::test]
    async fn an_action_that_fails_fails_the_session() {
        let dir = scratch("action");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let client = Ed25519SigningKey::from_bytes(&[25; 32]);
        publish(&vault.join(KEYS_DIR), "client", &client);

        // The member and the confirmation are both fine; it is the action itself that
        // cannot be carried out.
        let registry = registry(&scratch("action-registry"), &[("own", &vault)]);
        let host = Host {
            signing: signing(24),
            registry,
            max_concurrency: None,
            actions: vec![Some(Box::new(Failing))],
            served: StdMutex::new(HashMap::new()),
        };

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(server_io, Arc::new(host)));

        let mut channel = connect(client_io, &signing(25), &signing(24).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "own").await;
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
            Arc::new(host(scratch("handshake-registry"), signing(26))),
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
}
