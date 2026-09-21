use rorolala_protocol::{Action, ActionContext, ActionError, OnlyVault, OnlyWorkspace, Transfer};

/// An Action used for handshake interaction with the server.
///
/// This Action takes a `String` as input and builds a greeting message that both
/// sides hold by the time it returns. The Workspace's input is first sent to the
/// Vault, which rewrites it into `"Hello, {input}"`. The Vault then also appends
/// its own welcome response, producing a final string of the form
/// `"Hello, {input} ... {response}"`.
///
/// # Input
/// * `String` - The initial string that the Workspace holds and that needs to be
///   sent to the server and processed.
///
/// # Output
/// * `String` - The string rewritten to `"Hello, {input} ... {response}"`, where
///   `{response}` is the Vault's welcome message.
///
/// # Behavior Flow
/// 1. Start from a value only the Workspace holds.
/// 2. Hand the input to `transfer`, so the Vault receives the Workspace's value and
///    both sides end up holding the same message.
/// 3. Have the Vault provide its welcome response, held on both sides.
/// 4. Use `sync` so the Workspace receives the Vault's response and both sides hold
///    it.
/// 5. Return the response string.
///
/// # Errors
/// If the exchange fails, the corresponding [`ActionError`] is returned.
pub struct ActionHandshake;

impl Action for ActionHandshake {
    /// The handshake is id 0: it is what a peer speaks first, before it can ask for
    /// anything else.
    const ID: u32 = 0;

    type Input = String;
    type Output = String;

    async fn process(
        name: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // Obtain the Vault's information
        let vault_cfg = ctx.current_vault_config();
        let vault_name_and_desc = ctx.only_vault(|| {
            let vault_cfg = vault_cfg.unwrap();
            (
                vault_cfg.vault_config().name().to_owned(),
                vault_cfg.vault_config().description().to_owned(),
            )
        });

        // Obtain the Name input by the Workspace
        let user_name = name.transfer(&mut ctx).await?;

        // Compose the reply message
        let response = OnlyVault::new(&ctx, || {
            // UNWRAP: The following types are all OnlyVault, and can be safely unwrapped in the Vault branch
            let (name, desc) = vault_name_and_desc.unwrap();
            let user_name = user_name.unwrap();

            format!("Hello, {user_name}, I'm {name}.\n\n{desc}")
        });

        // Sync the reply message
        let response = ctx.sync(response).await?;

        // Unwrap the message and return it
        Ok(response.into_inner())
    }
}
