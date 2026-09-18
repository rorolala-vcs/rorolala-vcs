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
    /// # Examples
    ///
    /// ```
    /// use rorolala_protocol::{Both, OnlyWorkspace};
    ///
    /// let by_method: OnlyWorkspace<u64> = Both::new(7_u64).only_workspace();
    /// let by_into: OnlyWorkspace<u64> = Both::new(7_u64).into();
    /// assert_eq!(by_method.into_inner(), Some(7));
    /// assert_eq!(by_into.into_inner(), Some(7));
    /// ```
    #[must_use]
    pub fn only_workspace(self) -> OnlyWorkspace<T> {
        OnlyWorkspace::from(self)
    }

    /// Demotes the value to one only the Vault holds.
    ///
    /// What comes back is an [`OnlyVault`], so the Vault is the side that decides
    /// from here on: [`sync_mut`](OnlyVault::sync_mut) changes it there and carries the
    /// result to the Workspace, and [`mut_on_vault`](OnlyVault::mut_on_vault)
    /// changes it there and nowhere else.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_protocol::{Both, OnlyVault};
    ///
    /// let by_method: OnlyVault<u64> = Both::new(7_u64).only_vault();
    /// let by_into: OnlyVault<u64> = Both::new(7_u64).into();
    /// assert_eq!(by_method.into_inner(), Some(7));
    /// assert_eq!(by_into.into_inner(), Some(7));
    /// ```
    #[must_use]
    pub fn only_vault(self) -> OnlyVault<T> {
        OnlyVault::from(self)
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
        ctx: &mut ActionContext,
        inputs: Inputs,
        run: Run,
    ) -> Result<(), ActionError>
    where
        Inputs: SyncMutWith<T, Run>,
    {
        inputs.sync_mut_with(ctx, &mut self.value, run).await
    }
}
