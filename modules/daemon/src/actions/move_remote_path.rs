//! `move-remote-path`: moving a path in the Vault's Layout.
//!
//! Locating a file and deprecating one are both a move of its path in the Vault's Layout: it is
//! moved out of `@/new/` to where it belongs, or under `@/removed/` so it reads as deprecated.
//! Either is the holder's to do, or an administrator's, and what the Vault answers says which of
//! those happened — so the Workspace can change the copy it keeps to agree with it.

use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};

use super::layout_remote::{PathMove, codec_failed, local_layout, move_remote};
use super::sync_storage::{Side, carry_blob};

/// An Action that moves a path in the Vault's Layout.
///
/// # Input
///
/// The two paths, as the Vault's Layout writes them: what is at the first is to be at the second.
/// They belong to the caller, so they are read on the Workspace and cross to the Vault, which is
/// the side that has a Layout to move them in.
///
/// # Output
///
/// What became of it, as JSON: the entry is now at the path it was moved to, nothing was at the
/// path it came from, something is already at the path it was going to, the account may not move
/// it, or a path is not one a remote Layout may name. It crosses as text for the reason an
/// ownership outcome does — see `request-ownership`.
pub struct ActionMoveRemotePath;

impl Action for ActionMoveRemotePath {
    /// Moving a path is id 13: what locates a file and what deprecates one.
    const ID: u32 = 13;

    /// The path to move from, and the path to move it to.
    ///
    /// - from: String
    /// - to: String
    type Input = (String, String);
    type Output = String;

    /// Asks the Vault to move the path, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if a path or an
    /// outcome will not cross, [`ActionError::Store`] if the Vault's Layout could not be written,
    /// [`ActionError::Json`] if the outcome will not encode, and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // The paths are the caller's, so they are read on the Workspace and cross to the Vault.
        let here = input.into_inner();
        let to_vault = here
            .as_ref()
            .map(rorolala_protocol::Encodable::encode)
            .transpose()
            .map_err(codec_failed)?;
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let bytes = arrived.or(to_vault).unwrap_or_default();
        let (from, to) = <(String, String)>::decode(bytes).map_err(codec_failed)?;

        // Whether the account administrates the Vault is the Vault's own configuration's to say,
        // and which account it is the session's to say: neither is taken from the caller.
        let outcome = if ctx.is_vault() {
            let me = ctx
                .get_member()
                .into_inner()
                .map(|member| member.name())
                .ok_or(ActionError::MissingValue)?;
            let admin = ctx
                .current_vault_config()
                .into_inner()
                .is_some_and(|config| config.auth_config().is_admin(&me));

            Some(move_remote(&local_layout(&ctx)?, &from, &to, &me, admin)?)
        } else {
            None
        };

        let mine = outcome
            .as_ref()
            .map(rorolala_protocol::Encodable::encode)
            .transpose()
            .map_err(codec_failed)?;
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        let outcome = if ctx.is_vault() {
            // UNWRAP: the outcome above was made on exactly this side.
            outcome.unwrap()
        } else {
            PathMove::decode(bytes).map_err(codec_failed)?
        };

        serde_json::to_string(&outcome).map_err(|error| ActionError::Json(error.into()))
    }
}
