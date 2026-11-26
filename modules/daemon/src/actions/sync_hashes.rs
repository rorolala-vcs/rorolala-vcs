//! `sync-hashes`: making the two stores hold the same named keys.
//!
//! Where [`sync_all`](crate::sync_all) works out what to sync, everything either side holds, this
//! is told: the keys are the caller's, and the carrying both sides do afterwards is the very
//! primitive a full sync uses. So a full sync is this with everything named, and this is a full sync
//! narrowed to a list.

use std::str::FromStr as _;

use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

use super::sync_all::local_stores;
use super::sync_storage::{
    DEFAULT_WINDOW, Side, Unit, UnitKind, carry_blob, grant, split_ask, sync_units,
};

/// An Action that makes the two stores hold the units it is given, and answers how many reads it was
/// granted at once.
///
/// # Input
///
/// The units, separated by whitespace, one a line, as a listing prints them. `m:<key>` carries only
/// the manifest a content was cut into, `o:<key>` only the object, `q:<key>` whichever of the two
/// the sender holds, and a bare key the whole of what it names; a hash is read the way a key is
/// anywhere else. A token `c:<n>` asks the Vault to read `n` at once where the asking happens, so a
/// caller that wants to know what it is granted sends `c:<n>` and nothing else and reads the answer.
///
/// # Output
///
/// How many reads at once the Vault will serve, as decimal text: what the caller asked for, held to
/// what the Vault is set to serve. A caller that asked for none is granted what this primitive reads
/// at once by default.
pub struct ActionSyncHashes;

/// The units a named sync carries, and the ask for how much is read at once.
///
/// # FFI
///
/// A `RolaDaemonSyncHashesRequest` the caller fills. `units` is borrowed for the call and released
/// by the caller; it holds the units separated by whitespace, one a line as a listing prints them.
#[lazyffi(export = RolaDaemonSyncHashesRequest, input)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncHashesRequest {
    /// The units to carry, and the `c:<n>` ask, as a listing prints them.
    pub units: String,
}

impl Action for ActionSyncHashes {
    /// The named sync is id 7.
    const ID: u32 = 7;

    type Input = SyncHashesRequest;
    type Output = String;

    /// Has the two stores carry the named units, with as many reads at once as the Vault serves.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, and whatever the exchange fails with, see the sync primitive in
    /// [`sync_storage`](crate::sync_storage).
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_stores(&ctx)?;

        // The list is the caller's, so it belongs to the Workspace and crosses to the Vault first:
        // both sides must name the same units, or the carrying that follows would say two different
        // things on the two ends.
        let here = input.into_inner().map(|request| request.units);
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let text = arrived
            .or(to_vault)
            .map_or_default(|bytes| String::from_utf8_lossy(&bytes).into_owned());

        // How much the caller wants read at once, and how much the Vault will serve: the ask crosses,
        // the answer comes back, and the answer is what the caller is told. Both ends read the same
        // text and take the ask out of it, so they name the same units.
        let (asked, units) = split_ask(&text);
        let granted = grant(&mut ctx, asked).await?;
        let window = usize::try_from(granted).unwrap_or(DEFAULT_WINDOW);

        let required: Vec<Unit> = units.split_whitespace().filter_map(unit_of).collect();

        // Each side names the store it has; the other wrapper is empty and the other side fills it,
        // exactly as a full sync hands them in.
        let workspace = OnlyWorkspace::new(&ctx, || local.clone());
        let vault = OnlyVault::new(&ctx, || local.clone());
        sync_units(&mut ctx, workspace, vault, Both::new(required), window).await?;

        Ok(granted.to_string())
    }
}

/// The unit a token names.
///
/// `m:<key>` carries only the manifest a content was cut into, `o:<key>` only the object, and
/// `q:<key>` whichever of the two the other side holds, what a caller that does not know how a
/// content is kept asks for, learning from what arrives. A bare key carries the whole of what it
/// names, which is what a caller that has no manifest of its own asks for and what the two ends
/// expand. A token that names no key is passed over, as a listing's noise always has been.
fn unit_of(token: &str) -> Option<Unit> {
    let (kind, spelling) = token
        .strip_prefix("m:")
        .map(|rest| (UnitKind::Manifest, rest))
        .or_else(|| {
            token
                .strip_prefix("o:")
                .map(|rest| (UnitKind::Object, rest))
        })
        .or_else(|| {
            token
                .strip_prefix("q:")
                .map(|rest| (UnitKind::Discover, rest))
        })
        .unwrap_or((UnitKind::Whole, token));

    Key::from_str(spelling).ok().map(|key| Unit { key, kind })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use rorolala_storage::Key;

    use super::{UnitKind, unit_of};

    /// A token names a key and how much of it is carried: `m:` a manifest, `o:` an object, a bare
    /// key the whole.
    #[test]
    fn a_token_names_a_key_and_how_much_of_it() {
        let hex = "0f".repeat(32);
        let key = Key::from_str(&hex).unwrap();

        assert_eq!(
            unit_of(&hex).map(|unit| (unit.key, unit.kind)),
            Some((key, UnitKind::Whole))
        );
        assert_eq!(
            unit_of(&format!("m:{hex}")).map(|unit| (unit.key, unit.kind)),
            Some((key, UnitKind::Manifest))
        );
        assert_eq!(
            unit_of(&format!("o:{hex}")).map(|unit| (unit.key, unit.kind)),
            Some((key, UnitKind::Object))
        );
        assert_eq!(
            unit_of(&format!("q:{hex}")).map(|unit| (unit.key, unit.kind)),
            Some((key, UnitKind::Discover))
        );

        // A token that names no key is passed over rather than refused, as a listing's noise is.
        assert!(unit_of("not-a-key").is_none());
    }
}

#[cfg(test)]
mod exchange_tests {
    use rorolala_auth::{Account, Member};
    use rorolala_protocol::{Action as _, ActionContext, OnlyWorkspace};
    use rorolala_storage::{RorolalaStorage, StorageSet, store_file};
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;

    use crate::test_support::{run, scratch, session};

    use super::{ActionSyncHashes, SyncHashesRequest};

    /// The named unit is carried to the Vault, and the answer is how much was read at once.
    #[test]
    fn a_named_unit_crosses_and_the_grant_comes_back() {
        let dir = scratch("sync-hashes");
        let workspace_root = dir.join("workspace");
        Workspace::create(&workspace_root).unwrap();
        let workspace = Workspace::locate(&workspace_root).unwrap();
        let vault_root = dir.join("vault");
        Vault::create(&vault_root).unwrap();
        let vault = Vault::locate(&vault_root).unwrap();

        let workspace_store = RorolalaStorage::create(dir.join("workspace-store"));
        let workspace_set = StorageSet::sole(workspace_store.clone());
        let vault_store = RorolalaStorage::create(dir.join("vault-store"));
        let vault_set = StorageSet::sole(vault_store);

        let file = dir.join("content.bin");
        std::fs::write(&file, b"a content only the workspace holds").unwrap();
        let key = run(store_file(&workspace_store, &file)).unwrap();

        let units = format!("c:3 {}", key.hex());
        let (client, server) = session();
        let workspace_ctx = ActionContext::new_workspace_ctx(Account::default())
            .with_current_workspace(&workspace)
            .with_local_stores(&workspace_set)
            .with_channel(client);
        let vault_ctx = ActionContext::new_vault_ctx(Member::default())
            .with_current_vault(&vault)
            .with_local_stores(&vault_set)
            .with_channel(server);

        let (here, there) = run(async {
            tokio::join!(
                ActionSyncHashes::process(
                    OnlyWorkspace::from(Some(SyncHashesRequest { units })),
                    workspace_ctx
                ),
                ActionSyncHashes::process(
                    OnlyWorkspace::<SyncHashesRequest>::from(None),
                    vault_ctx
                ),
            )
        });

        // A Vault with no bound serves what was asked for: the Workspace is told the grant, while
        // the Vault's own answer is what it falls back to once the answer has been handed over.
        assert_eq!(here.unwrap(), "3");
        assert_eq!(there.unwrap(), "1");

        let held = run(vault_set.contains_keys(&[key])).unwrap();
        assert!(held.held(0), "the Vault took what was named");

        let _ = std::fs::remove_dir_all(dir);
    }
}
