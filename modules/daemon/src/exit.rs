use rorolala_utils_lazyffi::lazyffi;

/// Structure information for when the daemon exits
#[lazyffi(export = RolaDaemonExit)]
#[derive(Default)]
pub struct DaemonExit {}
