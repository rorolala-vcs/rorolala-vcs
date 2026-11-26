//! `create-remote-entry`: putting a `Uuid` only the Workspace holds into the Vault's Layout.
//!
//! A file enters a Layout by being tracked, and enters the Vault's Layout by this: the name to hold
//! it under is the path it is tracked at, which is the caller's to choose, that path when free,
//! else that path with the `Uuid` in short form, so what a reader sees is where the work holds the
//! file. The holder is the account that asked, which the Vault reads from the session rather than
//! from anything sent.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{batched, create_remote, remote_write};
use super::write_remote_entries::EntryWriteOutcome;

/// An Action that creates an entry of the Vault's Layout.
///
/// # Input
///
/// The name to hold it under, the `Uuid` it is known by, and the version to hold it at. All three
/// belong to the caller and cross to the Vault, which is the side that has a Layout to write.
///
/// # Output
///
/// What became of it: the entry is there, the `Uuid` already was, the path already names something,
/// or something could not be read. [`EntryWriteOutcome`](super::write_remote_entries::EntryWriteOutcome)
/// is what a C caller reads.
pub struct ActionCreateRemoteEntry;

impl Action for ActionCreateRemoteEntry {
    /// Creating an entry is id 14, the first of the two writes a sync makes.
    const ID: u32 = 14;

    /// The path, the `Uuid`, and the version.
    ///
    /// - path: String
    /// - uuid: String
    /// - version: String
    type Input = (String, String, String);
    type Output = EntryWriteOutcome;

    /// Asks the Vault to create the entry, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if an input or an
    /// outcome will not cross, [`ActionError::Store`] if the Vault's Layout could not be written,
    /// and whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcome = remote_write(
            &mut ctx,
            input,
            |layout, (path, uuid, version), me, _admin| {
                batched(layout, |batch| {
                    create_remote(batch, &path, &uuid, &version, "file", me)
                })
            },
        )
        .await?;

        Ok(EntryWriteOutcome::from(outcome))
    }
}

#[cfg(test)]
mod tests {
    use rorolala_auth::Account;
    use rorolala_protocol::{Action as _, ActionContext, OnlyWorkspace};

    use super::ActionCreateRemoteEntry;

    #[tokio::test]
    async fn creating_an_entry_with_no_channel_is_refused() {
        let ctx = ActionContext::new_workspace_ctx(Account::default());
        let input = OnlyWorkspace::from(Some((String::new(), String::new(), String::new())));

        assert!(ActionCreateRemoteEntry::process(input, ctx).await.is_err());
    }
}
