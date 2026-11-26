//! `remove-remote-path`: taking an entry out of the Vault's Layout.
//!
//! Deleting a file of the Vault's is the entry going rather than the entry moving: a move keeps what
//! is at the path and changes where it is, while this is the Vault's Layout no longer naming it. What
//! the entry's version names stays in the store, and the version and the objects under it stay in the
//! index, so what is lost is the file as a thing the Vault holds, its path, its holder and what it
//! says about itself.
//!
//! Who may remove is the account that holds it, or one that administrates the Vault, which is the
//! rule a version change already follows: taking an entry away is a change of who holds what, and an
//! account's own entries are its own to let go of.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

use super::layout_remote::{EntryRemove, remote_transact, remove_remote};

/// An Action that removes the entry a path names from the Vault's Layout.
///
/// # Input
///
/// The path, as the Vault's own Layout writes it. It belongs to the caller, so it is read on the
/// Workspace and crosses to the Vault, which is the side that has a Layout to remove it from.
///
/// # Output
///
/// What became of it: the entry is gone, the path names nothing, the account may not remove it, or
/// the path is not one a remote Layout may name. [`EntryRemoveOutcome`] is what a C caller reads.
pub struct ActionRemoveRemotePath;

/// The path whose entry is to be removed.
///
/// # FFI
///
/// A `RolaDaemonRemoveRemotePathRequest` the caller fills. `path` is borrowed for the call and
/// released by the caller, and is read as the Vault's own Layout writes it.
#[lazyffi(export = RolaDaemonRemoveRemotePathRequest, input)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoveRemotePathRequest {
    /// The path to remove.
    pub path: String,
}

impl Action for ActionRemoveRemotePath {
    /// Removing a path is id 19: the first of the writes a file's deletion makes.
    const ID: u32 = 19;

    type Input = RemoveRemotePathRequest;
    type Output = EntryRemoveOutcome;

    /// Asks the Vault to remove the path, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if a path or an outcome
    /// will not cross, [`ActionError::Store`] if the Vault's Layout could not be written, and
    /// whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcome: EntryRemove =
            remote_transact(&mut ctx, input, |layout, request, me, admin| {
                remove_remote(layout, &request.path, me, admin)
            })
            .await?;

        Ok(EntryRemoveOutcome::from(outcome))
    }
}

/// What became of a request to remove an entry, as a C caller reads it.
///
/// # FFI
///
/// A `RolaDaemonEntryRemoveOutcome *` the caller releases with `free_rola_daemon_entry_remove_outcome`. `kind` says
/// which outcome it was, and `owner` is the account that holds the entry when the removal was
/// refused and somebody holds it, empty otherwise.
#[lazyffi(export = RolaDaemonEntryRemoveOutcome)]
#[derive(Debug, Clone, Serialize)]
pub struct EntryRemoveOutcome {
    /// Which outcome the removal reached.
    kind: RemoveKind,
    /// The account that holds the entry, empty when nobody does.
    owner: String,
}

/// Which outcome a removal reached.
#[lazyffi(export = RolaDaemonEntryRemoveKind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RemoveKind {
    /// The Vault's Layout no longer holds the entry, or names a path for it.
    Removed,
    /// The Vault's Layout names nothing at that path, or holds no entry by it.
    Missing,
    /// The account does not hold the entry and does not administrate the Vault.
    Refused,
    /// A path is not one a remote Layout may name.
    Malformed,
}

#[lazyffi(export = rola_daemon_)]
impl EntryRemoveOutcome {
    /// Which outcome the removal reached.
    #[must_use]
    pub const fn kind(&self) -> RemoveKind {
        self.kind
    }

    /// The account that holds the entry, or an empty string when nobody does.
    ///
    /// # FFI
    ///
    /// The caller releases it with `free_string`.
    #[must_use]
    pub fn owner(&self) -> String {
        self.owner.clone()
    }
}

impl From<EntryRemove> for EntryRemoveOutcome {
    fn from(outcome: EntryRemove) -> Self {
        match outcome {
            EntryRemove::Removed => Self {
                kind: RemoveKind::Removed,
                owner: String::new(),
            },
            EntryRemove::Missing => Self {
                kind: RemoveKind::Missing,
                owner: String::new(),
            },
            EntryRemove::Refused { owner } => Self {
                kind: RemoveKind::Refused,
                owner: owner.unwrap_or_default(),
            },
            EntryRemove::Malformed => Self {
                kind: RemoveKind::Malformed,
                owner: String::new(),
            },
        }
    }
}

impl EntryRemoveOutcome {
    /// The outcome as the action made it, for a caller inside Rust.
    #[must_use]
    pub fn outcome(&self) -> EntryRemove {
        match self.kind {
            RemoveKind::Removed => EntryRemove::Removed,
            RemoveKind::Missing => EntryRemove::Missing,
            RemoveKind::Refused => EntryRemove::Refused {
                owner: (!self.owner.is_empty()).then(|| self.owner.clone()),
            },
            RemoveKind::Malformed => EntryRemove::Malformed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EntryRemove, EntryRemoveOutcome, RemoveKind};

    #[test]
    fn every_removal_outcome_crosses_and_comes_back() {
        for (outcome, kind) in [
            (EntryRemove::Removed, RemoveKind::Removed),
            (EntryRemove::Missing, RemoveKind::Missing),
            (
                EntryRemove::Refused {
                    owner: Some("alice".to_owned()),
                },
                RemoveKind::Refused,
            ),
            (EntryRemove::Malformed, RemoveKind::Malformed),
        ] {
            let crossed = EntryRemoveOutcome::from(outcome.clone());

            assert_eq!(crossed.kind(), kind);
            assert_eq!(crossed.outcome(), outcome);
        }
    }

    #[test]
    fn a_refusal_names_the_holder_only_when_there_is_one() {
        let held = EntryRemoveOutcome::from(EntryRemove::Refused {
            owner: Some("alice".to_owned()),
        });
        assert_eq!(held.owner(), "alice");

        let nobody = EntryRemoveOutcome::from(EntryRemove::Refused { owner: None });
        assert_eq!(nobody.owner(), "");
        assert_eq!(nobody.outcome(), EntryRemove::Refused { owner: None });
    }
}
