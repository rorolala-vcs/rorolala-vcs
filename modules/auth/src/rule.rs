use rorolala_utils_lazyffi::lazyffi;

/// Which key directories a search may look in, and therefore of what scope.
///
/// Every flag is on by default, so a caller that wants everything has nothing to say.
/// The flags are independent: turning one off is how a caller narrows a search without
/// knowing where the keys actually live.
///
/// The three non-local flags name where a **public** key — and so a member — may be
/// found. A private key is not shared, so an account is only ever looked for in the local
/// scopes, whatever the others say.
///
/// The fields are public to Rust, but C cannot see inside an exported `struct`, so the
/// methods below are what reads and writes a rule across the boundary. Four independent
/// switches are the whole of the rule, which is why it is a struct of flags and not a
/// choice among them.
#[allow(clippy::struct_excessive_bools)]
#[lazyffi(export = RolaKeyLocateRule)]
#[derive(Debug, Clone)]
pub struct KeyLocateRule {
    /// Search the public keys under the filesystem root (`/.rola/keys`).
    pub find_global: bool,
    /// Search the keys beside the current Workspace and Vault
    ///
    /// This is the only scope an account is ever looked for in, since a private key is
    /// not shared.
    pub find_local: bool,
    /// Search the public keys under the user's local data directory
    /// (`~/.local/share/rola/keys`).
    pub find_user: bool,
    /// Search the public keys under `ROLA_HOME` (`$ROLA_HOME/keys`).
    pub find_env: bool,
}

impl Default for KeyLocateRule {
    fn default() -> Self {
        Self::new()
    }
}

#[lazyffi]
impl KeyLocateRule {
    /// A rule that searches every directory.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            find_global: true,
            find_local: true,
            find_user: true,
            find_env: true,
        }
    }

    /// Whether the filesystem-root keys are searched.
    #[must_use]
    pub const fn find_global(&self) -> bool {
        self.find_global
    }

    /// Turns the filesystem-root keys on or off.
    pub const fn set_find_global(&mut self, value: bool) {
        self.find_global = value;
    }

    /// Whether the keys beside the current Vault are searched.
    #[must_use]
    pub const fn find_local(&self) -> bool {
        self.find_local
    }

    /// Turns the keys beside the current Vault on or off.
    pub const fn set_find_local(&mut self, value: bool) {
        self.find_local = value;
    }

    /// Whether the user's local data keys are searched.
    #[must_use]
    pub const fn find_user(&self) -> bool {
        self.find_user
    }

    /// Turns the user's local data keys on or off.
    pub const fn set_find_user(&mut self, value: bool) {
        self.find_user = value;
    }

    /// Whether the keys named by `ROLA_HOME` are searched.
    #[must_use]
    pub const fn find_env(&self) -> bool {
        self.find_env
    }

    /// Turns the keys named by `ROLA_HOME` on or off.
    pub const fn set_find_env(&mut self, value: bool) {
        self.find_env = value;
    }
}
