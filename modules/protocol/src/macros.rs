/// Builds a workspace-only value from a closure over the workspace-only values in
/// scope.
///
/// The closure's parameters name the [`OnlyWorkspace`](crate::OnlyWorkspace) values
/// it reads; each is unwrapped inside, so the body works with the plain values. The
/// closure runs only on the Workspace side — on the Vault the receiver is handed a
/// [`only_workspace`](crate::ActionContext::only_workspace) that never calls it, and
/// the result is empty until [`sync`](crate::ActionContext::sync) carries it over.
///
/// The context comes first because a `macro_rules!` body cannot reach a local
/// variable of the caller: `ctx` has to be passed in.
///
/// # Examples
///
/// ```
/// use rorolala_auth::Account;
/// use rorolala_protocol::{ActionContext, on_workspace};
///
/// let ctx = ActionContext::new_workspace_ctx(Account::default());
/// let num = ctx.only_workspace(|| 5);
/// let result = on_workspace!(ctx, |num| num + 2);
/// assert_eq!(result.into_inner(), Some(7));
/// ```
#[macro_export]
macro_rules! on_workspace {
    ($ctx:expr, |$($param:ident),* $(,)?| $($body:tt)*) => {
        $ctx.only_workspace(|| {
            $(
                let $param = $param.unwrap();
            )*
            #[allow(unused_braces)]
            { $($body)* }
        })
    };
}

/// Builds a vault-only value from a closure over the vault-only values in scope.
///
/// The counterpart of [`on_workspace!`]: the parameters name the
/// [`OnlyVault`](crate::OnlyVault) values the body reads, and the closure runs only
/// on the Vault side.
///
/// # Examples
///
/// ```
/// use rorolala_auth::Member;
/// use rorolala_protocol::{ActionContext, on_vault};
///
/// let ctx = ActionContext::new_vault_ctx(Member::default());
/// let num = ctx.only_vault(|| 5);
/// let result = on_vault!(ctx, |num| num + 2);
/// assert_eq!(result.into_inner(), Some(7));
/// ```
#[macro_export]
macro_rules! on_vault {
    ($ctx:expr, |$($param:ident),* $(,)?| $($body:tt)*) => {
        $ctx.only_vault(|| {
            $(
                let $param = $param.unwrap();
            )*
            #[allow(unused_braces)]
            { $($body)* }
        })
    };
}
