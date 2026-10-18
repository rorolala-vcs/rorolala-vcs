//! The port this daemon listens on, as the run and the machine name it.
//!
//! A port is not a Vault's any more: one daemon serves every namespace registered for the user,
//! so the port belongs to the daemon and is named where the daemon is started. What a run writes
//! is taken before the environment, and what the environment says before the default, because
//! that is the order in which the three can override one another: the flag is this run's, the
//! variable is this user's, and the default is what is left.

use std::env;

use mingling::{
    ProgramCollect,
    macros::arg,
    picker::{PickerArg, PickerHelper},
    setup::ProgramSetup,
};
use rorolala_utils_constants::{PORT_ENV_VAR, VAULT_DEFAULT_PORT};

/// Global argument naming the port the daemon listens on.
///
/// Usage: `--port 7727`
pub const GLOBAL_ARG_PORT: PickerArg<'static, String> = arg![port: String];

/// The port the daemon listens on, once the run and the machine have been asked.
#[derive(Debug, Default, Clone)]
pub struct ResPort {
    /// What the run wrote after `--port`, if it wrote anything.
    requested: Option<String>,
}

impl ResPort {
    /// The port to listen on: `--port`, then [`PORT_ENV_VAR`], then [`VAULT_DEFAULT_PORT`].
    ///
    /// # Errors
    ///
    /// Returns [`PortError`] when what was named is not a port a daemon can listen on. A run
    /// that names one is refused rather than quietly given another, since the port is how a
    /// caller reaches the daemon it just started.
    pub fn get(&self) -> Result<u16, PortError> {
        let environment = env::var(PORT_ENV_VAR).ok();

        resolve(self.requested.as_deref(), environment.as_deref())
    }
}

/// Why a port could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortError {
    /// The text is not a number in the range a port is written in.
    NotANumber,
    /// The number is zero, which names no port.
    Zero,
}

/// The port `requested` or `environment` names, in that order, or the default.
fn resolve(requested: Option<&str>, environment: Option<&str>) -> Result<u16, PortError> {
    requested
        .or(environment)
        .map_or(Ok(VAULT_DEFAULT_PORT), parse)
}

/// Reads one written port.
fn parse(port: &str) -> Result<u16, PortError> {
    let port = port.parse::<u16>().map_err(|_| PortError::NotANumber)?;

    if port == 0 {
        Err(PortError::Zero)
    } else {
        Ok(port)
    }
}

/// A [`ProgramSetup`] that takes `--port` out of the arguments before any command sees it.
pub struct PortSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for PortSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let requested = program.pick_argument(&GLOBAL_ARG_PORT);

        program.with_resource(ResPort { requested });
    }
}

#[cfg(test)]
mod tests {
    use rorolala_utils_constants::VAULT_DEFAULT_PORT;

    use super::{PortError, resolve};

    #[test]
    fn the_run_comes_before_the_machine_and_the_default_comes_last() {
        assert_eq!(resolve(None, None), Ok(VAULT_DEFAULT_PORT));
        assert_eq!(resolve(None, Some("7727")), Ok(7727));
        assert_eq!(resolve(Some("7728"), Some("7727")), Ok(7728));
        assert_eq!(resolve(Some("7728"), None), Ok(7728));
    }

    #[test]
    fn something_that_is_not_a_port_is_refused() {
        assert_eq!(resolve(Some("port"), None), Err(PortError::NotANumber));
        assert_eq!(resolve(Some("70000"), None), Err(PortError::NotANumber));
        assert_eq!(resolve(Some("0"), None), Err(PortError::Zero));
        assert_eq!(resolve(None, Some("0")), Err(PortError::Zero));
    }
}
