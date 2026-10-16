//! `sync-hashes`: making the two stores hold the same named keys.
//!
//! Where [`sync_all`](crate::sync_all) works out what to sync — everything either side holds — this
//! is told: the keys are the caller's, and the carrying both sides do afterwards is the very
//! primitive a full sync uses. So a full sync is this with everything named, and this is a full sync
//! narrowed to a list.

use std::str::FromStr as _;

use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::Key;

use super::sync_all::local_store;
use super::sync_storage::{Side, Unit, UnitKind, carry_blob, sync_units};

/// An Action that makes the two stores hold the units it is given, and answers how many lanes the
/// Vault will serve.
///
/// # Input
///
/// The units, separated by whitespace — one a line, as a listing prints them. `m:<key>` carries only
/// the manifest a content was cut into, `o:<key>` only the object, `q:<key>` whichever of the two
/// the sender holds, and a bare key the whole of what it names; a hash is read the way a key is
/// anywhere else. A token `c:<n>` asks the Vault for `n`
/// lanes where the asking happens, so a caller that wants to know what it is granted sends `c:<n>`
/// and nothing else and reads the answer.
///
/// # Output
///
/// The number of lanes the Vault will serve, as decimal text: what the caller asked for, held to
/// what the Vault is set to serve. A caller that asked for none is granted one.
pub struct ActionSyncHashes;

impl Action for ActionSyncHashes {
    /// The named sync is id 7.
    const ID: u32 = 7;

    type Input = String;
    type Output = String;

    /// Has the two stores carry the named units, granted as many lanes as the Vault serves.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, and whatever the exchange fails with — see the sync primitive in
    /// [`sync_storage`](crate::sync_storage).
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_store(&ctx)?;

        // The list is the caller's, so it belongs to the Workspace and crosses to the Vault first:
        // both sides must name the same units, or the carrying that follows would say two different
        // things on the two ends.
        let here = input.into_inner();
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let text = arrived
            .or(to_vault)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();

        // How many lanes the caller wants, and how many the Vault will serve: the ask crosses, the
        // answer comes back, and the answer is what the caller is told. Both ends read the same text
        // and take the ask out of it, so they name the same units.
        let (asked, units) = split_ask(&text);
        let granted = grant(&mut ctx, asked).await?;

        let required: Vec<Unit> = units.split_whitespace().filter_map(unit_of).collect();

        // Each side names the store it has; the other wrapper is empty and the other side fills it,
        // exactly as a full sync hands them in.
        let workspace = OnlyWorkspace::new(&ctx, || local.clone());
        let vault = OnlyVault::new(&ctx, || local.clone());
        sync_units(&mut ctx, workspace, vault, Both::new(required)).await?;

        Ok(granted.to_string())
    }
}

/// The lanes the caller asks for, and the units it names.
///
/// A token `c:<n>` is the ask and is taken out of the text; a caller that names none asks for one,
/// which is what a plain listing of keys has always meant.
fn split_ask(text: &str) -> (u32, String) {
    let mut asked = 1_u32;
    let mut units = String::new();

    for token in text.split_whitespace() {
        if let Some(count) = token.strip_prefix("c:") {
            asked = count.parse().unwrap_or(1);

            continue;
        }

        if !units.is_empty() {
            units.push(' ');
        }
        units.push_str(token);
    }

    (asked, units)
}

/// Asks the Vault for `asked` lanes and answers what it grants.
///
/// The ask goes to the Vault, which holds it to what it is set to serve — the configuration's
/// `max_concurrency` — and the grant comes back to the Workspace, which is the side that opens the
/// lanes. A Vault that is set to no number serves what is asked for.
async fn grant(ctx: &mut ActionContext<'_>, asked: u32) -> Result<u32, ActionError> {
    let ask: OnlyWorkspace<u32> = ctx.only_workspace(|| asked);
    let arrived: OnlyVault<u32> = ctx.transfer(ask).await?;

    let answer: OnlyVault<u32> = ctx.only_vault_with_context(|vault_ctx| {
        let cap = vault_ctx
            .current_vault_config()
            .into_inner()
            .map_or(u32::MAX, |config| {
                u32::from(config.daemon_config().max_concurrency())
            });

        granted(arrived.into_inner().unwrap_or(1), cap)
    });

    let granted: OnlyWorkspace<u32> = ctx.transfer(answer).await?;

    Ok(granted.into_inner().unwrap_or(1))
}

/// What a Vault with a cap of `cap` grants a caller that asked for `asked`.
///
/// Never fewer than one, and never more than the Vault serves: the two bounds are what make a
/// caller that asked for none still a caller with one lane, and a caller that asked for the world
/// still one the Vault can answer.
fn granted(asked: u32, cap: u32) -> u32 {
    asked.clamp(1, cap.max(1))
}

/// The unit a token names.
///
/// `m:<key>` carries only the manifest a content was cut into, `o:<key>` only the object, and
/// `q:<key>` whichever of the two the other side holds — what a caller that does not know how a
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

    use super::{UnitKind, granted, split_ask, unit_of};

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

    /// The ask is what the caller wants and the units are what it names, and a caller that asks for
    /// nothing gets one lane.
    #[test]
    fn the_ask_is_taken_out_of_the_units() {
        assert_eq!(split_ask("c:8 abc def"), (8, "abc def".to_string()));
        assert_eq!(split_ask("abc def"), (1, "abc def".to_string()));
        assert_eq!(split_ask("c:4"), (4, String::new()));
        assert_eq!(split_ask(""), (1, String::new()));
    }

    /// A Vault grants no more than it serves and never fewer than one lane.
    #[test]
    fn a_grant_is_held_to_what_the_vault_serves() {
        assert_eq!(granted(4, 16), 4);
        assert_eq!(granted(64, 16), 16);
        assert_eq!(granted(0, 16), 1);
        assert_eq!(granted(8, 0), 1);
        assert_eq!(granted(8, u32::MAX), 8);
    }
}
