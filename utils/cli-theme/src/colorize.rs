use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use colored::control::SHOULD_COLORIZE;

pub use colored::{ColoredString, Colorize};

/// Whether Rorolala's output is colorized
///
/// Starts as the environment's answer — see [`is_enabled`] — and is replaced the moment
/// a program states its own, which is why it is lazily built rather than a bare
/// [`AtomicBool`]: the initial value is not knowable until the environment is read.
static ENABLED: LazyLock<AtomicBool> =
    LazyLock::new(|| AtomicBool::new(SHOULD_COLORIZE.should_colorize()));

/// States whether output should be colorized, for the whole program
///
/// This takes the decision away from the environment: from here on the answer is what
/// was passed, whatever `NO_COLOR`, `CLICOLOR_FORCE` and the terminal say. It is handed
/// to `colored` as well, so anything in the workspace that colors through `colored`
/// directly follows it too.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    SHOULD_COLORIZE.set_override(enabled);
}

/// Whether output is being colorized
///
/// Until [`set_enabled`] is called, this is the environment's answer: `CLICOLOR_FORCE`
/// first, then `NO_COLOR`, then `CLICOLOR` together with whether standard output is a
/// terminal — the rules `colored` itself follows.
#[must_use]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Gives the decision back to the environment
///
/// The counterpart of [`set_enabled`]: what was stated is forgotten, and the answer
/// becomes the environment's again.
pub fn unset_enabled() {
    SHOULD_COLORIZE.unset_override();
    ENABLED.store(SHOULD_COLORIZE.should_colorize(), Ordering::Relaxed);
}
