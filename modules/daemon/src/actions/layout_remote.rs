//! The Layout a Vault-side action works on, and what became of an ownership change.
//!
//! Ownership is not a thing of its own: it is the `owner` a Layout keeps for one entry, and it
//! changes the way everything else about an entry changes — through the Layout's own log. So the
//! actions here reach the Vault's Layout through this module rather than opening one each, and
//! what they answer is one [`Ownership`], so the two ends of an exchange read the same three
//! outcomes without either being told a second time.

use std::str::FromStr as _;

use rorolala_errors::BincodeError;
use rorolala_layout::{Layout, LayoutPath, MutableData};
use rorolala_protocol::{ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_storage::Key;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::sync_storage::{Side, carry_blob};

/// The markers a remote path may sit under, at the top of the path.
///
/// A file is not put where it belongs when it is first submitted: it lands under `new` until
/// someone locates it, and moving it under `removed` is how it is deprecated. Both are states
/// rather than places, so `@` is reserved for them and a located path does not start with one —
/// and `@` rather than `#`, which a shell reads as the start of a comment.
const MARKERS: &[&str] = &["new", "removed"];

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

/// What became of a request to move a path in the Vault's Layout.
///
/// A move is how a file is located and how it is deprecated, so it is the holder's or an
/// administrator's to make: the Workspace reads which of the outcomes came back to know whether
/// the Layout it keeps a copy of is to be changed with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathMove {
    /// The entry is now at the path it was moved to.
    Moved,
    /// The Vault's Layout names nothing at the path that was to move.
    Missing,
    /// The path it was to move to already names something.
    Taken,
    /// The account neither holds the entry nor administrates the Vault.
    Refused,
    /// A path is not one a remote Layout may name.
    Malformed,
}

/// Moves whatever is at `from` to `to`, refusing what the account may not move.
///
/// Which account this runs as is already known — the session says — and whether it administrates
/// the Vault is read from the Vault's own configuration by the caller, so both come in as values
/// and the decision here is only about the Layout.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the Layout could not be written.
pub(crate) fn move_remote(
    layout: &Layout,
    from: &str,
    to: &str,
    me: &str,
    admin: bool,
) -> Result<PathMove, ActionError> {
    let (Ok(from), Ok(to)) = (LayoutPath::new(from), LayoutPath::new(to)) else {
        return Ok(PathMove::Malformed);
    };

    if !is_remote_path(&to) {
        return Ok(PathMove::Malformed);
    }

    let Some(id) = layout.id_of(&from) else {
        return Ok(PathMove::Missing);
    };
    if layout.id_of(&to).is_some() {
        return Ok(PathMove::Taken);
    }

    let held = layout
        .entry(id)
        .and_then(|data| data.owner().map(str::to_owned));
    if !admin && held.as_deref() != Some(me) {
        return Ok(PathMove::Refused);
    }

    layout
        .move_path(&from, &to)
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(PathMove::Moved)
}

/// Whether `path` is one a remote Layout may name.
///
/// A path whose first component is `@` is a marker path: it says what state the file is in rather
/// than where it belongs, so only the markers this design names may follow the `@`, and something
/// has to be under them. A path that starts anywhere else is a located path, and is left alone.
fn is_remote_path(path: &LayoutPath) -> bool {
    let mut components = path.as_str().split('/');

    if components.next() != Some("@") {
        return true;
    }

    components
        .next()
        .is_some_and(|marker| MARKERS.contains(&marker))
        && components.next().is_some()
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
pub(crate) fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(BincodeError::new(error.to_string()))
}

/// The prefix a `Uuid` only one side holds is put under when it goes up.
const NEW_PREFIX: &str = "@/new/";

/// What became of a request to write an entry of the Vault's Layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryWrite {
    /// The Vault's Layout now holds what was asked for.
    Written,
    /// The Vault already holds an entry by that `Uuid`.
    Exists,
    /// The Vault already names something at that path.
    Taken,
    /// The Vault holds no entry by that `Uuid`.
    Missing,
    /// The account does not hold the entry and does not administrate the Vault.
    Refused,
    /// A path, a `Uuid`, or a version is not one this can read.
    Malformed,
}

/// Runs one write against the Vault, carrying the input there and the outcome back.
///
/// Which write it is is `decide`'s to make: it is handed the Vault's Layout, what crossed, the
/// name of the account the session was established as, and whether that account administrates the
/// Vault. Neither of the last two is taken from the caller.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if the Vault holds no member to act as,
/// [`ActionError::Codec`] if the input or the outcome will not cross, and whatever the exchange
/// fails with.
pub(crate) async fn remote_write<Input>(
    ctx: &mut ActionContext<'_>,
    input: OnlyWorkspace<Input>,
    decide: impl FnOnce(&Layout, Input, &str, bool) -> Result<EntryWrite, ActionError>,
) -> Result<EntryWrite, ActionError>
where
    Input: rorolala_protocol::Encodable + Send + Sync + 'static,
{
    let here = input.into_inner();
    let to_vault = here
        .as_ref()
        .map(rorolala_protocol::Encodable::encode)
        .transpose()
        .map_err(codec_failed)?;
    let arrived = carry_blob(ctx, Side::Vault, to_vault.clone()).await?;
    let bytes = arrived.or(to_vault).unwrap_or_default();
    let crossed = Input::decode(bytes).map_err(codec_failed)?;

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

        Some(decide(&local_layout(ctx)?, crossed, &me, admin)?)
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
        // UNWRAP: the outcome above was made on exactly this side.
        Ok(outcome.unwrap())
    } else {
        EntryWrite::decode(bytes).map_err(codec_failed)
    }
}

/// Creates an entry of the Vault's Layout under `@/new/`, held by the account that asks.
///
/// The path keeps the name the file was submitted under and carries the short form of its `Uuid`,
/// so two files submitted under one name are still two paths.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the Layout could not be written.
pub(crate) fn create_remote(
    layout: &Layout,
    path: &str,
    uuid: &str,
    version: &str,
    owner: &str,
) -> Result<EntryWrite, ActionError> {
    let Ok(path) = LayoutPath::new(path) else {
        return Ok(EntryWrite::Malformed);
    };
    let Ok(id) = Uuid::parse_str(uuid.trim()) else {
        return Ok(EntryWrite::Malformed);
    };
    let Ok(version) = Key::from_str(version.trim()) else {
        return Ok(EntryWrite::Malformed);
    };

    if !path.as_str().starts_with(NEW_PREFIX) {
        return Ok(EntryWrite::Malformed);
    }
    if layout.entry(id).is_some() {
        return Ok(EntryWrite::Exists);
    }
    if layout.id_of(&path).is_some() {
        return Ok(EntryWrite::Taken);
    }

    layout
        .create_path(&path, id)
        .map_err(|error| ActionError::Store(error.to_string()))?;
    layout
        .create_entry(
            id,
            MutableData::new(Some(owner.to_owned()), *version.digest(), String::new()),
        )
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(EntryWrite::Written)
}

/// Sets the version an entry of the Vault's Layout is at, leaving its path and holder alone.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the Layout could not be written.
pub(crate) fn set_remote(
    layout: &Layout,
    uuid: &str,
    version: &str,
    me: &str,
    admin: bool,
) -> Result<EntryWrite, ActionError> {
    let Ok(id) = Uuid::parse_str(uuid.trim()) else {
        return Ok(EntryWrite::Malformed);
    };
    let Ok(version) = Key::from_str(version.trim()) else {
        return Ok(EntryWrite::Malformed);
    };

    let Some(data) = layout.entry(id) else {
        return Ok(EntryWrite::Missing);
    };
    if !admin && data.owner() != Some(me) {
        return Ok(EntryWrite::Refused);
    }

    layout
        .update_entry(
            id,
            MutableData::new(
                data.owner().map(str::to_owned),
                *version.digest(),
                data.description().to_owned(),
            ),
        )
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(EntryWrite::Written)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use std::str::FromStr as _;

    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use rorolala_storage::Key;

    use super::{
        EntryWrite, Ownership, PathMove, create_remote, giveup, move_remote, set_remote, take,
    };

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

    /// A Layout holding one entry at a marker path, held by `owner`.
    fn at(label: &str, path: &str, owner: Option<&str>) -> (Layout, uuid::Uuid) {
        let layout = Layout::open(scratch(label)).unwrap();
        let id = uuid::Uuid::from_u128(2);
        layout
            .create_entry(
                id,
                MutableData::new(owner.map(str::to_owned), [2; 32], String::new()),
            )
            .unwrap();
        layout
            .create_path(&LayoutPath::new(path).unwrap(), id)
            .unwrap();

        (layout, id)
    }

    #[test]
    fn a_path_is_moved_by_its_holder_or_an_administrator() {
        let (layout, id) = at(
            "move-holder",
            "@/new/临时文件/模型.fbx@a1b2c3",
            Some("alice"),
        );

        // A member who holds neither the entry nor the Vault may not move it.
        assert_eq!(
            move_remote(
                &layout,
                "@/new/临时文件/模型.fbx@a1b2c3",
                "Models/model.fbx",
                "bob",
                false
            )
            .unwrap(),
            PathMove::Refused
        );
        assert_eq!(
            layout.id_of(&LayoutPath::new("@/new/临时文件/模型.fbx@a1b2c3").unwrap()),
            Some(id)
        );

        // Its holder may locate it, and the marker path stops naming it.
        assert_eq!(
            move_remote(
                &layout,
                "@/new/临时文件/模型.fbx@a1b2c3",
                "Models/model.fbx",
                "alice",
                false
            )
            .unwrap(),
            PathMove::Moved
        );
        assert_eq!(
            layout.id_of(&LayoutPath::new("Models/model.fbx").unwrap()),
            Some(id)
        );
        assert!(
            layout
                .id_of(&LayoutPath::new("@/new/临时文件/模型.fbx@a1b2c3").unwrap())
                .is_none()
        );

        // An administrator may move what it does not hold.
        let (other, other_id) = at("move-admin", "@/new/a.psd@d4e5f6", Some("bob"));
        assert_eq!(
            move_remote(
                &other,
                "@/new/a.psd@d4e5f6",
                "@/removed/a.psd",
                "carol",
                true
            )
            .unwrap(),
            PathMove::Moved
        );
        assert_eq!(
            other.id_of(&LayoutPath::new("@/removed/a.psd").unwrap()),
            Some(other_id)
        );
    }

    #[test]
    fn a_move_that_cannot_be_made_says_which_way() {
        let (layout, _) = at("move-missing", "@/new/a.psd@d4e5f6", Some("alice"));

        // Nothing is at the path that was to move.
        assert_eq!(
            move_remote(&layout, "nowhere/a.psd", "Models/a.psd", "alice", false).unwrap(),
            PathMove::Missing
        );

        // The path it was to move to already names something.
        assert_eq!(
            move_remote(
                &layout,
                "@/new/a.psd@d4e5f6",
                "@/new/a.psd@d4e5f6",
                "alice",
                false
            )
            .unwrap(),
            PathMove::Taken
        );

        // `@` is reserved for the two markers this design names, and a marker names a state
        // rather than a place, so something has to be under it.
        for refused in ["@/archive/a.psd", "@/new", "@/removed"] {
            assert_eq!(
                move_remote(&layout, "@/new/a.psd@d4e5f6", refused, "alice", true).unwrap(),
                PathMove::Malformed,
                "{refused}"
            );
        }
    }

    #[test]
    fn a_new_entry_goes_under_new_and_is_held_by_its_creator() {
        let layout = Layout::open(scratch("create")).unwrap();
        let id = uuid::Uuid::from_u128(3);
        let version = "11".repeat(32);

        assert_eq!(
            create_remote(
                &layout,
                "@/new/art/hero.psd@abc1234",
                &id.to_string(),
                &version,
                "alice"
            )
            .unwrap(),
            EntryWrite::Written
        );
        assert_eq!(layout.entry(id).unwrap().owner(), Some("alice"));
        assert_eq!(
            layout.id_of(&LayoutPath::new("@/new/art/hero.psd@abc1234").unwrap()),
            Some(id)
        );

        // The same `Uuid` again, and a path already taken, are told apart from a write.
        assert_eq!(
            create_remote(
                &layout,
                "@/new/other.psd@abc1234",
                &id.to_string(),
                &version,
                "alice"
            )
            .unwrap(),
            EntryWrite::Exists
        );
        assert_eq!(
            create_remote(
                &layout,
                "@/new/art/hero.psd@abc1234",
                &uuid::Uuid::from_u128(4).to_string(),
                &version,
                "alice"
            )
            .unwrap(),
            EntryWrite::Taken
        );

        // A path outside `@/new/` is not a name a new entry is made under.
        assert_eq!(
            create_remote(
                &layout,
                "art/hero.psd",
                &uuid::Uuid::from_u128(4).to_string(),
                &version,
                "alice"
            )
            .unwrap(),
            EntryWrite::Malformed
        );
    }

    #[test]
    fn a_version_is_set_by_the_holder_and_left_by_others() {
        let (layout, id) = at("set", "@/new/a.psd@d4e5f6", Some("alice"));
        let version = "22".repeat(32);

        assert_eq!(
            set_remote(&layout, &id.to_string(), &version, "bob", false).unwrap(),
            EntryWrite::Refused
        );
        assert_eq!(
            set_remote(&layout, &id.to_string(), &version, "alice", false).unwrap(),
            EntryWrite::Written
        );
        assert_eq!(
            layout.entry(id).unwrap().version(),
            *Key::from_str(&version).unwrap().digest()
        );

        // A version change touches neither the holder nor the path, and an administrator may
        // make one it does not hold.
        assert_eq!(layout.entry(id).unwrap().owner(), Some("alice"));
        assert_eq!(
            layout.id_of(&LayoutPath::new("@/new/a.psd@d4e5f6").unwrap()),
            Some(id)
        );
        assert_eq!(
            set_remote(&layout, &id.to_string(), &version, "carol", true).unwrap(),
            EntryWrite::Written
        );

        // A `Uuid` the Layout does not hold is missing.
        assert_eq!(
            set_remote(
                &layout,
                &uuid::Uuid::from_u128(9).to_string(),
                &version,
                "alice",
                false
            )
            .unwrap(),
            EntryWrite::Missing
        );
    }
}
