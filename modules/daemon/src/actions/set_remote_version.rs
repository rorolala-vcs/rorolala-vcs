//! `set-remote-version`: moving the version the Vault's Layout holds for a `Uuid` on.
//!
//! A version goes up only from the side that holds the entry, so which account is asking decides
//! whether it is written at all, and an administrator of the Vault may write one it does not
//! hold. What a version change touches is the version and nothing else: the path a reader sees and
//! the holder stay as they were.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};

use super::layout_remote::{batched, remote_write, set_remote};
use super::write_remote_entries::EntryWriteOutcome;

/// An Action that sets the version an entry of the Vault's Layout is at.
///
/// # Input
///
/// The `Uuid` to set, and the version to set it at. Both belong to the caller and cross to the
/// Vault, which is the side that has a Layout to write.
///
/// # Output
///
/// What became of it: the version is set, the Vault holds no such entry, the account may not write
/// it, or something could not be read. [`EntryWriteOutcome`](super::write_remote_entries::EntryWriteOutcome)
/// is what a C caller reads.
pub struct ActionSetRemoteVersion;

impl Action for ActionSetRemoteVersion {
    /// Setting a version is id 15, the second of the two writes a sync makes.
    const ID: u32 = 15;

    /// The `Uuid` and the version.
    ///
    /// - uuid: String
    /// - version: String
    type Input = (String, String);
    type Output = EntryWriteOutcome;

    /// Asks the Vault to set the version, and answers with what it said.
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
        let outcome = remote_write(&mut ctx, input, |layout, (uuid, version), me, admin| {
            batched(layout, |batch| {
                set_remote(batch, &uuid, &version, me, admin)
            })
        })
        .await?;

        Ok(EntryWriteOutcome::from(outcome))
    }
}

#[cfg(test)]
mod tests {
    use rorolala_auth::Account;
    use rorolala_protocol::{Action as _, ActionContext, OnlyWorkspace};

    use super::ActionSetRemoteVersion;

    #[tokio::test]
    async fn setting_a_version_with_no_channel_is_refused() {
        let ctx = ActionContext::new_workspace_ctx(Account::default());
        let input = OnlyWorkspace::from(Some((String::new(), String::new())));

        assert!(ActionSetRemoteVersion::process(input, ctx).await.is_err());
    }
}

#[cfg(test)]
mod exchange_tests {
    use rorolala_auth::Account;
    use rorolala_layout::MutableData;
    use rorolala_protocol::{Action as _, ActionContext, OnlyWorkspace};
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use uuid::Uuid;

    use crate::test_support::{member, run, scratch, session};

    use super::ActionSetRemoteVersion;
    use crate::EntryWriteKind;

    /// The holder of an entry sets its version, and the answer says the write was made.
    #[test]
    fn a_version_is_set_on_the_vault_by_the_account_that_holds_the_entry() {
        let dir = scratch("set-remote-version");
        let vault_root = dir.join("vault");
        Vault::create(&vault_root).unwrap();
        let vault = Vault::locate(&vault_root).unwrap();
        let layout = vault.layout().unwrap();
        let id = Uuid::from_u128(1);
        layout
            .create_entry(
                id,
                MutableData::new(Some("alice".to_owned()), [1; 32], String::new()),
            )
            .unwrap();

        let version = "11".repeat(32);
        let input = (id.to_string(), version);
        let (client, server) = session();

        let workspace_ctx =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(client);
        let vault_ctx = ActionContext::new_vault_ctx(member(
            &vault_root.join(rorolala_vault::KEYS_DIR),
            "alice",
        ))
        .with_current_vault(&vault)
        .with_channel(server);

        let (here, there) = run(async {
            tokio::join!(
                ActionSetRemoteVersion::process(
                    OnlyWorkspace::from(Some(input.clone())),
                    workspace_ctx
                ),
                ActionSetRemoteVersion::process(
                    OnlyWorkspace::<(String, String)>::from(None),
                    vault_ctx
                ),
            )
        });

        assert_eq!(here.unwrap().kind(), EntryWriteKind::Written);
        assert_eq!(there.unwrap().kind(), EntryWriteKind::Written);

        let _ = std::fs::remove_dir_all(dir);
    }
}
