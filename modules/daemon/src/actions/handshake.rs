use rorolala_protocol::{Action, ActionContext, ActionError, OnlyVault, OnlyWorkspace};

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
/// 2. Hand the input to `sync`, so the Vault receives the Workspace's value and
///    both sides end up holding the same message.
/// 3. Have the Vault provide its welcome response, held on both sides.
/// 4. Use `sync_mut_with`: the Workspace rewrites the message by formatting it as
///    `"Hello, {raw} ... {response}"`, and the Vault takes what it produced.
/// 5. Return the rewritten string.
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
        // Turn the Workspace's input into something both sides hold
        let mut message = ctx.sync(name).await?;

        // The server responds
        let response = OnlyVault::new(&ctx, || "Welcome!".to_string());

        // Append the server's response to the message and have both sides sync
        message
            .sync_mut_with(&mut ctx, response, |raw, response| {
                *raw = format!("Hello, {raw} ... {response}");
            })
            .await?;

        // Unwrap the message and return it
        Ok(message.into_inner())
    }
}
