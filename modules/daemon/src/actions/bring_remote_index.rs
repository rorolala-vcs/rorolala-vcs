//! `bring-remote-index`: the chain under a version this index does not hold, brought here whole.
//!
//! A retrack may move the pointer to a version the Vault holds and this side never fetched. What is
//! needed for such a version to be readable here is the chain it is the top of — the version, the
//! variant that produced it, the words they are named by, and so on down to a version this index
//! already holds — and reading it one object per exchange is a connection and a handshake per object.
//!
//! It is one exchange instead: the Workspace sends the hash it wants the chain of and everything it
//! already holds, the Vault walks down until it meets something the Workspace has, and what it found
//! crosses back together, to be written here as one pack.
//!
//! # Input
//!
//! The hash of the version to start from, as hex or named with `blake3:` in front of it. What this
//! Workspace already holds is read here rather than sent by the caller: it is the index's own answer,
//! and a caller that had to build it would be reading the index itself.
//!
//! # Output
//!
//! What became of it, as JSON: the chain was brought and written, the Vault holds an object under that
//! hash that is not a version, the object under a hash the chain needs is not the variant the version
//! above it names, or the Vault holds nothing under a hash the chain needs. It crosses as text for the
//! reason an ownership outcome does — see `request-ownership`.

use std::collections::BTreeSet;
use std::str::FromStr as _;

use rorolala_errors::{BincodeError, Failure as _};
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_storage::{Blake3Hash, Key};
use rorolala_vcs::{VCSIndex, VCSIndexObject, VCSIndexReadingError, VCSWrite};
use serde::{Deserialize, Serialize};

use super::sync_index::local_index;
use super::sync_storage::{Side, carry_blob};

/// An Action that brings the chain under a version from the other end's index.
pub struct ActionBringRemoteIndex;

impl Action for ActionBringRemoteIndex {
    /// Bringing a chain is id 17: what a retrack reads when the version it goes to is not here.
    const ID: u32 = 17;

    type Input = String;
    type Output = String;

    /// Asks the Vault for the chain under the hash, and writes what comes back into this index.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a Vault,
    /// [`ActionError::Codec`] if the hash does not read or a payload will not cross,
    /// [`ActionError::Store`] if the index or the chain could not be read, [`ActionError::Json`] if
    /// the answer will not encode, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_index(&ctx)?;

        // What is wanted, and what this side already holds, is the Workspace's to say: the Vault is
        // told both as one value, so it walks down only as far as it has to.
        let asked = if ctx.is_workspace() {
            let named = input.into_inner().unwrap_or_default();
            let start = key_of(&named)?;
            let held = local
                .object_keys()
                .await
                .map_err(|error| ActionError::Store(error.reason()))?;

            Some(
                (
                    start.hex(),
                    held.iter().map(Key::hex).collect::<Vec<String>>(),
                )
                    .encode()
                    .map_err(codec_failed)?,
            )
        } else {
            None
        };
        let arrived = carry_blob(&mut ctx, Side::Vault, asked.clone()).await?;
        let asked = <(String, Vec<String>)>::decode(arrived.or(asked).unwrap_or_default())
            .map_err(codec_failed)?;

        // The chain is walked on the Vault: it is the side with the index that holds it.
        let found = if ctx.is_vault() {
            let start = key_of(&asked.0)?;
            let held: BTreeSet<Blake3Hash> = asked
                .1
                .iter()
                .filter_map(|hex| Key::from_str(hex.trim()).ok())
                .map(|key| *key.digest())
                .collect();

            Some(chain_under(&local, start, &held).await?)
        } else {
            None
        };
        let mine = found
            .as_ref()
            .map(rorolala_protocol::Encodable::encode)
            .transpose()
            .map_err(codec_failed)?;
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        // The objects are written here, where the index is the Workspace's own, and what the caller
        // is told is what became of the walk rather than the objects themselves.
        let answer = if ctx.is_vault() {
            // UNWRAP: nothing was decoded on the Vault, and the answer it gave was made above.
            let _ = found.unwrap();

            Chain::Brought
        } else {
            Found::decode(bytes)
                .map_err(codec_failed)?
                .write(&local)
                .await?
        };

        serde_json::to_string(&answer).map_err(|error| ActionError::Json(error.into()))
    }
}

/// What became of a chain this index asked the other end for.
#[derive(Debug, Serialize, Deserialize)]
pub enum Chain {
    /// The chain was brought, and is written into this index.
    Brought,
    /// The Vault holds an object under the hash, and it is not the version a chain starts from.
    NotAVersion,
    /// The object under the hash a version names is not the variant of that version.
    NotAVariant(String),
    /// The Vault holds nothing under a hash the chain needs.
    Missing(String),
}

/// What the Vault found while walking a chain.
///
/// It crosses back whole rather than object by object: the walk is one exchange, so what it came to is
/// one value — the objects it brings or the hash it stopped at.
#[derive(Debug, Serialize, Deserialize)]
enum Found {
    /// The objects the chain is made of, as the canonical encoding of each.
    Objects(Vec<Vec<u8>>),
    /// The Vault holds an object under the hash, and it is not the version a chain starts from.
    NotAVersion,
    /// The object under a hash is not the variant the version above it names.
    NotAVariant(String),
    /// The Vault holds nothing under a hash the chain needs.
    Missing(String),
}

impl Found {
    /// Writes the objects this brought into `local` as one pack, and says what became of it.
    ///
    /// What crosses is each object as its canonical encoding, which is what a key names. The objects
    /// become one pack, so a chain of any length is one file and one index here rather than a file per
    /// object, and a reader finds the chain when the pack naming it is there rather than one object at
    /// a time.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::Store`] if an object does not read back or the pack cannot be written.
    async fn write(self, local: &VCSIndex) -> Result<Chain, ActionError> {
        let objects = match self {
            Self::Objects(objects) => objects,
            Self::NotAVersion => return Ok(Chain::NotAVersion),
            Self::NotAVariant(key) => return Ok(Chain::NotAVariant(key)),
            Self::Missing(key) => return Ok(Chain::Missing(key)),
        };

        let mut batch = local.batch();
        for bytes in &objects {
            let object = VCSIndexObject::decode(bytes)
                .map_err(|error| ActionError::Store(error.to_string()))?;

            batch.add(&object);
        }

        batch
            .publish()
            .await
            .map_err(|error| ActionError::Store(error.to_string()))?;

        Ok(Chain::Brought)
    }
}

/// Walks the chain under `start` down to a version `held` already names.
///
/// What is gathered is what a version a reader is moved to is drawn with: the version, the variant
/// that produced it, the words the variant names, and the same for each version below it — as far down
/// as the run has to go, which is the first version this side already holds.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the index cannot be read at all, which is an index that is
/// failing rather than one that does not hold a hash.
async fn chain_under(
    local: &VCSIndex,
    start: Key,
    held: &BTreeSet<Blake3Hash>,
) -> Result<Found, ActionError> {
    let mut going = start;
    let mut objects = Vec::new();

    loop {
        if held.contains(going.digest()) {
            break;
        }

        let Some(object) = object_under(local, &going).await? else {
            return Ok(Found::Missing(going.hex()));
        };
        let VCSIndexObject::Version(version) = object else {
            return Ok(Found::NotAVersion);
        };

        objects.push(VCSWrite::encode(&version));

        // The root is a version pointing at no variant: it is the bottom of every chain, and there is
        // nothing under it to read.
        if version.is_root() {
            break;
        }

        let variant_key = Key::new(*version.variant());
        let Some(object) = object_under(local, &variant_key).await? else {
            return Ok(Found::Missing(variant_key.hex()));
        };
        let VCSIndexObject::Variant(variant) = object else {
            return Ok(Found::NotAVariant(variant_key.hex()));
        };

        if !held.contains(variant_key.digest()) {
            objects.push(VCSWrite::encode(&variant));
        }

        // The words are not what the chain is made of, but a version a reader is moved to is drawn
        // with them: they are brought when this side does not hold them.
        for word in [*variant.creator(), *variant.message()] {
            let word = Key::new(word);
            if held.contains(word.digest()) {
                continue;
            }

            let Some(object) = object_under(local, &word).await? else {
                return Ok(Found::Missing(word.hex()));
            };
            objects.push(VCSWrite::encode(&object));
        }

        going = Key::new(*variant.base_version());
    }

    Ok(Found::Objects(objects))
}

/// The object stored under `key` in `local`, or nothing when the index does not hold one.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the index cannot be read at all, or holds something under the key
/// that does not read back as an object.
async fn object_under(local: &VCSIndex, key: &Key) -> Result<Option<VCSIndexObject>, ActionError> {
    match local.read(*key).await {
        Ok(object) => Ok(Some(object)),
        Err(VCSIndexReadingError::NotFound { .. }) => Ok(None),
        Err(error) => Err(ActionError::Store(error.reason())),
    }
}

/// The key `text` names, as hex or named with `blake3:` in front of it.
///
/// # Errors
///
/// Returns [`ActionError::Codec`] if it does not read as a hash.
fn key_of(text: &str) -> Result<Key, ActionError> {
    Key::from_str(text.trim())
        .map_err(|_| ActionError::Codec(BincodeError::new("the hash does not read".to_owned())))
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(BincodeError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_auth::{Account, KeyAlgorithm, Member, SecureStream, SigningKey};
    use rorolala_protocol::{Action as _, ActionContext, Channel, OnlyWorkspace, Socket};
    use rorolala_storage::{Blake3Hash, Key};
    use rorolala_utils_constants::{VAULT_INDEX_DIR, WORKSPACE_INDEX_DIR};
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_vcs::{
        Creator, Message, ROOT_VERSION, VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version,
    };
    use rorolala_workspace::Workspace;

    use super::{ActionBringRemoteIndex, Chain, Found, chain_under};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-bring-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);

        dir
    }

    /// An index kept in a directory of its own.
    fn index(label: &str) -> VCSIndex {
        VCSIndex::create(scratch(label))
    }

    /// Waits for `future` on a runtime of this test's own.
    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
            .block_on(future)
    }

    /// The objects a chain of two versions is made of, written into `index`.
    fn chain(index: &VCSIndex) -> Written {
        let creator = Creator::try_from("alice".to_owned()).expect("a creator");
        let message = Message::try_from("the first model".to_owned()).expect("a message");
        let root = Version::root();
        let first_variant = Variant::new_bare_variant(
            [1; 32],
            *root.hash().digest(),
            None,
            *creator.hash().digest(),
            *message.hash().digest(),
            ROOT_VERSION,
        );
        let first = first_variant.new_version();
        let second_variant =
            first.new_variant([2; 32], *creator.hash().digest(), *message.hash().digest());
        let second = second_variant.new_version();

        // Written loose: what a chain is read from is the index, however the objects got there.
        for object in [
            VCSIndexObject::Creator(creator.clone()),
            VCSIndexObject::Message(message.clone()),
            VCSIndexObject::Version(root.clone()),
            VCSIndexObject::Version(first.clone()),
            VCSIndexObject::Version(second.clone()),
            VCSIndexObject::Variant(first_variant.clone()),
            VCSIndexObject::Variant(second_variant.clone()),
        ] {
            run(index.write(object)).unwrap();
        }

        Written {
            top: *second.hash().digest(),
            root: *root.hash().digest(),
            first: *first.hash().digest(),
            first_variant: *first_variant.hash().digest(),
            second_variant: *second_variant.hash().digest(),
            creator: *creator.hash().digest(),
            message: *message.hash().digest(),
        }
    }

    /// The keys of a chain, as it was written.
    struct Written {
        /// The version at the top.
        top: Blake3Hash,
        /// The root version at the bottom.
        root: Blake3Hash,
        /// The first version above the root.
        first: Blake3Hash,
        /// The variant that made it.
        first_variant: Blake3Hash,
        /// The variant that made the top version.
        second_variant: Blake3Hash,
        /// The creator the top variant names.
        creator: Blake3Hash,
        /// The message the top variant names.
        message: Blake3Hash,
    }

    /// The keys of the objects the walk brought.
    fn brought(found: Found) -> BTreeSet<Blake3Hash> {
        let Found::Objects(objects) = found else {
            panic!("the walk did not bring objects");
        };

        objects
            .iter()
            .map(|bytes| {
                *VCSIndexObject::decode(bytes)
                    .expect("an object")
                    .hash()
                    .digest()
            })
            .collect()
    }

    #[test]
    fn a_walk_brings_the_chain_down_to_what_the_side_already_holds() {
        let index = index("chain-held");
        let chain = chain(&index);

        // The side holds the root version, so what is brought is everything above it and no more —
        // and a version above it is drawn with its variant and the words it names.
        let held: BTreeSet<Blake3Hash> = BTreeSet::from([chain.root]);
        let found = run(chain_under(&index, Key::new(chain.top), &held)).unwrap();

        assert_eq!(
            brought(found),
            BTreeSet::from([
                chain.top,
                chain.second_variant,
                chain.first,
                chain.first_variant,
                chain.creator,
                chain.message,
            ])
        );
    }

    #[test]
    fn a_walk_to_nothing_held_brings_the_root_and_stops_there() {
        let index = index("chain-nothing");
        let chain = chain(&index);

        let found = run(chain_under(&index, Key::new(chain.top), &BTreeSet::new())).unwrap();

        // The root is a version pointing at no variant, so there is nothing under it to read: the
        // walk stops there rather than failing on a variant that does not exist.
        assert_eq!(
            brought(found),
            BTreeSet::from([
                chain.top,
                chain.second_variant,
                chain.first,
                chain.first_variant,
                chain.root,
                chain.creator,
                chain.message,
            ])
        );
    }

    #[test]
    fn a_walk_that_starts_on_something_held_brings_nothing() {
        let index = index("chain-start-held");
        let chain = chain(&index);

        let held: BTreeSet<Blake3Hash> = BTreeSet::from([chain.top]);
        let found = run(chain_under(&index, Key::new(chain.top), &held)).unwrap();

        assert!(brought(found).is_empty());
    }

    #[test]
    fn a_walk_that_meets_something_else_says_which_way() {
        let index = index("chain-wrong");
        let chain = chain(&index);

        // A hash whose object is not a version: the chain starts at versions, so this is not one.
        let found = run(chain_under(
            &index,
            Key::new(chain.first_variant),
            &BTreeSet::new(),
        ))
        .unwrap();
        assert!(matches!(found, Found::NotAVersion));

        // A version naming something that is not the variant it names.
        let wrong = Version::new_bare_version(chain.message, ROOT_VERSION);
        let key = run(index.write(VCSIndexObject::Version(wrong))).unwrap();
        let found = run(chain_under(&index, key, &BTreeSet::new())).unwrap();
        assert!(
            matches!(found, Found::NotAVariant(named) if named == Key::new(chain.message).hex())
        );

        // A hash nothing is stored under.
        let absent = Key::new([9; 32]);
        let found = run(chain_under(&index, absent, &BTreeSet::new())).unwrap();
        assert!(matches!(found, Found::Missing(named) if named == absent.hex()));
    }

    /// The two ends of one encrypted session over an in-memory stream.
    fn session() -> (Channel, Channel) {
        run(async {
            let server = SigningKey::from_bytes(KeyAlgorithm::Ed25519, [1; 32]).expect("a key");
            let expected = server.public_key();
            let client = SigningKey::from_bytes(KeyAlgorithm::Ed25519, [2; 32]).expect("a key");
            let (client_io, server_io) = tokio::io::duplex(64 * 1024);
            let client_io: Box<dyn Socket> = Box::new(client_io);
            let server_io: Box<dyn Socket> = Box::new(server_io);

            let (client, server) = tokio::join!(
                SecureStream::connect(client_io, &client, &expected),
                SecureStream::accept(server_io, &server),
            );

            (
                client.expect("the client handshakes"),
                server.expect("the server handshakes"),
            )
        })
    }

    #[test]
    fn the_exchange_brings_a_chain_from_one_index_into_the_other() {
        // The Vault's index holds the chain; the Workspace's holds nothing at all, which is the case
        // a retrack meets when it goes to a version the Vault has and this side never fetched.
        let vault_root = scratch("exchange-vault");
        let vault_index = VCSIndex::create(vault_root.join(VAULT_INDEX_DIR));
        let chain = chain(&vault_index);

        let workspace_root = scratch("exchange-workspace");
        std::fs::create_dir_all(workspace_root.join(".rola")).expect("a Workspace's data");
        let workspace = Workspace::locate(&workspace_root).expect("the Workspace");

        let (client, server) = session();
        let vault = Vault::at(&vault_root);
        let workspace_ctx = ActionContext::new_workspace_ctx(Account::default())
            .with_current_workspace(&workspace)
            .with_channel(client);
        let vault_ctx = ActionContext::new_vault_ctx(Member::default())
            .with_current_vault(&vault)
            .with_channel(server);

        // Both ends run the action, as an action does: what one sends the other receives.
        let (said, _) = run(async {
            tokio::join!(
                ActionBringRemoteIndex::process(
                    OnlyWorkspace::from(Some(Key::new(chain.top).hex())),
                    workspace_ctx,
                ),
                ActionBringRemoteIndex::process(OnlyWorkspace::empty(), vault_ctx),
            )
        });

        let said = said.expect("the Workspace side answers");
        let answer: Chain = serde_json::from_str(&said).expect("the answer is JSON");
        assert!(matches!(answer, Chain::Brought), "{said}");

        // What came back is written here: the objects of the chain are in this Workspace's index,
        // which is what makes the version readable on this side from now on.
        let here =
            VCSIndex::at(workspace_root.join(WORKSPACE_INDEX_DIR)).expect("the Workspace's index");
        let held: BTreeSet<Blake3Hash> = run(here.object_keys())
            .expect("the keys")
            .iter()
            .map(|key| *key.digest())
            .collect();

        assert_eq!(
            held,
            BTreeSet::from([
                chain.top,
                chain.second_variant,
                chain.first,
                chain.first_variant,
                chain.root,
                chain.creator,
                chain.message,
            ])
        );
    }
}
