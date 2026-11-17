//! A write's cancellation, as one flag a caller sets and every step reads.

use std::sync::atomic::{AtomicBool, Ordering};

use rorolala_utils_lazyffi::lazyffi;

/// Whether a write has been asked to stop.
///
/// A write checks it at the start and between the steps of a long loop. What it has already written
/// stays written: cancelling is stopping, not undoing, and a caller that wants the old state asks for
/// it again rather than assuming the write never happened.
#[lazyffi(export = RolaCancellation)]
#[derive(Debug)]
pub struct Cancellation {
    /// Whether the caller has asked to stop.
    cancelled: AtomicBool,
}

impl Cancellation {
    /// A cancellation nobody has set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
        }
    }

    /// Asks the write to stop.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Whether the write has been asked to stop.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

/// Makes a cancellation a C caller can set.
///
/// # FFI
///
/// A `RolaCancellation *` the caller releases with `free_rola_cancellation`, or null when one cannot
/// be made.
#[must_use]
#[lazyffi(export = rola_cancellation_new)]
pub const fn new_cancellation() -> Option<Cancellation> {
    Some(Cancellation::new())
}

/// Asks a write to stop.
///
/// # FFI
///
/// Nothing is returned. The write checks the flag at its next step.
#[lazyffi(export = rola_cancellation_cancel)]
pub fn cancel(cancellation: &Cancellation) {
    cancellation.cancel();
}

/// Whether a write has been asked to stop.
///
/// # FFI
///
/// The flag as a `bool`.
#[must_use]
#[lazyffi(export = rola_cancellation_is_cancelled)]
pub fn is_cancelled(cancellation: &Cancellation) -> bool {
    cancellation.is_cancelled()
}
