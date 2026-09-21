use std::future::Future;

use serde::Serialize;

use crate::{ActionContext, ActionError, Encodable, OnlyWorkspace};

/// The smallest unit of version-control behavior: one thing a user asked for,
/// written once and run on both sides.
///
/// An action is not a function of one side. It is handed an [`ActionContext`]
/// that says which side it is running as, and the same code takes the Workspace
/// route on one end and the Vault route on the other. A value only one side holds
/// is built with [`only_workspace`](ActionContext::only_workspace) or
/// [`on_vault`](crate::on_vault), and whatever the two sides have to agree on
/// crosses between them through [`sync`](ActionContext::sync) over the encrypted
/// channel — so an action never assumes it holds what the other side holds.
///
/// An action describes *what* to do. Which side it is talking to is read from the
/// context, never passed beside it.
///
/// Like [`Future`], an action names its data with associated types rather than
/// type parameters, so one action is one type: [`Input`](Self::Input) is what it
/// runs on and [`Output`](Self::Output) is what it yields. Running the action
/// yields a future that is [`Send`], so it can be spawned or moved between
/// threads; an implementor can write [`process`](Self::process) as an `async fn`.
pub trait Action {
    /// The id a caller reaches this action by.
    ///
    /// It is a constant rather than only a method because it is read from the source: the
    /// daemon's build script lays its registry out by id, so which slot an action answers
    /// to is settled when the crate is built, and two actions claiming one id stop the
    /// build. [`get_id`](Self::get_id) is the same number, read at runtime.
    const ID: u32;

    /// What the action runs on, carried as a [`OnlyWorkspace`].
    ///
    /// The input belongs to whoever asked for the action, so it is made on the
    /// Workspace side and is empty on the Vault until the action
    /// [`sync`](ActionContext::sync)s it across.
    type Input: Encodable + Send + Sync + 'static;

    /// What the action yields once it has run: the state the caller asked for,
    /// not a step of getting there.
    ///
    /// It is [`Serialize`] because a caller outside Rust reads it: the entry points that
    /// cross the C ABI hand the output back as JSON.
    type Output: Serialize;

    /// The id a caller reaches this action by: its [`ID`](Self::ID).
    ///
    /// An id is what names an action from outside Rust, so an action the C ABI can reach
    /// has one, and no two actions share it.
    #[must_use]
    fn get_id(&self) -> u32 {
        Self::ID
    }

    /// Carries the action out on `input` against `ctx`.
    ///
    /// This runs on both sides. Which one it is follows from `ctx` — see
    /// [`is_workspace`](ActionContext::is_workspace) and
    /// [`is_vault`](ActionContext::is_vault) — and the two sides keep in step by
    /// calling [`sync`](ActionContext::sync) for the same values at the same
    /// time.
    ///
    /// # Errors
    ///
    /// The future resolves to an [`ActionError`] when the action cannot be
    /// carried out: a context with no channel, a value that will not cross it, or
    /// the stream failing.
    fn process(
        input: OnlyWorkspace<Self::Input>,
        ctx: ActionContext,
    ) -> impl Future<Output = Result<Self::Output, ActionError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::Action;
    use crate::{ActionContext, ActionError, OnlyWorkspace};

    /// An action that only reports its id; enough to read the default `get_id`.
    struct Question;

    impl Action for Question {
        const ID: u32 = 42;
        type Input = u64;
        type Output = u64;

        async fn process(
            input: OnlyWorkspace<Self::Input>,
            _ctx: ActionContext,
        ) -> Result<Self::Output, ActionError> {
            Ok(input.unwrap_or_default())
        }
    }

    #[test]
    fn an_action_defaults_to_reporting_its_own_id() {
        assert_eq!(Question.get_id(), 42);
        assert_eq!(Question::ID, 42);
    }
}
