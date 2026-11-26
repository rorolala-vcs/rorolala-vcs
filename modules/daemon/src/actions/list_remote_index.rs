//! `ls-remote-index`: what the index on the other end of an exchange holds.
//!
//! The listing half of an index exchange, where [`sync_index`](crate::sync_index) is the carrying
//! half: a Workspace asks, the Vault answers with what its own index holds, and nothing but the
//! listing moves. Which listing is asked for is the caller's word, the variants, the versions, or
//! the text objects, and what is answered is the hashes, one a line, so a hash read here is one to
//! hand to [`read_remote_index`](crate::read_remote_index).

use rorolala_errors::Failure as _;
use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_vcs::{VCSIndex, VCSIndexKind};
use serde::{Deserialize, Serialize};

use super::sync_index::local_index;
use super::sync_storage::{Side, carry_blob};

/// An Action that lists what the index on the other end holds.
///
/// # Input
///
/// `"variants"` for the variants, `"versions"` for the versions, `"str"` for the Creator and Message
/// objects, and anything else for every object. The word is read on the Workspace and crosses to the
/// Vault, which is the side that has an index of its own to answer with.
///
/// # Output
///
/// The hashes, as a listing prints them, each as hex, one a line. It is the Workspace's answer that
/// is read, the Vault's listing handed back, since the Workspace is where the caller is. The listing
/// travels as one string rather than a run of keys because that is what crosses the C ABI whole; a
/// caller that wants hashes apart reads it the way it was written, a line at a time.
pub struct ActionListRemoteIndex;

/// The word that names the index listing wanted.
///
/// # FFI
///
/// A `RolaDaemonListRemoteIndexRequest` the caller fills. `which` is borrowed for the call and
/// released by the caller; `variants`, `versions` and `str` name one kind of object, and any other
/// word, empty included, asks for every object.
#[lazyffi(export = RolaDaemonListRemoteIndexRequest, input)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListRemoteIndexRequest {
    /// Which listing is wanted, or empty for every object.
    pub which: String,
}

impl Action for ActionListRemoteIndex {
    /// The listing is id 8.
    const ID: u32 = 8;

    type Input = ListRemoteIndexRequest;
    type Output = String;

    /// Asks the other end what its index holds, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a Vault,
    /// [`ActionError::Store`] if the other end's index would not list, and whatever the exchange
    /// fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_index(&ctx)?;

        // What listing is wanted is the caller's word, so it belongs to the Workspace and crosses to
        // the Vault first, the Vault is the side with an index of its own to answer with.
        let here = input.into_inner().map(|request| request.which);
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let which = arrived
            .or(to_vault)
            .map_or_default(|bytes| String::from_utf8_lossy(&bytes).into_owned());

        // What the Vault holds crosses back, and the Workspace is where it is read: the same call
        // reads both ways, so the side that sent it keeps its own.
        let mine = if ctx.is_vault() {
            Some(listed(&local, &which).await?.into_bytes())
        } else {
            None
        };
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// The hashes `which` names, from `local`, written as the hex a listing prints.
///
/// What a listing is made of is the index's keys, and the keys alone are the answer unless a kind was
/// asked for: every object read whole to be filtered is a read, a decode and a hash check apiece for
/// an answer the keys already gave. When a kind was asked for, the one byte behind each object's frame
/// is what tells it apart, see [`VCSIndex::kind_of`].
async fn listed(local: &VCSIndex, which: &str) -> Result<String, ActionError> {
    let keys = local
        .object_keys()
        .await
        .map_err(|error| ActionError::Store(error.reason()))?;

    let mut hashes = Vec::with_capacity(keys.len());
    for key in keys {
        if !asks_for_kind(which) {
            hashes.push(key.hex());
            continue;
        }

        let kind = local
            .kind_of(&key)
            .await
            .map_err(|error| ActionError::Store(error.to_string()))?;
        if wanted(which, kind) {
            hashes.push(key.hex());
        }
    }

    Ok(hashes.join("\n"))
}

/// Whether the word `which` asks for a kind rather than for every object.
///
/// A word that names no kind names every one, which is what a caller that asked for nothing in
/// particular is answered with, and it is what lets a listing answer without reading an object.
fn asks_for_kind(which: &str) -> bool {
    matches!(which.trim(), "variants" | "versions" | "str")
}

/// Whether the object of kind `kind` is one of the kinds `which` names.
fn wanted(which: &str, kind: Option<VCSIndexKind>) -> bool {
    match which.trim() {
        "variants" => kind == Some(VCSIndexKind::Variant),
        "versions" => kind == Some(VCSIndexKind::Version),
        "str" => matches!(kind, Some(VCSIndexKind::Creator | VCSIndexKind::Message)),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use rorolala_auth::Account;
    use rorolala_protocol::{Action as _, ActionContext, OnlyWorkspace};
    use rorolala_vcs::{VCSIndex, VCSIndexKind};

    use super::{ActionListRemoteIndex, ListRemoteIndexRequest, asks_for_kind, listed, wanted};

    #[test]
    fn a_listing_word_names_a_kind_or_every_object() {
        assert!(asks_for_kind(" variants "));
        assert!(asks_for_kind("versions"));
        assert!(asks_for_kind("str"));
        assert!(!asks_for_kind(""));

        assert!(wanted("variants", Some(VCSIndexKind::Variant)));
        assert!(!wanted("variants", Some(VCSIndexKind::Version)));
        assert!(wanted("versions", Some(VCSIndexKind::Version)));
        assert!(wanted("str", Some(VCSIndexKind::Creator)));
        assert!(wanted("str", Some(VCSIndexKind::Message)));
        assert!(!wanted("str", Some(VCSIndexKind::Variant)));
        assert!(wanted("", None));
    }

    #[tokio::test]
    async fn an_empty_index_lists_nothing() {
        let dir =
            std::env::temp_dir().join(format!("rola-list-remote-index-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let index = VCSIndex::create(dir.clone());

        assert_eq!(listed(&index, "variants").await.unwrap(), "");
        assert_eq!(listed(&index, "").await.unwrap(), "");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_listing_with_no_channel_is_refused() {
        let ctx = ActionContext::new_workspace_ctx(Account::default());
        let input = OnlyWorkspace::from(Some(ListRemoteIndexRequest::default()));

        assert!(ActionListRemoteIndex::process(input, ctx).await.is_err());
    }
}
