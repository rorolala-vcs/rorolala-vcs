//! Why the daemon stopped, and what a caller that started it has to branch on.

use rorolala_utils_lazyffi::lazyffi;

/// Why the daemon stopped.
///
/// A daemon is started to serve and stopped by hand, so `0` is the ordinary end. Anything else
/// is a reason it could not serve at all, which is what a caller that started it — rather than a
/// person who pressed Ctrl-C — has to branch on.
pub const STOPPED: u8 = 0;

/// The daemon found no account to prove itself with.
pub const NO_IDENTITY: u8 = 1;

/// The namespace table could not be read.
pub const REGISTRY_UNREADABLE: u8 = 2;

/// The port could not be bound.
pub const PORT_TAKEN: u8 = 3;

/// Structure information for when the daemon exits
#[lazyffi(export = RolaDaemonExit)]
#[derive(Default)]
pub struct DaemonExit {
    /// Why it stopped: one of the constants beside this type.
    code: u8,
}

impl DaemonExit {
    /// The daemon that stopped for `code`.
    #[must_use]
    pub const fn with_code(code: u8) -> Self {
        Self { code }
    }

    /// Why the daemon stopped.
    #[must_use]
    pub const fn code(&self) -> u8 {
        self.code
    }
}
