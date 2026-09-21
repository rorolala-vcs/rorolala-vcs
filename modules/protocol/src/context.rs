use rorolala_auth::{Account, Member};
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
pub struct ActionContext {
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

impl ActionContext {
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
            channel: None,
        }
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
}
