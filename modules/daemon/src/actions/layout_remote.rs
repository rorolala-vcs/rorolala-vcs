//! The Layout a Vault-side action works on, and what became of an ownership change.
//!
//! Ownership is not a thing of its own: it is the `owner` a Layout keeps for one entry, and it
//! changes the way everything else about an entry changes — through the Layout's own log. So the
//! actions here reach the Vault's Layout through this module rather than opening one each, and
//! what they answer is one [`Ownership`], so the two ends of an exchange read the same three
//! outcomes without either being told a second time.

use rorolala_errors::BincodeError;
use rorolala_layout::{Layout, MutableData};
use rorolala_protocol::{ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::sync_storage::{Side, carry_blob};

/// What became of a request to take or let go of an entry.
///
/// It is the answer of a Vault-side change, and the Workspace reads it to know whether to touch
/// the Layout it keeps a copy of: [`Owner`](Self::Owner) is the one outcome that is a change,
/// since it is the only one the Vault made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ownership {
    /// The entry is now held by this account, or by none when it is empty.
    Owner(Option<String>),
    /// The Vault holds no entry by that `Uuid`.
    Missing,
    /// Another account holds it, so nothing was changed.
    HeldBy(String),
}

/// The Vault's own Layout, made if the Vault has not one yet.
///
/// Which Vault that is follows from where this runs: an action served by a Vault works on the
/// Vault's Layout, and a Workspace side has none — so a context that is not a Vault is not one an
/// ownership change can run on.
pub(crate) fn local_layout(ctx: &ActionContext<'_>) -> Result<Layout, ActionError> {
    let vault = ctx
        .current_vault()
        .into_inner()
        .ok_or(ActionError::MissingValue)?;

    vault
        .layout()
        .map_err(|error| ActionError::Store(error.to_string()))
}

/// Runs one ownership change against the Vault, carrying the `Uuid` there and the outcome back.
///
/// Which change it is is `decide`'s to make: it is handed the Vault's Layout, the entry, and the
/// name of the account the session was established as, and answers what became of it. The two
/// actions that use this differ in nothing else.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if the Vault holds no member to act as,
/// [`ActionError::Codec`] if the `Uuid` or the outcome will not cross, and whatever the exchange
/// fails with.
pub(crate) async fn ownership_change(
    ctx: &mut ActionContext<'_>,
    input: OnlyWorkspace<String>,
    decide: impl FnOnce(&Layout, Uuid, &str) -> Result<Ownership, ActionError>,
) -> Result<Ownership, ActionError> {
    let id = carry_id(ctx, input).await?;

    // Which account this runs as is the Vault's to say: the session was established as a member,
    // and a name the caller sent would be one the Vault cannot hold itself to.
    let outcome = if ctx.is_vault() {
        let me = ctx
            .get_member()
            .into_inner()
            .map(|member| member.name())
            .ok_or(ActionError::MissingValue)?;

        Some(decide(&local_layout(ctx)?, id, &me)?)
    } else {
        None
    };

    let mine = outcome
        .as_ref()
        .map(rorolala_protocol::Encodable::encode)
        .transpose()
        .map_err(codec_failed)?;
    let arrived = carry_blob(ctx, Side::Workspace, mine.clone()).await?;
    let bytes = arrived.or(mine).unwrap_or_default();

    if ctx.is_vault() {
        // UNWRAP: the outcome above was made on exactly this side, so the side that is a Vault has
        // one to hand back rather than bytes to read.
        Ok(outcome.unwrap())
    } else {
        Ownership::decode(bytes).map_err(codec_failed)
    }
}

/// Names the entry `id` as held by `owner`, refusing when another account holds it.
///
/// What is rewritten is the whole of what the Layout keeps for the entry, since that is what a
/// Layout's log holds: the version and what it says are read back as they were and written
/// unchanged, so naming an owner changes nothing else about the entry.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the Layout could not be written.
pub(crate) fn take(layout: &Layout, id: Uuid, owner: &str) -> Result<Ownership, ActionError> {
    let Some(data) = layout.entry(id) else {
        return Ok(Ownership::Missing);
    };

    if let Some(held) = data.owner()
        && held != owner
    {
        return Ok(Ownership::HeldBy(held.to_owned()));
    }

    layout
        .update_entry(
            id,
            MutableData::new(
                Some(owner.to_owned()),
                data.version(),
                data.description().to_owned(),
            ),
        )
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(Ownership::Owner(Some(owner.to_owned())))
}

/// Removes the name of whoever holds the entry `id`, refusing when it is another account's.
///
/// Naming no owner for an entry nobody holds is nothing to write, so it is answered as the
/// success it is rather than as a change: a run that lets go of what it no longer holds has
/// done what it asked.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the Layout could not be written.
pub(crate) fn giveup(layout: &Layout, id: Uuid, owner: &str) -> Result<Ownership, ActionError> {
    let Some(data) = layout.entry(id) else {
        return Ok(Ownership::Missing);
    };

    match data.owner() {
        Some(held) if held != owner => Ok(Ownership::HeldBy(held.to_owned())),
        None => Ok(Ownership::Owner(None)),
        Some(_) => {
            layout
                .update_entry(
                    id,
                    MutableData::new(None, data.version(), data.description().to_owned()),
                )
                .map_err(|error| ActionError::Store(error.to_string()))?;

            Ok(Ownership::Owner(None))
        }
    }
}

/// Carries the `Uuid` a change names from the Workspace to the Vault, and reads it on both ends.
///
/// # Errors
///
/// Returns [`ActionError::Codec`] if the text is not a `Uuid`, and whatever the exchange fails
/// with.
async fn carry_id(
    ctx: &mut ActionContext<'_>,
    input: OnlyWorkspace<String>,
) -> Result<Uuid, ActionError> {
    let here = input.into_inner();
    let to_vault = here.map(String::into_bytes);
    let arrived = carry_blob(ctx, Side::Vault, to_vault.clone()).await?;
    let named = arrived
        .or(to_vault)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();

    Uuid::parse_str(named.trim())
        .map_err(|_| ActionError::Codec(BincodeError::new("the uuid does not read".to_owned())))
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(BincodeError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::{Layout, MutableData};

    use super::{Ownership, giveup, take};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-layout-remote-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A Layout holding one entry nobody owns.
    fn held(label: &str) -> (Layout, uuid::Uuid) {
        let layout = Layout::open(scratch(label)).unwrap();
        let id = uuid::Uuid::from_u128(1);
        layout
            .create_entry(id, MutableData::new(None, [1; 32], "x".to_owned()))
            .unwrap();

        (layout, id)
    }

    #[test]
    fn an_entry_nobody_holds_is_taken_and_let_go_of() {
        let (layout, id) = held("take");

        assert_eq!(
            take(&layout, id, "alice").unwrap(),
            Ownership::Owner(Some("alice".to_owned()))
        );
        // The version and what the entry says are not what an ownership change touches.
        let data = layout.entry(id).unwrap();
        assert_eq!(data.owner(), Some("alice"));
        assert_eq!(data.version(), [1; 32]);
        assert_eq!(data.description(), "x");

        assert_eq!(
            giveup(&layout, id, "alice").unwrap(),
            Ownership::Owner(None)
        );
        assert_eq!(layout.entry(id).unwrap().owner(), None);

        // Letting go of what nobody holds is already what was asked for.
        assert_eq!(
            giveup(&layout, id, "alice").unwrap(),
            Ownership::Owner(None)
        );
    }

    #[test]
    fn an_entry_another_account_holds_is_left_alone() {
        let (layout, id) = held("held");

        take(&layout, id, "alice").unwrap();

        assert_eq!(
            take(&layout, id, "bob").unwrap(),
            Ownership::HeldBy("alice".to_owned())
        );
        assert_eq!(
            giveup(&layout, id, "bob").unwrap(),
            Ownership::HeldBy("alice".to_owned())
        );

        // Neither refusal changed it.
        assert_eq!(layout.entry(id).unwrap().owner(), Some("alice"));
    }

    #[test]
    fn an_entry_the_layout_does_not_hold_is_reported_missing() {
        let (layout, _) = held("missing");

        assert_eq!(
            take(&layout, uuid::Uuid::from_u128(9), "alice").unwrap(),
            Ownership::Missing
        );
        assert_eq!(
            giveup(&layout, uuid::Uuid::from_u128(9), "alice").unwrap(),
            Ownership::Missing
        );
    }
}
