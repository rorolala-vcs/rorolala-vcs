use std::ops::Deref;

use crate::{ActionContext, ActionError, Encodable, OnlyVault, OnlyWorkspace, SyncMutWith};

/// A value both sides hold.
///
/// A `Both` is read through [`Deref`] only, so a reader sees one value and cannot
/// change it behind the other side's back. Changing it goes through
/// [`sync_mut_with`](Self::sync_mut_with), which changes it on the side that holds the
/// read-only inputs it is handed and carries the result to the other side, so the two
/// cannot drift apart unnoticed.
///
/// When the two sides no longer have to keep the value in step at all, it is handed to
/// one of them to keep with [`only_workspace`](Self::only_workspace) or
/// [`only_vault`](Self::only_vault); that side is then the one that decides, and
/// changing it goes through [`OnlyWorkspace::sync_mut`] or [`OnlyVault::sync_mut`].
#[derive(Debug, Clone)]
pub struct Both<T> {
    /// The value both sides hold.
    value: T,
}

impl<T> Both<T> {
    /// Wraps a value both sides already hold.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Takes the value out, leaving the two sides nothing to keep in step.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for Both<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Both<T>
where
    T: Send + Sync + 'static,
{
    /// Demotes the value to one only the Workspace holds.
    ///
    /// What comes back is an [`OnlyWorkspace`], so the Workspace is the side that
    /// decides from here on: [`sync_mut`](OnlyWorkspace::sync_mut) changes it there and
    /// carries the result to the Vault, and
    /// [`mut_on_workspace`](OnlyWorkspace::mut_on_workspace) changes it there and
    /// nowhere else.
    ///
    /// The context is what says whether this is the side that keeps the value: a value
    /// only the Workspace holds must not be left lying on the Vault, so on the Vault it is
    /// dropped and the wrapper comes back empty. There is deliberately no `From` impl to
    /// demote with — one could not see the side, and would hand the value over on both.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::{Account, Member};
    /// use rorolala_protocol::{ActionContext, Both, OnlyWorkspace};
    ///
    /// // On the Workspace, which is the side that holds it, the value stays.
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// let kept: OnlyWorkspace<u64> = Both::new(7_u64).only_workspace(&ctx);
    /// assert_eq!(kept.into_inner(), Some(7));
    ///
    /// // On the Vault it is dropped, since what only the Workspace holds is not made up
    /// // on the side that does not hold it.
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// let dropped: OnlyWorkspace<u64> = Both::new(7_u64).only_workspace(&ctx);
    /// assert!(dropped.into_inner().is_none());
    /// ```
    #[must_use]
    pub fn only_workspace(self, ctx: &ActionContext<'_>) -> OnlyWorkspace<T> {
        OnlyWorkspace::new(ctx, || self.into_inner())
    }

    /// Demotes the value to one only the Vault holds.
    ///
    /// What comes back is an [`OnlyVault`], so the Vault is the side that decides
    /// from here on: [`sync_mut`](OnlyVault::sync_mut) changes it there and carries the
    /// result to the Workspace, and [`mut_on_vault`](OnlyVault::mut_on_vault)
    /// changes it there and nowhere else.
    ///
    /// The context is what says whether this is the side that keeps the value: a value
    /// only the Vault holds must not be left lying on the Workspace, so on the Workspace it
    /// is dropped and the wrapper comes back empty. There is deliberately no `From` impl to
    /// demote with — one could not see the side, and would hand the value over on both.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::{Account, Member};
    /// use rorolala_protocol::{ActionContext, Both, OnlyVault};
    ///
    /// // On the Vault, which is the side that holds it, the value stays.
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// let kept: OnlyVault<u64> = Both::new(7_u64).only_vault(&ctx);
    /// assert_eq!(kept.into_inner(), Some(7));
    ///
    /// // On the Workspace it is dropped, since what only the Vault holds is not made up
    /// // on the side that does not hold it.
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// let dropped: OnlyVault<u64> = Both::new(7_u64).only_vault(&ctx);
    /// assert!(dropped.into_inner().is_none());
    /// ```
    #[must_use]
    pub fn only_vault(self, ctx: &ActionContext<'_>) -> OnlyVault<T> {
        OnlyVault::new(ctx, || self.into_inner())
    }
}

impl<T> Both<T>
where
    T: Encodable + Send + Sync + 'static,
{
    /// Changes the value both sides hold on the side that holds `inputs`, and carries
    /// the result to the other side.
    ///
    /// A value both sides already hold has no side of its own, so which side runs the
    /// closure is read from `inputs` rather than from the receiver: this is the variant
    /// that takes them. A value only one side holds names its side itself, so it is
    /// changed with [`OnlyWorkspace::sync_mut`] or [`OnlyVault::sync_mut`] instead.
    ///
    /// The closure is handed the value first, as `&mut T`, so what it writes is what
    /// the two sides end up holding; then it is handed `inputs` unwrapped, as read-only
    /// data that came from outside:
    ///
    /// * with an [`OnlyVault`], the Vault is the side that holds the input, so the Vault
    ///   runs the closure and the Workspace takes what it produced;
    /// * with an [`OnlyWorkspace`], the Workspace runs it, and the Vault takes what it
    ///   produced.
    ///
    /// Which of the two it is follows from the inputs and not from the caller: a tuple
    /// mixing the two, or holding a value that is not wrapped at all, has no
    /// implementation, so the compiler rejects it. The side that does not hold the inputs
    /// does not run the closure and cannot disagree about what the change was — it takes
    /// what arrives.
    ///
    /// One to twelve inputs are accepted. One is passed bare and reaches the closure as
    /// its second argument:
    ///
    /// ```ignore
    /// let mut value = Both::new(0_i32);
    /// let input: OnlyVault<i32> = ctx.only_vault(|| 7);
    /// value
    ///     .sync_mut_with(&mut ctx, input, |value: &mut i32, input: i32| *value = input)
    ///     .await?;
    /// ```
    ///
    /// Two or more are passed as a tuple, and the closure takes that tuple:
    ///
    /// ```ignore
    /// let mut value = Both::new(0_i32);
    /// let first: OnlyWorkspace<i32> = ctx.only_workspace(|| 10);
    /// let second: OnlyWorkspace<i32> = ctx.only_workspace(|| 20);
    /// value
    ///     .sync_mut_with(&mut ctx, (first, second), |value: &mut i32, (first, second)| {
    ///         *value = first + second;
    ///     })
    ///     .await?;
    /// ```
    ///
    /// Both sides call this for the same value at the same time; the value crosses
    /// through the encrypted channel, and the receiving side ends up holding what the
    /// change produced.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError`] if the context has no channel, the side that holds the
    /// inputs held none to read, the value will not cross the channel, or the stream
    /// fails.
    pub async fn sync_mut_with<Inputs, Run>(
        &mut self,
        ctx: &mut ActionContext<'_>,
        inputs: Inputs,
        run: Run,
    ) -> Result<(), ActionError>
    where
        Inputs: SyncMutWith<T, Run>,
    {
        inputs.sync_mut_with(ctx, &mut self.value, run).await
    }
}
