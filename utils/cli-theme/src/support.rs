//! Scaffolding for the tests.
//!
//! The colouring switch is one for the whole process, so a test that draws through it
//! has to say which way it wants it rather than read whichever way it happens to be.
//! The environment decides until something calls [`set_enabled`](crate::set_enabled) —
//! `CLICOLOR_FORCE` exported in the shell a test run was started from is enough — and
//! two tests sharing the switch would otherwise read each other's answer.

use std::sync::Mutex;

use crate::{set_enabled, unset_enabled};

/// Held for as long as a test has the switch set, so that no two of them are looking
/// at it at once.
static SWITCH: Mutex<()> = Mutex::new(());

/// Draws what `body` asks for with colouring on.
pub fn with_color<T>(body: impl FnOnce() -> T) -> T {
    switched(true, body)
}

/// Draws what `body` asks for with colouring off.
pub fn without_color<T>(body: impl FnOnce() -> T) -> T {
    switched(false, body)
}

/// Runs `body` with the switch set `on`, and puts it back afterwards.
fn switched<T>(on: bool, body: impl FnOnce() -> T) -> T {
    let _held = SWITCH
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    set_enabled(on);
    let value = body();
    unset_enabled();

    value
}
