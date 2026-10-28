
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
    use rorolala_protocol::{
        Action as _, ActionContext, ActionError, Channel, OnlyWorkspace, Socket,
    };
    use rorolala_utils_location::Locate;
    use rorolala_utils_progress::Progress;
    use rorolala_vault::{KEYS_DIR, RootVault, VAULTS_DIR, Vault};
    use rorolala_workspace::Workspace;
    use tokio::io::{AsyncWriteExt as _, DuplexStream, duplex};
    use tokio::net::TcpListener;

    use super::{
        Host, Registry, Resolution, Served, SessionError, identity_in, member_scopes, serve,
        split_namespace,
    };
    use crate::{
        ActionHandshake, Namespaces, Preference, build_action_registry, proc_action,
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

    /// A Vault named `name`, saying `sections` about itself beside that.
    fn vault_with(dir: &Path, name: &str, sections: &str) {
        Vault::create(dir).unwrap();
        fs::write(
            dir.join(rorolala_vault::CONFIG_PATH),
            format!(
                "[vault_config]\nvault_name = \"{name}\"\nvault_description = \"A test Vault\"\n\n{sections}"
            ),
        )
        .unwrap();
    }

    /// A Vault named `name`, admitting members from its own keys alone.
    fn vault_named(dir: &Path, name: &str) {
        vault_with(dir, name, "[daemon_config]\nkey_discovery = [\"vault\"]\n");
    }

    /// The table naming each of `entries` under the namespace beside it.
    fn registry(dir: &Path, entries: &[(&str, &Path)]) -> Arc<Registry> {
        let file = dir.join("registry.toml");
        let mut namespaces = Namespaces::default();

        for (namespace, path) in entries {
            namespaces.register(namespace, *path).unwrap();
        }

        namespaces.write_to(&file).unwrap();

        Arc::new(Registry {
            path: file,
            served: StdMutex::new(HashMap::new()),
        })
    }

    /// A daemon proving `identity`, with nothing narrowed.
    fn host(identity: SigningKey) -> Host {
        Host {
            signing: identity,
            max_concurrency: None,
            actions: build_action_registry(),
        }
    }

    /// This machine's preferences, with a registration allowed or not.
    fn preference(auto_register: bool) -> Preference {
        let mut preference = Preference::default();
        preference
            .security_mut()
            .set_auto_register_by_ssh(auto_register);

        preference
    }

    /// A connection served for one Vault, which is what a session is.
    fn session(served: Served, preference: Preference) -> Resolution {
        Resolution::Session {
            served: Arc::new(served),
            preference: Arc::new(preference),
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

    /// The greeting `ActionHandshake` produces, once its confirmation has been read.
    ///
    /// It takes the channel because the action is carried over it and not over anything else: a
    /// session that reached the confirmation is one whose channel is the action's from there on.
    async fn handshake_after(mut channel: Channel) -> String {
        let confirmation = wire::read_confirmation(&mut channel).await.unwrap();
        assert_eq!(confirmation, wire::confirmation(ActionHandshake::ID));

        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_channel(channel);

        ActionHandshake::process(OnlyWorkspace::from(Some("world".to_string())), ctx)
            .await
            .unwrap()
    }

    #[test]
    fn a_namespace_is_the_first_segment_and_the_rest_is_a_vault_below_it() {
        assert_eq!(split_namespace(""), ("", ""));
        assert_eq!(split_namespace("/"), ("", ""));
        assert_eq!(split_namespace("alpha"), ("alpha", ""));
        assert_eq!(split_namespace("alpha/"), ("alpha", ""));
        assert_eq!(split_namespace("/alpha"), ("alpha", ""));
        assert_eq!(split_namespace("alpha/beta"), ("alpha", "beta"));
        assert_eq!(
            split_namespace("alpha/vaults/beta/"),
            ("alpha", "vaults/beta")
        );
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
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(1))),
            Resolution::Namespaces(registry),
        ));

        let mut channel = connect(client_io, &signing(2), &server.public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "own").await;

        // The input is a value only the Workspace holds, so it is where the action starts; both
        // sides end up holding what comes back.
        let output = handshake_after(channel).await;
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
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(3))),
            Resolution::Namespaces(registry),
        ));

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
        let daemon = host(signing(31));

        for sub in ["", "nowhere"] {
            let (client_io, server_io) = duplex(64 * 1024);
            let serving = tokio::spawn(serve(
                server_io,
                Arc::new(host(signing(31))),
                Resolution::Namespaces(Arc::clone(&registry)),
            ));

            let mut channel = connect(client_io, &signing(32), &signing(31).public_key()).await;
            request_sub(&mut channel, ActionHandshake::ID, "client", sub).await;

            let error = serving.await.unwrap().unwrap_err();
            assert!(matches!(error, SessionError::UnknownVault(name) if name == sub));
            assert!(wire::read_confirmation(&mut channel).await.is_err());
        }

        // The table is still the one it was read from, and what it holds is unchanged.
        assert!(registry.serve_for(&daemon, "own").await.is_some());
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

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(33))),
            Resolution::Namespaces(registry),
        ));

        let mut channel = connect(client_io, &signing(34), &signing(33).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "root/alpha").await;

        // The greeting is named after the Vault the request reached, which is the one under the
        // namespace rather than the one the namespace names.
        let output = handshake_after(channel).await;
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
        let daemon = host(signing(35));

        // Nothing, `.`, and the path the namespace names all name the Vault the namespace
        // registered itself.
        for own in ["ns", "ns/.", "ns/"] {
            assert_eq!(
                registry
                    .serve_for(&daemon, own)
                    .await
                    .unwrap()
                    .vault
                    .get_root(),
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
                registry
                    .serve_for(&daemon, named)
                    .await
                    .unwrap()
                    .vault
                    .get_root(),
                alpha.as_path(),
                "{named}"
            );
        }

        // A name the root holds nothing under names no Vault.
        assert!(registry.serve_for(&daemon, "ns/nowhere").await.is_none());
    }

    #[tokio::test]
    async fn a_workspace_that_names_someone_else_is_refused() {
        let dir = scratch("mismatch");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        // The Vault knows "client" as one key, but the peer proves another.
        publish(
            &vault.join(KEYS_DIR),
            "client",
            &Ed25519SigningKey::from_bytes(&[6; 32]),
        );

        let registry = registry(&scratch("mismatch-registry"), &[("own", &vault)]);

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(5))),
            Resolution::Namespaces(registry),
        ));

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
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(9))),
            Resolution::Namespaces(registry),
        ));

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
        let host = Arc::new(host(signing(11)));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let serving = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            serve(stream, host, Resolution::Namespaces(registry)).await
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

    #[tokio::test]
    async fn a_session_serves_the_vault_it_was_started_for() {
        let dir = scratch("session");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let client = Ed25519SigningKey::from_bytes(&[40; 32]);
        publish(&vault.join(KEYS_DIR), "client", &client);

        let served = Served::from_path(&vault, None).unwrap();

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(41))),
            session(served, preference(false)),
        ));

        // A session's request names no Vault: the path it was started with already did.
        let mut channel = connect(client_io, &signing(40), &signing(41).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "").await;

        let output = handshake_after(channel).await;
        assert_eq!(output, "Hello, world, I'm Own Vault.\n\nA test Vault");
        serving.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn a_session_answers_only_for_the_vault_it_was_started_for() {
        let dir = scratch("session-other");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        let served = Served::from_path(&vault, None).unwrap();

        for sub in ["other", "own", "vaults/alpha"] {
            let (client_io, server_io) = duplex(64 * 1024);
            let serving = tokio::spawn(serve(
                server_io,
                Arc::new(host(signing(44))),
                session(Served::from_path(&vault, None).unwrap(), preference(false)),
            ));

            let mut channel = connect(client_io, &signing(45), &signing(44).public_key()).await;
            request_sub(&mut channel, ActionHandshake::ID, "client", sub).await;

            let error = serving.await.unwrap().unwrap_err();
            assert!(matches!(error, SessionError::UnknownVault(name) if name == sub));
        }

        // What was resolved is still the Vault the path named.
        assert_eq!(served.vault.get_root(), vault.as_path());
    }

    #[test]
    fn only_a_vaults_own_root_is_what_a_session_serves() {
        let dir = scratch("from-path");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        fs::create_dir_all(vault.join("nested")).unwrap();

        // What is served is the Vault the path names, and a directory that merely lies inside one
        // is not that: a session's path is exact rather than searched upward for.
        assert!(Served::from_path(&vault, None).is_some());
        assert!(Served::from_path(&vault.join("nested"), None).is_none());
        assert!(Served::from_path(&scratch("not-a-vault"), None).is_none());
    }

    #[tokio::test]
    async fn a_session_registers_a_name_it_was_allowed_to_register() {
        let dir = scratch("register");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(43))),
            session(
                Served::from_path(&vault, None).unwrap(),
                preference(true),
            ),
        ));

        let mut channel = connect(client_io, &signing(42), &signing(43).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "").await;

        // The name is registered before the confirmation is written, so a session that is told it
        // may go ahead is one whose registration is already on disk.
        let output = handshake_after(channel).await;
        assert_eq!(output, "Hello, world, I'm Own Vault.\n\nA test Vault");
        serving.await.unwrap().unwrap();

        let written = fs::read_to_string(vault.join(KEYS_DIR).join("client.pub")).unwrap();
        assert_eq!(
            PublicKey::from_pem(&written).unwrap(),
            signing(42).public_key()
        );
    }

    #[tokio::test]
    async fn a_session_registers_nothing_when_it_was_not_allowed_to() {
        let dir = scratch("register-off");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(46))),
            session(
                Served::from_path(&vault, None).unwrap(),
                preference(false),
            ),
        ));

        let mut channel = connect(client_io, &signing(47), &signing(46).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::UnknownMember(name) if name == "client"));
        assert!(!vault.join(KEYS_DIR).join("client.pub").exists());
    }

    #[tokio::test]
    async fn a_registration_never_takes_a_name_that_holds_another_key() {
        let dir = scratch("register-taken");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");
        // Somebody else already holds the name, with a key the peer does not have.
        publish(
            &vault.join(KEYS_DIR),
            "client",
            &Ed25519SigningKey::from_bytes(&[48; 32]),
        );
        let before = fs::read_to_string(vault.join(KEYS_DIR).join("client.pub")).unwrap();

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(49))),
            session(
                Served::from_path(&vault, None).unwrap(),
                preference(true),
            ),
        ));

        let mut channel = connect(client_io, &signing(50), &signing(49).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::IdentityMismatch(name) if name == "client"));

        // What was there is still what is there.
        assert_eq!(
            fs::read_to_string(vault.join(KEYS_DIR).join("client.pub")).unwrap(),
            before
        );
        assert!(!vault.join(KEYS_DIR).join("client.pub.1").exists());
    }

    #[tokio::test]
    async fn two_first_connections_for_one_name_both_get_in() {
        let dir = scratch("register-race");
        let vault = dir.join("own");
        vault_named(&vault, "Own Vault");

        // Both connections prove the same key, which is what a caller retrying a failed first
        // connection does: what is registered is the key either of them proved.
        let mut openings = Vec::new();

        for _ in 0..2 {
            let (client_io, server_io) = duplex(64 * 1024);
            let serving = tokio::spawn(serve(
                server_io,
                Arc::new(host(signing(51))),
                session(
                    Served::from_path(&vault, None).unwrap(),
                    preference(true),
                ),
            ));

            let mut channel = connect(client_io, &signing(52), &signing(51).public_key()).await;
            request_sub(&mut channel, ActionHandshake::ID, "client", "").await;

            openings.push((serving, channel));
        }

        for (serving, channel) in openings {
            let output = handshake_after(channel).await;
            assert_eq!(output, "Hello, world, I'm Own Vault.\n\nA test Vault");
            serving.await.unwrap().unwrap();
        }

        // One file, one key: the second registration found the first one's and agreed with it.
        let written = fs::read_to_string(vault.join(KEYS_DIR).join("client.pub")).unwrap();
        assert_eq!(
            PublicKey::from_pem(&written).unwrap(),
            signing(52).public_key()
        );
    }

    #[tokio::test]
    async fn a_name_the_vault_does_not_look_for_cannot_be_registered() {
        let dir = scratch("register-undiscovered");
        let vault = dir.join("own");
        // The Vault looks for its members in the user's own store, so a key written into its own
        // keys is one nothing would find: saying it was registered would be a lie.
        vault_with(&vault, "Own Vault", "[daemon_config]\nkey_discovery = [\"user\"]\n");

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(53))),
            session(
                Served::from_path(&vault, None).unwrap(),
                preference(true),
            ),
        ));

        let mut channel = connect(client_io, &signing(54), &signing(53).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "").await;

        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::RegisterNotDiscovered(name) if name == "client"));
        assert!(!vault.join(KEYS_DIR).join("client.pub").exists());
    }

    #[tokio::test]
    async fn an_administrators_name_is_never_registered_by_a_connection() {
        let dir = scratch("register-admin");
        let vault = dir.join("own");
        vault_with(
            &vault,
            "Own Vault",
            "[daemon_config]\nkey_discovery = [\"vault\"]\n\n[auth]\nadmins = [\"client\"]\n",
        );

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(55))),
            session(
                Served::from_path(&vault, None).unwrap(),
                preference(true),
            ),
        ));

        let mut channel = connect(client_io, &signing(56), &signing(55).public_key()).await;
        request_sub(&mut channel, ActionHandshake::ID, "client", "").await;

        // An administrator may move any file in the Vault, so a name that may be one is not a
        // name a connection may claim.
        let error = serving.await.unwrap().unwrap_err();
        assert!(matches!(error, SessionError::RegisterAdmin(name) if name == "client"));
        assert!(!vault.join(KEYS_DIR).join("client.pub").exists());
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
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(20))),
            Resolution::Namespaces(registry),
        ));

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
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host(signing(22))),
            Resolution::Namespaces(registry),
        ));

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
            max_concurrency: None,
            actions: vec![Some(Box::new(Failing))],
        };

        let (client_io, server_io) = duplex(64 * 1024);
        let serving = tokio::spawn(serve(
            server_io,
            Arc::new(host),
            Resolution::Namespaces(registry),
        ));

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
            Arc::new(host(signing(26))),
            Resolution::Namespaces(registry(
                &scratch("handshake-registry"),
                &[],
            )),
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

        // A name that is not a member says what may be done about it, since that is the one
        // failure a caller can act on from where it is sitting.
        let refusal = SessionError::UnknownMember("alice".to_string()).to_string();
        assert!(refusal.starts_with("no member is named alice"), "{refusal}");
        assert!(refusal.contains("auto-register-by-ssh"), "{refusal}");

        // A registration that was refused says which rule refused it.
        let admin = SessionError::RegisterAdmin("alice".to_string()).to_string();
        assert!(admin.contains("administrator"), "{admin}");

        let undiscovered =
            SessionError::RegisterNotDiscovered("alice".to_string()).to_string();
        assert!(undiscovered.contains("key_discovery"), "{undiscovered}");

        let unwritten = SessionError::Register {
            path: PathBuf::from("/srv/keys/alice.pub"),
            reason: "access is denied".to_string(),
        }
        .to_string();
        assert!(unwritten.contains("/srv/keys/alice.pub"), "{unwritten}");
        assert!(unwritten.contains("access is denied"), "{unwritten}");
    }
}
