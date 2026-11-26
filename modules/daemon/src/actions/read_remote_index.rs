//! `read-remote-index`: read one index object from the other end of an exchange.
//!
//! The one-object reach of [`list_remote_index`](crate::list_remote_index): a Workspace names a hash,
//! the Vault reads the object under it from its own index, and what comes back is the object and, for
//! a Version, the number its chain traces to. The number is worked out on the Vault because the chain
//! the Version sits in is there, not here, and it crosses beside the object rather than in it, since
//! a Version is written down without its number.

use std::str::FromStr as _;

use rorolala_errors::{BincodeError, Failure as _};
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_vcs::{
    UNKNOWN_VERSION, VCSIndex, VCSIndexObject, VCSIndexReadingError, VCSWrite as _, Version,
};
use serde::{Deserialize, Serialize};

use super::sync_index::local_index;
use super::sync_storage::{Side, carry_blob};

/// An Action that reads one index object from the other end's index.
///
/// # Input
///
/// The hash of the object, as hex or named with `blake3:` in front of it. It is read on the
/// Workspace and crosses to the Vault, which is the side that has an index to read it from.
///
/// # Output
///
/// The object. A Version carries the number its chain traces to, worked out on the Vault, or
/// [`UNKNOWN_VERSION`] when the chain will not trace, since the chain is there and not here.
pub struct ActionReadRemoteIndex;

/// The hash of the index object to read.
///
/// # FFI
///
/// A `RolaDaemonReadRemoteIndexRequest` the caller fills. `hash` is borrowed for the call and
/// released by the caller, and is read as hex or named with `blake3:` in front of it.
#[lazyffi(export = RolaDaemonReadRemoteIndexRequest, input)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadRemoteIndexRequest {
    /// The hash of the object, as hex or named with `blake3:`.
    pub hash: String,
}

impl Action for ActionReadRemoteIndex {
    /// The read is id 9.
    const ID: u32 = 9;

    type Input = ReadRemoteIndexRequest;
    type Output = VCSIndexObject;

    /// Asks the other end for the object under the hash, and answers with what it read.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a Vault,
    /// [`ActionError::MissingObject`] if no object is held under the hash, [`ActionError::Codec`] if
    /// the hash does not read or a payload will not cross, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_index(&ctx)?;

        // The hash is the caller's, so it belongs to the Workspace and crosses to the Vault first.
        let here = input.into_inner().map(|request| request.hash);
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let named = arrived
            .or(to_vault)
            .map_or_default(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        let key = Key::from_str(named.trim()).map_err(|_| {
            ActionError::Codec(BincodeError::new("the hash does not read".to_owned()))
        })?;

        // What is read is read on the Vault: it is the side with the index. What crosses back is the
        // object's bytes and its number, or nothing when no object is held under the hash, and a
        // missing object is answered as nothing rather than as a failure, so both ends reach the one
        // decision after the carrying rather than the Vault leaving the Workspace waiting.
        let held = if ctx.is_vault() {
            read_the_object(&local, &key).await?
        } else {
            None
        };

        // The number is not part of a Version's bytes (it is derived, and written down nowhere) so
        // the object and its number cross together as one payload, and the two ends read the same.
        let mine = if ctx.is_vault() {
            Some(held.encode().map_err(codec_failed)?)
        } else {
            None
        };
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        let payload: Option<(Vec<u8>, Option<u64>)> = if ctx.is_vault() {
            held
        } else {
            Option::decode(bytes).map_err(codec_failed)?
        };
        let Some((object, number)) = payload else {
            return Err(ActionError::MissingObject);
        };

        object_with_number(&object, number)
    }
}

/// The object under `key` in `local` and its number, or nothing when none is held.
///
/// A Version is the only kind whose number is worth carrying: the chain it sits in is here on the
/// Vault, so it is traced here rather than left for a side that cannot reach it. A chain that will
/// not trace leaves the number as nothing, as a listing that could not work one out does.
async fn read_the_object(
    local: &VCSIndex,
    key: &Key,
) -> Result<Option<(Vec<u8>, Option<u64>)>, ActionError> {
    let object = match local.read(*key).await {
        Ok(object) => object,
        Err(VCSIndexReadingError::NotFound { .. }) => return Ok(None),
        Err(error) => return Err(ActionError::Store(error.reason())),
    };

    let number = match &object {
        VCSIndexObject::Version(version) => local.version_num(version).await.ok(),
        _ => None,
    };

    Ok(Some((object.encode(), number)))
}

/// The object the bytes hold, with `number` written into it when it is a Version.
///
/// A Version is read back without its number, it is written down nowhere, so the number that
/// crossed beside it is put back in, which is what a reader is shown. Anything else is itself.
fn object_with_number(bytes: &[u8], number: Option<u64>) -> Result<VCSIndexObject, ActionError> {
    let object =
        VCSIndexObject::decode(bytes).map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(match object {
        VCSIndexObject::Version(version) => VCSIndexObject::Version(Version::new_bare_version(
            *version.variant(),
            number.unwrap_or(UNKNOWN_VERSION),
        )),
        other => other,
    })
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(BincodeError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rorolala_auth::Account;
    use rorolala_protocol::{Action as _, ActionContext, ActionError, OnlyWorkspace};
    use rorolala_storage::Key;
    use rorolala_vcs::{VCSIndex, VCSIndexObject, VCSWrite as _, Version};

    use super::{
        ActionReadRemoteIndex, ReadRemoteIndexRequest, codec_failed, object_with_number,
        read_the_object,
    };

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rola-read-remote-index-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        dir
    }

    #[tokio::test]
    async fn a_hash_with_no_channel_is_refused() {
        let ctx = ActionContext::new_workspace_ctx(Account::default());
        let input = OnlyWorkspace::from(Some(ReadRemoteIndexRequest::default()));

        assert!(ActionReadRemoteIndex::process(input, ctx).await.is_err());
    }

    #[tokio::test]
    async fn a_key_the_index_does_not_hold_is_no_object() {
        let dir = scratch("missing");
        let index = VCSIndex::create(dir.clone());

        assert!(
            read_the_object(&index, &Key::new([1; 32]))
                .await
                .unwrap()
                .is_none()
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_version_is_read_back_with_the_number_that_crossed_beside_it() {
        let version = VCSIndexObject::Version(Version::new_bare_version([1; 32], 0));
        let bytes = version.encode();

        let object = object_with_number(&bytes, Some(9)).unwrap();
        let VCSIndexObject::Version(version) = object else {
            panic!("a version was read");
        };

        assert_eq!(version.version_num(), 9);
    }

    #[test]
    fn bytes_that_are_not_an_object_are_refused() {
        assert!(object_with_number(&[], None).is_err());
        assert!(matches!(codec_failed("bad"), ActionError::Codec(_)));
    }
}

#[cfg(test)]
mod exchange_tests {
    use rorolala_auth::{Account, Member};
    use rorolala_protocol::{Action as _, ActionContext, OnlyWorkspace};
    use rorolala_utils_constants::VAULT_INDEX_DIR;
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_vcs::{VCSIndex, VCSIndexObject, Version};
    use rorolala_workspace::Workspace;

    use crate::test_support::{run, scratch, session};

    use super::{ActionReadRemoteIndex, ReadRemoteIndexRequest};

    /// A version is read from the Vault's index, with the number its chain traces to put back in.
    #[test]
    fn a_version_is_read_from_the_vaults_index() {
        let dir = scratch("read-remote-index");
        let workspace_root = dir.join("workspace");
        Workspace::create(&workspace_root).unwrap();
        let workspace = Workspace::locate(&workspace_root).unwrap();
        let vault_root = dir.join("vault");
        Vault::create(&vault_root).unwrap();
        let vault = Vault::locate(&vault_root).unwrap();

        let index = VCSIndex::create(vault_root.join(VAULT_INDEX_DIR));
        let written = run(index.write(VCSIndexObject::Version(Version::root()))).unwrap();

        let (client, server) = session();
        let workspace_ctx = ActionContext::new_workspace_ctx(Account::default())
            .with_current_workspace(&workspace)
            .with_channel(client);
        let vault_ctx = ActionContext::new_vault_ctx(Member::default())
            .with_current_vault(&vault)
            .with_channel(server);

        let (here, there) = run(async {
            tokio::join!(
                ActionReadRemoteIndex::process(
                    OnlyWorkspace::from(Some(ReadRemoteIndexRequest {
                        hash: written.hex(),
                    })),
                    workspace_ctx
                ),
                ActionReadRemoteIndex::process(
                    OnlyWorkspace::<ReadRemoteIndexRequest>::from(None),
                    vault_ctx
                ),
            )
        });

        assert!(matches!(here.unwrap(), VCSIndexObject::Version(_)));
        assert!(matches!(there.unwrap(), VCSIndexObject::Version(_)));

        let _ = std::fs::remove_dir_all(dir);
    }
}
