use std::future::Future;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::{ActionContext, ActionError, Both, Channel, Encodable, OnlyVault, OnlyWorkspace};

impl ActionContext {
    /// Builds a value only the Vault side holds.
    ///
    /// This side decides: on the Vault the closure runs and the wrapper holds its
    /// value, while on the Workspace the closure does not run and the wrapper is
    /// empty — the value reaches the Workspace through [`sync`](Self::sync). The
    /// closure is handed this context, so what it builds can depend on the side.
    pub fn only_vault_with_context<Inner, F>(&mut self, f: F) -> OnlyVault<Inner>
    where
        Inner: Encodable + Send + Sync + 'static,
        F: FnOnce(&mut Self) -> Inner,
    {
        if self.is_vault() {
            OnlyVault::from(Some(f(self)))
        } else {
            OnlyVault::empty()
        }
    }

    /// Builds a value only the Workspace side holds.
    ///
    /// This side decides: on the Workspace the closure runs and the wrapper holds
    /// its value, while on the Vault the closure does not run and the wrapper is
    /// empty — the value reaches the Vault through [`sync`](Self::sync). The closure
    /// is handed this context, so what it builds can depend on the side.
    pub fn only_workspace_with_context<Inner, F>(&mut self, f: F) -> OnlyWorkspace<Inner>
    where
        Inner: Encodable + Send + Sync + 'static,
        F: FnOnce(&mut Self) -> Inner,
    {
        if self.is_workspace() {
            OnlyWorkspace::from(Some(f(self)))
        } else {
            OnlyWorkspace::empty()
        }
    }

    /// Builds a value only the Vault side holds.
    ///
    /// As [`only_vault_with_context`](Self::only_vault_with_context), but the closure
    /// takes no context.
    pub fn only_vault<Inner, F>(&self, f: F) -> OnlyVault<Inner>
    where
        Inner: Encodable + Send + Sync + 'static,
        F: FnOnce() -> Inner,
    {
        if self.is_vault() {
            OnlyVault::from(Some(f()))
        } else {
            OnlyVault::empty()
        }
    }

    /// Builds a value only the Workspace side holds.
    ///
    /// As [`only_workspace_with_context`](Self::only_workspace_with_context), but the
    /// closure takes no context.
    pub fn only_workspace<Inner, F>(&self, f: F) -> OnlyWorkspace<Inner>
    where
        Inner: Encodable + Send + Sync + 'static,
        F: FnOnce() -> Inner,
    {
        if self.is_workspace() {
            OnlyWorkspace::from(Some(f()))
        } else {
            OnlyWorkspace::empty()
        }
    }

    /// Exchanges `data` with the peer, returning it as a [`Both`] once both sides
    /// hold it.
    ///
    /// Which direction the value travels is [`data`](DataSync)'s to decide, not the
    /// caller's: the wrapper names the side that owns the value, that side sends it,
    /// and the other side receives it. Both sides therefore call this for the same
    /// value at the same time, and the byte the exchange spends is the value itself.
    ///
    /// The [`Both`] that comes back is read-only; changing it means handing it to one
    /// side with [`Both::only_workspace`] or [`Both::only_vault`], and changing it
    /// there with [`sync_mut`](Both::sync_mut_with), which carries the result to the
    /// other side.
    ///
    /// # Errors
    ///
    /// Returns whatever the exchange fails with — see [`ActionError`]. In
    /// particular, a context with no channel reports
    /// [`NoChannel`](ActionError::NoChannel).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let seed: OnlyWorkspace<u64> = context.only_workspace(|| 7);
    /// let seed: Both<u64> = context.sync(seed).await?;
    /// ```
    pub async fn sync<Type, Data>(&mut self, mut data: Data) -> Result<Both<Type>, Data::Error>
    where
        Data: DataSync<Type>,
    {
        let value = if self.is_workspace() {
            data.do_workspace(self).await?
        } else {
            data.do_vault(self).await?
        };

        Ok(Both::new(value))
    }
}

/// A trait for values that can be requested from both a workspace and a vault.
///
/// Implementors provide two pieces of behavior: one that runs on / for the
/// workspace side and one that runs on / for the vault side. Both return a value
/// of the same `Type`.
pub trait DataSync<Type> {
    /// The error type returned when the operation fails.
    type Error;

    /// Returns the value for the workspace side.
    fn do_workspace(
        &mut self,
        context: &mut ActionContext,
    ) -> impl Future<Output = Result<Type, Self::Error>> + Send;

    /// Returns the value for the vault side.
    fn do_vault(
        &mut self,
        context: &mut ActionContext,
    ) -> impl Future<Output = Result<Type, Self::Error>>;
}

impl<Inner> DataSync<Inner> for OnlyVault<Inner>
where
    Inner: Encodable + Send + Sync + 'static,
{
    type Error = ActionError;

    async fn do_workspace(&mut self, context: &mut ActionContext) -> Result<Inner, Self::Error> {
        let channel = context.channel_mut().ok_or(ActionError::NoChannel)?;
        receive_value(channel).await
    }

    async fn do_vault(&mut self, context: &mut ActionContext) -> Result<Inner, Self::Error> {
        let value = self.take().ok_or(ActionError::MissingValue)?;
        let channel = context.channel_mut().ok_or(ActionError::NoChannel)?;
        send_value(channel, &value).await?;
        Ok(value)
    }
}

impl<Inner> DataSync<Inner> for OnlyWorkspace<Inner>
where
    Inner: Encodable + Send + Sync + 'static,
{
    type Error = ActionError;

    async fn do_workspace(&mut self, context: &mut ActionContext) -> Result<Inner, Self::Error> {
        let value = self.take().ok_or(ActionError::MissingValue)?;
        let channel = context.channel_mut().ok_or(ActionError::NoChannel)?;
        send_value(channel, &value).await?;
        Ok(value)
    }

    async fn do_vault(&mut self, context: &mut ActionContext) -> Result<Inner, Self::Error> {
        let channel = context.channel_mut().ok_or(ActionError::NoChannel)?;
        receive_value(channel).await
    }
}

/// The longest value one frame will carry.
///
/// A frame states its length in four bytes, so a peer could otherwise ask for
/// four gigabytes to be allocated before sending any of it.
const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Frames `value` and writes it to `channel`.
pub(crate) async fn send_value<Inner>(
    channel: &mut Channel,
    value: &Inner,
) -> Result<(), ActionError>
where
    Inner: Encodable + Sync,
{
    let bytes = value.encode()?;
    if bytes.len() > MAX_FRAME {
        return Err(ActionError::ValueTooLarge);
    }
    let length = u32::try_from(bytes.len()).map_err(|_| ActionError::ValueTooLarge)?;

    channel.write_all(&length.to_be_bytes()).await?;
    channel.write_all(&bytes).await?;
    channel.flush().await?;

    Ok(())
}

/// Reads one frame from `channel` and decodes it.
pub(crate) async fn receive_value<Inner>(channel: &mut Channel) -> Result<Inner, ActionError>
where
    Inner: Encodable,
{
    let mut header = [0_u8; 4];
    channel.read_exact(&mut header).await?;

    let length =
        usize::try_from(u32::from_be_bytes(header)).map_err(|_| ActionError::ValueTooLarge)?;
    if length > MAX_FRAME {
        return Err(ActionError::ValueTooLarge);
    }

    let mut bytes = vec![0_u8; length];
    channel.read_exact(&mut bytes).await?;

    Ok(Inner::decode(bytes)?)
}

#[cfg(test)]
mod tests {
    use rorolala_auth::{Account, KeyAlgorithm, Member, SecureStream, SigningKey};
    use tokio::io::duplex;

    use crate::{ActionContext, Channel, Socket};

    /// A signing key from a fixed seed, so a test names the same identity twice.
    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(KeyAlgorithm::Ed25519, [seed; 32]).expect("a seed names a key")
    }

    /// A client and a server session, handshaken over an in-memory socket.
    async fn session_pair() -> (Channel, Channel) {
        let client = key(1);
        let server = key(2);
        let server_key = server.public_key();

        let (client_socket, server_socket) = duplex(8 * 1024);
        let client_socket: Box<dyn Socket> = Box::new(client_socket);
        let server_socket: Box<dyn Socket> = Box::new(server_socket);

        let (client, server) = tokio::join!(
            SecureStream::connect(client_socket, &client, &server_key),
            SecureStream::accept(server_socket, &server),
        );

        (
            client.expect("the client handshakes"),
            server.expect("the server handshakes"),
        )
    }

    #[tokio::test]
    async fn a_synced_value_crosses_the_encrypted_channel() {
        let (client, server) = session_pair().await;

        let mut workspace =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(client);
        let mut vault = ActionContext::new_vault_ctx(Member::default()).with_channel(server);

        // A value only the Workspace holds: it sends, the Vault receives.
        let sent = workspace.only_workspace(|| 7_u64);
        let empty = vault.only_workspace(|| 0_u64);
        let (workspace_result, vault_result) =
            tokio::join!(workspace.sync(sent), vault.sync(empty));
        assert_eq!(*workspace_result.unwrap(), 7);
        assert_eq!(*vault_result.unwrap(), 7);

        // A value only the Vault holds: it sends, the Workspace receives.
        let empty = workspace.only_vault(|| 0_u64);
        let sent = vault.only_vault(|| 9_u64);
        let (workspace_result, vault_result) =
            tokio::join!(workspace.sync(empty), vault.sync(sent));
        assert_eq!(*workspace_result.unwrap(), 9);
        assert_eq!(*vault_result.unwrap(), 9);
    }

    #[tokio::test]
    async fn a_change_on_the_vault_reaches_the_workspace() {
        let (client, server) = session_pair().await;

        let mut workspace =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(client);
        let mut vault = ActionContext::new_vault_ctx(Member::default()).with_channel(server);

        // Both sides come to hold the same value.
        let sent = workspace.only_workspace(|| 1_i32);
        let empty = vault.only_workspace(|| 0_i32);
        let (workspace_result, vault_result) =
            tokio::join!(workspace.sync(sent), vault.sync(empty));
        let mut on_workspace = workspace_result.unwrap();
        let mut on_vault = vault_result.unwrap();
        assert_eq!(*on_workspace, 1);
        assert_eq!(*on_vault, 1);

        // The input is a value only the Vault holds, so the Vault is the side that runs
        // the closure; the Workspace takes what it produced and never runs its own.
        let input = vault.only_vault(|| 7_i32);
        let empty_input = workspace.only_vault(|| 0_i32);
        let (vault_result, workspace_result) = tokio::join!(
            on_vault.sync_mut_with(&mut vault, input, |value: &mut i32, input: i32| {
                *value = input;
            }),
            on_workspace.sync_mut_with(&mut workspace, empty_input, |_: &mut i32, _: i32| {
                unreachable!();
            }),
        );
        vault_result.unwrap();
        workspace_result.unwrap();

        assert_eq!(*on_vault, 7);
        assert_eq!(*on_workspace, 7);
    }

    #[tokio::test]
    async fn a_change_from_two_workspace_inputs_reaches_the_vault() {
        let (client, server) = session_pair().await;

        let mut workspace =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(client);
        let mut vault = ActionContext::new_vault_ctx(Member::default()).with_channel(server);

        // Both sides come to hold the same value.
        let sent = workspace.only_workspace(|| 0_i32);
        let empty = vault.only_workspace(|| 0_i32);
        let (workspace_result, vault_result) =
            tokio::join!(workspace.sync(sent), vault.sync(empty));
        let mut on_workspace = workspace_result.unwrap();
        let mut on_vault = vault_result.unwrap();

        // Two inputs only the Workspace holds, so the Workspace runs the closure; the
        // Vault takes what it produced, its own empty inputs never read.
        let first = workspace.only_workspace(|| 10_i32);
        let second = workspace.only_workspace(|| 20_i32);
        let empty_first = vault.only_workspace(|| 0_i32);
        let empty_second = vault.only_workspace(|| 0_i32);
        let (workspace_result, vault_result) = tokio::join!(
            on_workspace.sync_mut_with(
                &mut workspace,
                (first, second),
                |value: &mut i32, (first, second): (i32, i32)| {
                    *value = first + second;
                }
            ),
            on_vault.sync_mut_with(
                &mut vault,
                (empty_first, empty_second),
                |_: &mut i32, (_first, _second): (i32, i32)| {
                    unreachable!();
                }
            ),
        );
        workspace_result.unwrap();
        vault_result.unwrap();

        assert_eq!(*on_workspace, 30);
        assert_eq!(*on_vault, 30);
    }

    #[tokio::test]
    async fn a_value_changed_on_its_own_side_comes_back_as_both() {
        let (client, server) = session_pair().await;

        let mut workspace =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(client);
        let mut vault = ActionContext::new_vault_ctx(Member::default()).with_channel(server);

        // Only the Workspace holds the value; the Vault's wrapper is empty, which is how
        // it is built on that side.
        let on_workspace = workspace.only_workspace(|| "world".to_owned());
        let on_vault = vault.only_workspace(String::new);

        let (workspace_result, vault_result) = tokio::join!(
            on_workspace.sync_mut(&mut workspace, |raw| *raw = format!("Hello, {raw}")),
            on_vault.sync_mut(&mut vault, |_| unreachable!()),
        );
        let from_workspace = workspace_result.unwrap();
        let from_vault = vault_result.unwrap();

        // Both sides come out holding the rewritten string, and the Vault never ran the
        // Workspace's closure.
        assert_eq!(*from_workspace, "Hello, world");
        assert_eq!(*from_vault, "Hello, world");
    }

    #[tokio::test]
    async fn a_demoted_value_is_changed_on_one_side_alone() {
        let (client, server) = session_pair().await;

        let mut workspace =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(client);
        let mut vault = ActionContext::new_vault_ctx(Member::default()).with_channel(server);

        // Both sides come to hold the same value.
        let sent = workspace.only_workspace(|| 1_u64);
        let empty = vault.only_workspace(|| 0_u64);
        let (workspace_result, vault_result) =
            tokio::join!(workspace.sync(sent), vault.sync(empty));
        let workspace_value = workspace_result.unwrap();
        let vault_value = vault_result.unwrap();
        assert_eq!(*workspace_value, 1);
        assert_eq!(*vault_value, 1);

        // The Workspace demotes its copy and changes that, without the Vault being
        // asked: the two have stopped keeping this value in step, so the Vault's
        // copy is left as it was.
        let mut demoted = workspace_value.only_workspace();
        demoted.mut_on_workspace(&workspace, |value| *value += 10);
        assert_eq!(demoted.into_inner(), Some(11));
        assert_eq!(*vault_value, 1);
    }
}
