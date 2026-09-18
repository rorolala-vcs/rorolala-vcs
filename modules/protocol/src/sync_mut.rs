//! `sync_mut`: changing a value on the side that holds it, and carrying it across.
//!
//! There are two entry points, split by where the side a change lands on comes from:
//!
//! * [`OnlyWorkspace::sync_mut`] and [`OnlyVault::sync_mut`] take no data from outside.
//!   The wrapper already names the side — it holds the value — so the closure is handed
//!   that value alone, and both sides come out holding what it wrote as a [`Both`].
//! * [`Both::sync_mut_with`] changes a value both sides already hold. The side is named
//!   by the read-only inputs the caller passes instead, which is what the `_with` is
//!   about.
//!
//! The input variants for two to twelve values are generated from a single template by
//! `internal_repeat!`. The one-input case is written out instead, because it takes its
//! input bare rather than as a one-element tuple.

use std::future::Future;

use rorolala_protocol_macros::internal_repeat;

use crate::sync::{receive_value, send_value};
use crate::{ActionContext, ActionError, Both, Encodable, OnlyVault, OnlyWorkspace};

impl<T> OnlyWorkspace<T>
where
    T: Encodable + Send + Sync + 'static,
{
    /// Changes the value on the Workspace, and carries it to the Vault.
    ///
    /// The Workspace is the side that holds the value, so it is the side that runs the
    /// closure; what the closure wrote is sent to the Vault, and both sides come out
    /// holding it as a [`Both`]. The closure is handed the value as `&mut T`, so it reads
    /// what the Workspace held and writes what both sides are to hold.
    ///
    /// Both sides call this for the same value at the same time. The Vault's own wrapper
    /// is empty, which is how it is built on that side, so the Vault does not run the
    /// closure and cannot disagree about what the change was; it takes what arrives.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError`] if the context has no channel, the Workspace held no value
    /// to change, the value will not cross the channel, or the stream fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let input: OnlyWorkspace<String> = ctx.only_workspace(|| "world".to_owned());
    /// let received = input
    ///     .sync_mut(&mut ctx, |raw| *raw = format!("Hello, {raw}"))
    ///     .await?;
    /// assert_eq!(received.into_inner(), "Hello, world");
    /// ```
    pub async fn sync_mut<Run>(
        self,
        ctx: &mut ActionContext,
        run: Run,
    ) -> Result<Both<T>, ActionError>
    where
        Run: FnOnce(&mut T) + Send,
    {
        if ctx.is_workspace() {
            let mut value = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(&mut value);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, &value).await?;

            Ok(Both::new(value))
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            let value = receive_value(channel).await?;

            Ok(Both::new(value))
        }
    }
}

impl<T> OnlyVault<T>
where
    T: Encodable + Send + Sync + 'static,
{
    /// Changes the value on the Vault, and carries it to the Workspace.
    ///
    /// As [`OnlyWorkspace::sync_mut`], with the two sides swapped: the Vault is the side
    /// that holds the value, so the Vault runs the closure and the Workspace takes what
    /// it produced.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError`] if the context has no channel, the Vault held no value to
    /// change, the value will not cross the channel, or the stream fails.
    pub async fn sync_mut<Run>(
        self,
        ctx: &mut ActionContext,
        run: Run,
    ) -> Result<Both<T>, ActionError>
    where
        Run: FnOnce(&mut T) + Send,
    {
        if ctx.is_vault() {
            let mut value = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(&mut value);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, &value).await?;

            Ok(Both::new(value))
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            let value = receive_value(channel).await?;

            Ok(Both::new(value))
        }
    }
}

/// What a [`Both::sync_mut_with`](crate::Both::sync_mut_with) input is, and which side
/// runs the closure.
///
/// Implemented for a single [`OnlyWorkspace`] or [`OnlyVault`], and for a tuple of two
/// to twelve of either. A tuple mixing the two, or holding a value that is not wrapped
/// at all, has no implementation, so the compiler rejects it — which is what makes the
/// side a change lands on follow from the inputs rather than from a flag.
///
/// This is the shape [`sync_mut_with`](crate::Both::sync_mut_with) dispatches through; it
/// is an implementation detail, not something to implement or name.
#[doc(hidden)]
pub trait SyncMutWith<T, Run> {
    /// Changes `value` on the side that holds the inputs, then carries it across.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError`] if the context has no channel, the side that holds the
    /// inputs held none to read, the value will not cross the channel, or the stream
    /// fails.
    fn sync_mut_with(
        self,
        ctx: &mut ActionContext,
        value: &mut T,
        run: Run,
    ) -> impl Future<Output = Result<(), ActionError>> + Send;
}

/// The one-input Vault case: the Vault holds the input, so the Vault runs the closure.
impl<T, A, Run> SyncMutWith<T, Run> for OnlyVault<A>
where
    T: Encodable + Send + Sync + 'static,
    A: Send + Sync + 'static,
    Run: FnOnce(&mut T, A) + Send,
{
    async fn sync_mut_with(
        self,
        ctx: &mut ActionContext,
        value: &mut T,
        run: Run,
    ) -> Result<(), ActionError> {
        if ctx.is_vault() {
            let input = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(value, input);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, value).await
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            *value = receive_value(channel).await?;
            Ok(())
        }
    }
}

/// The one-input Workspace case: the Workspace holds the input, so it runs the closure.
impl<T, A, Run> SyncMutWith<T, Run> for OnlyWorkspace<A>
where
    T: Encodable + Send + Sync + 'static,
    A: Send + Sync + 'static,
    Run: FnOnce(&mut T, A) + Send,
{
    async fn sync_mut_with(
        self,
        ctx: &mut ActionContext,
        value: &mut T,
        run: Run,
    ) -> Result<(), ActionError> {
        if ctx.is_workspace() {
            let input = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(value, input);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, value).await
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            *value = receive_value(channel).await?;
            Ok(())
        }
    }
}

internal_repeat!(2..=12 => {
    /// Vault-only inputs: the Vault holds them, so the Vault runs the closure.
    impl<T, Run, (A$-, +)> SyncMutWith<T, Run>
        for (
            (
                OnlyVault<A$->,
            +)
            ,
        )
    where
        Run: FnOnce(
            &mut T,
            (
                (
                    A$-,
                +)
                ,
            ),
        ) + Send,
        T: Encodable + Send + Sync + 'static,
        (
            A$-: Send + Sync + 'static,
        +)
    {
        async fn sync_mut_with(
            self,
            ctx: &mut ActionContext,
            value: &mut T,
            run: Run,
        ) -> Result<(), ActionError> {
            let (
                (
                    a$-,
                +)
                ,
            ) = self;

            if ctx.is_vault() {
                let (
                    (
                        i$-,
                    +)
                    ,
                ) = (
                    (
                        a$-.into_inner().ok_or(ActionError::MissingValue)?,
                    +)
                    ,
                );

                run(value, (
                    (
                        i$-,
                    +)
                    ,
                ));

                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                send_value(channel, value).await
            } else {
                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                *value = receive_value(channel).await?;
                Ok(())
            }
        }
    }

    /// Workspace-only inputs: the Workspace holds them, so it runs the closure.
    impl<T, Run, (A$-, +)> SyncMutWith<T, Run>
        for (
            (
                OnlyWorkspace<A$->,
            +)
            ,
        )
    where
        Run: FnOnce(
            &mut T,
            (
                (
                    A$-,
                +)
                ,
            ),
        ) + Send,
        T: Encodable + Send + Sync + 'static,
        (
            A$-: Send + Sync + 'static,
        +)
    {
        async fn sync_mut_with(
            self,
            ctx: &mut ActionContext,
            value: &mut T,
            run: Run,
        ) -> Result<(), ActionError> {
            let (
                (
                    a$-,
                +)
                ,
            ) = self;

            if ctx.is_workspace() {
                let (
                    (
                        i$-,
                    +)
                    ,
                ) = (
                    (
                        a$-.into_inner().ok_or(ActionError::MissingValue)?,
                    +)
                    ,
                );

                run(value, (
                    (
                        i$-,
                    +)
                    ,
                ));

                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                send_value(channel, value).await
            } else {
                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                *value = receive_value(channel).await?;
                Ok(())
            }
        }
    }
});
