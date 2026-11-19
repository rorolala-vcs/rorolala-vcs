//! The flag a Ctrl+C sets while a write is running.
//!
//! A write may already have put content down when the signal arrives, so what stops is the run and
//! not the process: the flag is set, the step in hand finishes, and the write answers that it was
//! cancelled. Nothing is rolled back, which is what lets the caller read what happened instead of
//! guessing.
//!
//! The flag is one per process. `ctrlc::set_handler` may be called once, so a second command
//! installing its own handler would take the first's place; one flag also means a Ctrl+C that
//! arrives while a plan is being read stops the write that follows it.

use std::sync::OnceLock;

use librorolala::Cancellation;

/// The flag a Ctrl+C sets, made once for the process.
pub fn on_ctrl_c() -> &'static Cancellation {
    static TOKEN: OnceLock<Cancellation> = OnceLock::new();
    static INSTALLED: OnceLock<()> = OnceLock::new();

    let token = TOKEN.get_or_init(Cancellation::new);

    INSTALLED.get_or_init(|| {
        // A handler that cannot be installed leaves the run uncancellable rather than failing the
        // command: the work still reaches its own end, and a Ctrl+C the process cannot catch is
        // the terminal's to end.
        let _ = ctrlc::set_handler(|| {
            if let Some(token) = TOKEN.get() {
                token.cancel();
            }
        });
    });

    token
}

#[cfg(test)]
mod tests {
    use super::on_ctrl_c;

    /// One flag is shared, and a run starts without having been asked to stop.
    #[test]
    fn one_flag_is_shared() {
        assert!(!on_ctrl_c().is_cancelled());
        assert!(std::ptr::eq(on_ctrl_c(), on_ctrl_c()));
    }
}
