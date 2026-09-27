//! `ls-remote`: what the store on the other end of an exchange holds.
//!
//! The listing half of an exchange, where [`sync_all`](crate::sync_all) is the carrying half: a
//! Workspace asks, the Vault answers with what its own store holds, and nothing but the listing
//! moves. Which listing is asked for is the caller's word, and the two the store has are the two
//! it can be told to give — the objects it holds, and the contents it keeps cut.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use rorolala_storage::{Key, RorolalaStorage, StorageBackend as _};

use super::sync_all::local_store;
use super::sync_storage::{Side, carry_blob};

/// An Action that lists what the store on the other end holds.
///
/// # Input
///
/// `"manifests"` for the contents the store keeps cut, and anything else for the objects it holds.
/// The word is read on the Workspace and crosses to the Vault, which is the side that has a store
/// of its own to answer with.
///
/// # Output
///
/// The keys, as a listing prints them — each as hex, one a line. It is the Workspace's answer that
/// is read, the Vault's listing handed back, since the Workspace is where the caller is. The listing
/// travels as one string rather than a run of keys because that is what crosses the C ABI whole; a
/// caller that wants keys apart reads it the way it was written, a line at a time.
pub struct ActionListRemote;

impl Action for ActionListRemote {
    /// The listing is id 6.
    const ID: u32 = 6;

    type Input = String;
    type Output = String;

    /// Asks the other end what it holds, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, [`ActionError::Store`] if the other end's store would not list, and whatever the
    /// exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_store(&ctx)?;

        // What listing is wanted is the caller's word, so it belongs to the Workspace and crosses to
        // the Vault first — the Vault is the side with a store of its own to answer with.
        let here = input.into_inner();
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let which = arrived
            .or(to_vault)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();

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

/// The keys `which` names, from `local`, written as the hex a listing prints.
async fn listed(local: &RorolalaStorage, which: &str) -> Result<String, ActionError> {
    let listed = if which.trim() == "manifests" {
        local.list_manifest_keys().await
    } else {
        local.list_exist_keys().await
    };
    let keys = listed.map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(keys.iter().map(Key::hex).collect::<Vec<_>>().join("\n"))
}
