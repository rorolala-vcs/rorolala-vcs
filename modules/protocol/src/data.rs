use crate::{ActionContext, Both};

/// A value that only exists on the Vault side.
///
/// `OnlyVault` is used to represent, at the type level, that a value lives on
/// the Vault side rather than the Workspace side during the execution of an
/// `Action`.
///
/// The wrapper is read through [`Deref`](std::ops::Deref) only; changing the
/// value goes through [`mut_on_vault`](Self::mut_on_vault), which changes it on
/// the side that holds it.
#[derive(Debug)]
pub struct OnlyVault<Inner>
where
    Inner: Send + Sync + 'static,
{
    inner: Option<Inner>,
}

impl<Inner> OnlyVault<Inner>
where
    Inner: Send + Sync + 'static,
{
    /// Builds a value from `f`, where the action runs as a Vault.
    ///
    /// As [`ActionContext::only_vault`](crate::ActionContext::only_vault): on the
    /// Vault `f` runs and the wrapper holds its value, while on the Workspace `f`
    /// does not run and the wrapper is empty. A value only the Vault holds is
    /// therefore never made up on the side that does not hold it.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::{Account, Member};
    /// use rorolala_protocol::{ActionContext, OnlyVault};
    ///
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// let value = OnlyVault::new(&ctx, || 7_u64);
    /// assert_eq!(value.into_inner(), Some(7));
    ///
    /// // On the Workspace `f` never runs.
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// let empty: OnlyVault<u64> = OnlyVault::new(&ctx, || unreachable!());
    /// assert!(empty.into_inner().is_none());
    /// ```
    #[must_use]
    pub fn new<F>(ctx: &ActionContext, f: F) -> Self
    where
        F: FnOnce() -> Inner,
    {
        if ctx.is_vault() {
            Self::from(Some(f()))
        } else {
            Self::empty()
        }
    }

    /// Returns an empty `OnlyVault` with no inner value.
    #[must_use]
    pub const fn empty() -> Self {
        Self { inner: None }
    }

    /// Consumes the wrapper and returns the inner value.
    pub fn into_inner(self) -> Option<Inner> {
        self.inner
    }

    /// Takes the inner value out, leaving the wrapper empty.
    pub(crate) const fn take(&mut self) -> Option<Inner> {
        self.inner.take()
    }

    /// Returns the contained value or panics with a default message.
    ///
    /// # Panics
    ///
    /// Panics if the contained value is `None`.
    pub fn unwrap(self) -> Inner {
        self.inner.unwrap()
    }

    /// Returns the contained value or the provided fallback.
    pub fn unwrap_or(self, default: Inner) -> Inner {
        self.inner.unwrap_or(default)
    }

    /// Returns the contained value or computes it from the provided closure.
    pub fn unwrap_or_else<F>(self, f: F) -> Inner
    where
        F: FnOnce() -> Inner,
    {
        self.inner.unwrap_or_else(f)
    }

    /// Returns the contained value or the default value of `Inner`.
    pub fn unwrap_or_default(self) -> Inner
    where
        Inner: Default,
    {
        self.inner.unwrap_or_default()
    }

    /// Changes the value on the Vault side, and only there.
    ///
    /// `f` runs only where the action runs as a Vault; on the Workspace the
    /// wrapper holds nothing and nothing is changed, so a value only the Vault
    /// holds is changed only on the side that holds it. Nothing crosses the channel:
    /// [`sync_mut`](crate::OnlyVault::sync_mut) is what carries a change to the other side.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Member;
    /// use rorolala_protocol::{ActionContext, OnlyVault};
    ///
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// let mut value: OnlyVault<u64> = ctx.only_vault(|| 1);
    /// value.mut_on_vault(&ctx, |value| *value += 2);
    /// assert_eq!(value.into_inner(), Some(3));
    /// ```
    pub fn mut_on_vault<F>(&mut self, ctx: &ActionContext, f: F)
    where
        F: FnOnce(&mut Inner),
    {
        if ctx.is_vault()
            && let Some(inner) = self.inner.as_mut()
        {
            f(inner);
        }
    }
}

impl<Inner> From<Option<Inner>> for OnlyVault<Inner>
where
    Inner: Send + Sync + 'static,
{
    fn from(inner: Option<Inner>) -> Self {
        Self { inner }
    }
}

impl<Inner> From<Both<Inner>> for OnlyVault<Inner>
where
    Inner: Send + Sync + 'static,
{
    fn from(both: Both<Inner>) -> Self {
        Self {
            inner: Some(both.into_inner()),
        }
    }
}

impl<Inner> AsRef<Option<Inner>> for OnlyVault<Inner>
where
    Inner: Send + Sync + 'static,
{
    fn as_ref(&self) -> &Option<Inner> {
        &self.inner
    }
}

impl<Inner> std::ops::Deref for OnlyVault<Inner>
where
    Inner: Send + Sync + 'static,
{
    type Target = Option<Inner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A value that only exists on the Workspace side.
///
/// `OnlyWorkspace` is used to represent, at the type level, that a value lives
/// on the Workspace side rather than the Vault side during the execution of an
/// `Action`.
///
/// The wrapper is read through [`Deref`](std::ops::Deref) only; changing the
/// value goes through [`mut_on_workspace`](Self::mut_on_workspace), which changes
/// it on the side that holds it.
#[derive(Debug)]
pub struct OnlyWorkspace<Inner>
where
    Inner: Send + Sync + 'static,
{
    inner: Option<Inner>,
}

impl<Inner> OnlyWorkspace<Inner>
where
    Inner: Send + Sync + 'static,
{
    /// Builds a value from `f`, where the action runs as a Workspace.
    ///
    /// As [`ActionContext::only_workspace`](crate::ActionContext::only_workspace):
    /// on the Workspace `f` runs and the wrapper holds its value, while on the
    /// Vault `f` does not run and the wrapper is empty. A value only the Workspace
    /// holds is therefore never made up on the side that does not hold it.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::{Account, Member};
    /// use rorolala_protocol::{ActionContext, OnlyWorkspace};
    ///
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// let value = OnlyWorkspace::new(&ctx, || 7_u64);
    /// assert_eq!(value.into_inner(), Some(7));
    ///
    /// // On the Vault `f` never runs.
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// let empty: OnlyWorkspace<u64> = OnlyWorkspace::new(&ctx, || unreachable!());
    /// assert!(empty.into_inner().is_none());
    /// ```
    #[must_use]
    pub fn new<F>(ctx: &ActionContext, f: F) -> Self
    where
        F: FnOnce() -> Inner,
    {
        if ctx.is_workspace() {
            Self::from(Some(f()))
        } else {
            Self::empty()
        }
    }

    /// Returns an empty `OnlyWorkspace` with no inner value.
    #[must_use]
    pub const fn empty() -> Self {
        Self { inner: None }
    }

    /// Consumes the wrapper and returns the inner value.
    pub fn into_inner(self) -> Option<Inner> {
        self.inner
    }

    /// Takes the inner value out, leaving the wrapper empty.
    pub(crate) const fn take(&mut self) -> Option<Inner> {
        self.inner.take()
    }

    /// Returns the contained value or panics with a default message.
    ///
    /// # Panics
    ///
    /// Panics if the contained value is `None`.
    pub fn unwrap(self) -> Inner {
        self.inner.unwrap()
    }

    /// Returns the contained value or the provided fallback.
    pub fn unwrap_or(self, default: Inner) -> Inner {
        self.inner.unwrap_or(default)
    }

    /// Returns the contained value or computes it from the provided closure.
    pub fn unwrap_or_else<F>(self, f: F) -> Inner
    where
        F: FnOnce() -> Inner,
    {
        self.inner.unwrap_or_else(f)
    }

    /// Returns the contained value or the default value of `Inner`.
    pub fn unwrap_or_default(self) -> Inner
    where
        Inner: Default,
    {
        self.inner.unwrap_or_default()
    }

    /// Changes the value on the Workspace side, and only there.
    ///
    /// `f` runs only where the action runs as a Workspace; on the Vault the
    /// wrapper holds nothing and nothing is changed, so a value only the
    /// Workspace holds is changed only on the side that holds it. Nothing crosses
    /// the channel: [`sync_mut`](crate::OnlyWorkspace::sync_mut) is what carries a change
    /// to the other side.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Account;
    /// use rorolala_protocol::{ActionContext, OnlyWorkspace};
    ///
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// let mut value: OnlyWorkspace<u64> = ctx.only_workspace(|| 1);
    /// value.mut_on_workspace(&ctx, |value| *value += 2);
    /// assert_eq!(value.into_inner(), Some(3));
    /// ```
    pub fn mut_on_workspace<F>(&mut self, ctx: &ActionContext, f: F)
    where
        F: FnOnce(&mut Inner),
    {
        if ctx.is_workspace()
            && let Some(inner) = self.inner.as_mut()
        {
            f(inner);
        }
    }
}

impl<Inner> From<Option<Inner>> for OnlyWorkspace<Inner>
where
    Inner: Send + Sync + 'static,
{
    fn from(inner: Option<Inner>) -> Self {
        Self { inner }
    }
}

impl<Inner> From<Both<Inner>> for OnlyWorkspace<Inner>
where
    Inner: Send + Sync + 'static,
{
    fn from(both: Both<Inner>) -> Self {
        Self {
            inner: Some(both.into_inner()),
        }
    }
}

impl<Inner> AsRef<Option<Inner>> for OnlyWorkspace<Inner>
where
    Inner: Send + Sync + 'static,
{
    fn as_ref(&self) -> &Option<Inner> {
        &self.inner
    }
}

impl<Inner> std::ops::Deref for OnlyWorkspace<Inner>
where
    Inner: Send + Sync + 'static,
{
    type Target = Option<Inner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
