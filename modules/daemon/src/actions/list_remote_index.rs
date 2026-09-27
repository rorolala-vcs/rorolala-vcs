//! `ls-remote-index`: what the index on the other end of an exchange holds.
//!
//! The listing half of an index exchange, where [`sync_index`](crate::sync_index) is the carrying
//! half: a Workspace asks, the Vault answers with what its own index holds, and nothing but the
//! listing moves. Which listing is asked for is the caller's word — the variants, the versions, or
//! the text objects — and what is answered is the hashes, one a line, so a hash read here is one to
//! hand to [`read_remote_index`](crate::read_remote_index).

use rorolala_errors::Failure as _;
use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use rorolala_vcs::{VCSIndex, VCSIndexObject};

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
/// The hashes, as a listing prints them — each as hex, one a line. It is the Workspace's answer that
/// is read, the Vault's listing handed back, since the Workspace is where the caller is. The listing
/// travels as one string rather than a run of keys because that is what crosses the C ABI whole; a
/// caller that wants hashes apart reads it the way it was written, a line at a time.
pub struct ActionListRemoteIndex;

impl Action for ActionListRemoteIndex {
    /// The listing is id 8.
    const ID: u32 = 8;

    type Input = String;
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
        // the Vault first — the Vault is the side with an index of its own to answer with.
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

/// The hashes `which` names, from `local`, written as the hex a listing prints.
async fn listed(local: &VCSIndex, which: &str) -> Result<String, ActionError> {
    let objects = local
        .read_objects()
        .await
        .map_err(|error| ActionError::Store(error.reason()))?;

    let hashes: Vec<String> = objects
        .into_iter()
        .filter(|(_, object)| wanted(which, object))
        .map(|(key, _)| key.hex())
        .collect();

    Ok(hashes.join("\n"))
}

/// Whether `object` is one of the kinds `which` names.
///
/// A word that names no kind names every one, which is what a caller that asked for nothing in
/// particular is answered with.
fn wanted(which: &str, object: &VCSIndexObject) -> bool {
    match which.trim() {
        "variants" => matches!(object, VCSIndexObject::Variant(_)),
        "versions" => matches!(object, VCSIndexObject::Version(_)),
        "str" => matches!(
            object,
            VCSIndexObject::Creator(_) | VCSIndexObject::Message(_)
        ),
        _ => true,
    }
}
