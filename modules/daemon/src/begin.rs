// This is a placeholder module.

use std::path::Path;

use crate::{CancelSignal, DaemonExit};

/// Input provided to the daemon for its operation.
#[allow(dead_code)] // Temp
pub(crate) struct DaemonInput<'a> {
    /// The current working directory in which the daemon operates.
    pub(crate) cwd: &'a Path,
    /// The configuration used to run the daemon.
    pub(crate) config: &'a rorolala_vault::Config,
    /// The signal used to cancel the daemon.
    pub(crate) signal: CancelSignal,
}

/// ok
#[allow(clippy::unused_async)] // Temp
pub(crate) async fn daemon(input: DaemonInput<'_>) -> DaemonExit {
    let _ = input;
    let _ = input.signal.rx;
    DaemonExit::default()
}
