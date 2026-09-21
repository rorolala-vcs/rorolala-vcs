use rorolala_auth::{Account, Member};
use rorolala_vault::{Config as VaultConfig, RootVault, Vault};
use rorolala_workspace::{Config as WorkspaceConfig, Workspace};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{OnlyVault, OnlyWorkspace};

/// The socket an encrypted [`Channel`] carries its records over.
///
/// This is the transport underneath a channel, reduced to the traits the protocol
/// needs so a channel is not generic over it. It is never read or written directly:
/// the channel seals and opens everything that crosses it.
pub trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> Socket for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

/// The encrypted session an action's two sides exchange values over.
///
/// This is auth's [`SecureStream`](rorolala_auth::SecureStream), already past its
/// handshake, over a type-erased [`Socket`]. An [`ActionContext`] accepts nothing
/// else, so every value the protocol exchanges crosses an encrypted channel: a
/// plain stream has no way in.
pub type Channel = rorolala_auth::SecureStream<Box<dyn Socket>>;

/// Action context, used in client-server interaction.
///
/// The lifetime is that of the local things the action is handed: the Workspace it is taken
/// from and the Vault it is taken against, and what either is configured with. They belong to
/// whoever built the context and outlive the action, so the context borrows them rather than
/// taking copies.
pub struct ActionContext<'a> {
    /// The side on which the action occurs.
    side: ActionSide,

    /// The member the action runs as.
    ///
    /// A member is the Vault's record of whoever is acting, so it is held on the Vault
    /// side: the wrapper is empty wherever the action runs as a Workspace, and a value
    /// that only the Vault holds is not made up on the side that does not.
    member: OnlyVault<Member>,

    /// The account the action runs as.
    ///
    /// An account is private, so it is held on the Workspace side: the wrapper is empty
    /// wherever the action runs as a Vault, and a value that only the Workspace holds is
    /// not made up on the side that does not.
    account: OnlyWorkspace<Account>,

    /// The Workspace the action is being taken from.
    ///
    /// The Workspace is the side the action runs on, so what holds it is the Workspace: the
    /// wrapper is empty wherever the action runs as a Vault, which is the other side of the
    /// channel and has no Workspace to hand.
    current_workspace: OnlyWorkspace<&'a Workspace>,

    /// The Vault the action is being taken against.
    ///
    /// The Vault is the other side, so what holds it is the Vault: the wrapper is empty
    /// wherever the action runs as a Workspace.
    current_vault: OnlyVault<&'a Vault>,

    /// The outermost Vault above [`current_vault`](Self::current_vault).
    ///
    /// A Vault sits under a root that holds it — see [`RootVault`] — and a Vault-side
    /// action that works on the Vaults below it is handed that root rather than looking for
    /// it again. It is held on the Vault side, for the reason the Vault is.
    current_root_vault: OnlyVault<&'a RootVault>,

    /// The configuration the Workspace works from.
    ///
    /// It is the Workspace's own, so it is held on the Workspace side, for the reason the
    /// Workspace is.
    current_workspace_config: OnlyWorkspace<&'a WorkspaceConfig>,

    /// The configuration the Vault is served from.
    ///
    /// It is the Vault's own, so it is held on the Vault side, for the reason the Vault is.
    current_vault_config: OnlyVault<&'a VaultConfig>,

    /// The configuration the root above [`current_vault`](Self::current_vault) is served
    /// from.
    ///
    /// As [`current_root_vault`](Self::current_root_vault): a Vault-side action that works on
    /// the Vaults below a root is handed what that root was configured with.
    current_root_vault_config: OnlyVault<&'a VaultConfig>,

    /// The encrypted channel values are exchanged over, once one has been attached.
    channel: Option<Channel>,
}

/// The side on which the action occurs.
#[derive(Clone, Copy)]
pub enum ActionSide {
    /// The Workspace side.
    Workspace,
    /// The Vault side.
    Vault,
}

impl<'a> ActionContext<'a> {
    /// Returns the side on which the action occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Account;
    /// use rorolala_protocol::{ActionContext, ActionSide};
    ///
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// assert!(matches!(ctx.side(), ActionSide::Workspace));
    /// ```
    #[must_use]
    pub const fn side(&self) -> ActionSide {
        self.side
    }

    /// Returns `true` if the action occurs on the Vault side.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Member;
    /// use rorolala_protocol::ActionContext;
    ///
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// assert!(ctx.is_vault());
    /// ```
    #[must_use]
    pub const fn is_vault(&self) -> bool {
        matches!(self.side, ActionSide::Vault)
    }

    /// Returns `true` if the action occurs on the Workspace side.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Account;
    /// use rorolala_protocol::ActionContext;
    ///
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// assert!(ctx.is_workspace());
    /// ```
    #[must_use]
    pub const fn is_workspace(&self) -> bool {
        matches!(self.side, ActionSide::Workspace)
    }

    /// Creates a new `ActionContext` for the Workspace side.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Account;
    /// use rorolala_protocol::ActionContext;
    ///
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// assert!(ctx.is_workspace());
    /// ```
    #[must_use]
    pub const fn new_workspace_ctx(account: Account) -> Self {
        Self {
            side: ActionSide::Workspace,
            member: OnlyVault::empty(),
            account: OnlyWorkspace::holding(account),
            current_workspace: OnlyWorkspace::empty(),
            current_vault: OnlyVault::empty(),
            current_root_vault: OnlyVault::empty(),
            current_workspace_config: OnlyWorkspace::empty(),
            current_vault_config: OnlyVault::empty(),
            current_root_vault_config: OnlyVault::empty(),
            channel: None,
        }
    }

    /// Creates a new `ActionContext` for the Vault side.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Member;
    /// use rorolala_protocol::ActionContext;
    ///
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// assert!(ctx.is_vault());
    /// ```
    #[must_use]
    pub const fn new_vault_ctx(member: Member) -> Self {
        Self {
            side: ActionSide::Vault,
            member: OnlyVault::holding(member),
            account: OnlyWorkspace::empty(),
            current_workspace: OnlyWorkspace::empty(),
            current_vault: OnlyVault::empty(),
            current_root_vault: OnlyVault::empty(),
            current_workspace_config: OnlyWorkspace::empty(),
            current_vault_config: OnlyVault::empty(),
            current_root_vault_config: OnlyVault::empty(),
            channel: None,
        }
    }

    /// Attaches the Workspace this action is being taken from.
    ///
    /// The Workspace handed over has to outlive the context, which it does: the caller runs
    /// the action against a Workspace it already holds.
    #[must_use]
    pub const fn with_current_workspace(mut self, workspace: &'a Workspace) -> Self {
        self.current_workspace = OnlyWorkspace::holding(workspace);
        self
    }

    /// Attaches the Vault this action is being taken against.
    ///
    /// The Vault handed over has to outlive the context, which it does: the caller runs the
    /// action against a Vault it already holds.
    #[must_use]
    pub const fn with_current_vault(mut self, vault: &'a Vault) -> Self {
        self.current_vault = OnlyVault::holding(vault);
        self
    }

    /// Attaches the root of the Vault this action is being taken against.
    ///
    /// As [`with_current_vault`](Self::with_current_vault): an action that works on the
    /// Vaults below this one is handed the root that holds them.
    #[must_use]
    pub const fn with_current_root_vault(mut self, root_vault: &'a RootVault) -> Self {
        self.current_root_vault = OnlyVault::holding(root_vault);
        self
    }

    /// Attaches the configuration the Workspace works from.
    ///
    /// As [`with_current_workspace`](Self::with_current_workspace): a Workspace's own
    /// configuration outlives the action, since the caller works from one it already holds.
    #[must_use]
    pub const fn with_current_workspace_config(
        mut self,
        workspace_config: &'a WorkspaceConfig,
    ) -> Self {
        self.current_workspace_config = OnlyWorkspace::holding(workspace_config);
        self
    }

    /// Attaches the configuration the Vault is served from.
    ///
    /// As [`with_current_vault`](Self::with_current_vault).
    #[must_use]
    pub const fn with_current_vault_config(mut self, vault_config: &'a VaultConfig) -> Self {
        self.current_vault_config = OnlyVault::holding(vault_config);
        self
    }

    /// Attaches the configuration the root of the Vault is served from.
    ///
    /// As [`with_current_root_vault`](Self::with_current_root_vault).
    #[must_use]
    pub const fn with_current_root_vault_config(
        mut self,
        root_vault_config: &'a VaultConfig,
    ) -> Self {
        self.current_root_vault_config = OnlyVault::holding(root_vault_config);
        self
    }

    /// Attaches `channel` so synced values can cross to the peer.
    ///
    /// A context without a channel runs actions that keep to one side; one that
    /// syncs a value needs this, and reports
    /// [`NoChannel`](crate::ActionError::NoChannel) without it. Only a
    /// handshake-finished [`Channel`] fits, so what crosses is always encrypted.
    #[must_use]
    pub fn with_channel(mut self, channel: Channel) -> Self {
        self.channel = Some(channel);
        self
    }

    /// The encrypted channel values are exchanged over, if one has been attached.
    pub(crate) fn channel_mut(&mut self) -> Option<&mut Channel> {
        self.channel.as_mut()
    }

    /// The account the current action runs as, on the Workspace side.
    ///
    /// An account is private, so it exists only where the action is taken by a
    /// Workspace; a context on the Vault side yields an empty [`OnlyWorkspace`].
    /// The account is handed out as a clone, so the context keeps its own copy.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Account;
    /// use rorolala_protocol::ActionContext;
    ///
    /// let ctx = ActionContext::new_workspace_ctx(Account::default());
    /// assert_eq!(ctx.get_account().into_inner(), Some(Account::default()));
    /// ```
    #[must_use]
    pub fn get_account(&self) -> OnlyWorkspace<Account> {
        self.account.clone()
    }

    /// The member the current action runs as, on the Vault side.
    ///
    /// A member is the Vault's record of whoever is acting, so it exists only where
    /// the action is taken by a Vault; a context on the Workspace side yields an
    /// empty [`OnlyVault`]. The member is handed out as a clone, so the context
    /// keeps its own copy.
    ///
    /// # Examples
    ///
    /// ```
    /// use rorolala_auth::Member;
    /// use rorolala_protocol::ActionContext;
    ///
    /// let ctx = ActionContext::new_vault_ctx(Member::default());
    /// assert_eq!(ctx.get_member().into_inner(), Some(Member::default()));
    /// ```
    #[must_use]
    pub fn get_member(&self) -> OnlyVault<Member> {
        self.member.clone()
    }

    /// The Workspace the current action is taken from, on the Workspace side.
    ///
    /// A Workspace is the side the action runs on, so it exists only where the action is
    /// taken by a Workspace; a context on the Vault side yields an empty [`OnlyWorkspace`].
    #[must_use]
    pub fn current_workspace(&self) -> OnlyWorkspace<&'a Workspace> {
        self.current_workspace.clone()
    }

    /// The Vault the current action runs against, on the Vault side.
    ///
    /// A Vault is the other side's own, so it exists only where the action is taken by a
    /// Vault; a context on the Workspace side yields an empty [`OnlyVault`].
    #[must_use]
    pub fn current_vault(&self) -> OnlyVault<&'a Vault> {
        self.current_vault.clone()
    }

    /// The root above the Vault the current action runs against, on the Vault side.
    ///
    /// As [`current_vault`](Self::current_vault): it exists only where the action is taken
    /// by a Vault.
    #[must_use]
    pub fn current_root_vault(&self) -> OnlyVault<&'a RootVault> {
        self.current_root_vault.clone()
    }

    /// The configuration the Workspace the current action is taken from works from, on the
    /// Workspace side.
    ///
    /// As [`current_workspace`](Self::current_workspace): a Workspace's own configuration
    /// exists only where the action is taken by a Workspace.
    #[must_use]
    pub fn current_workspace_config(&self) -> OnlyWorkspace<&'a WorkspaceConfig> {
        self.current_workspace_config.clone()
    }

    /// The configuration the Vault the current action runs against is served from, on the
    /// Vault side.
    ///
    /// As [`current_vault`](Self::current_vault): a Vault's own configuration exists only
    /// where the action is taken by a Vault.
    #[must_use]
    pub fn current_vault_config(&self) -> OnlyVault<&'a VaultConfig> {
        self.current_vault_config.clone()
    }

    /// The configuration the root above the Vault the current action runs against is served
    /// from, on the Vault side.
    ///
    /// As [`current_root_vault`](Self::current_root_vault): it exists only where the action is
    /// taken by a Vault.
    #[must_use]
    pub fn current_root_vault_config(&self) -> OnlyVault<&'a VaultConfig> {
        self.current_root_vault_config.clone()
    }
}

#[cfg(test)]
mod tests {
    use rorolala_auth::{Account, Member};
    use rorolala_vault::{Config as VaultConfig, RootVault, Vault};
    use rorolala_workspace::{Config as WorkspaceConfig, Workspace};

    use super::ActionContext;

    #[test]
    fn a_context_keeps_the_local_side_it_was_handed_and_only_there() {
        let vault = Vault::default();
        let root = RootVault::default();
        let workspace = Workspace::default();
        let vault_config = VaultConfig::default();
        let workspace_config = WorkspaceConfig::default();

        let ctx = ActionContext::new_vault_ctx(Member::default())
            .with_current_vault(&vault)
            .with_current_root_vault(&root)
            .with_current_vault_config(&vault_config)
            .with_current_root_vault_config(&vault_config);

        assert!(ctx.current_vault().into_inner().is_some());
        assert!(ctx.current_root_vault().into_inner().is_some());
        assert!(ctx.current_vault_config().into_inner().is_some());
        assert!(ctx.current_root_vault_config().into_inner().is_some());

        // The Workspace side has no Vault to hand, so there is nothing there to reach for —
        // and a Vault context that was handed none is empty the same way.
        let ctx = ActionContext::new_workspace_ctx(Account::default());
        assert!(ctx.current_vault().into_inner().is_none());
        assert!(ctx.current_root_vault().into_inner().is_none());
        assert!(ctx.current_vault_config().into_inner().is_none());
        assert!(ctx.current_root_vault_config().into_inner().is_none());

        // The other way round for the Workspace the action is taken from.
        let ctx = ActionContext::new_workspace_ctx(Account::default())
            .with_current_workspace(&workspace)
            .with_current_workspace_config(&workspace_config);
        assert!(ctx.current_workspace().into_inner().is_some());
        assert!(ctx.current_workspace_config().into_inner().is_some());

        let ctx = ActionContext::new_vault_ctx(Member::default());
        assert!(ctx.current_workspace().into_inner().is_none());
        assert!(ctx.current_workspace_config().into_inner().is_none());
    }
}
