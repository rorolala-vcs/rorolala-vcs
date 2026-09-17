#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::redundant_pub_crate)]

mod begin;
mod exit;

use std::{path::Path, pin::Pin, sync::Arc};

pub use exit::*;
use rorolala_utils_cli_theme::err_line;
use rorolala_utils_lazyffi::lazyffi;
use tokio::sync::mpsc::Receiver;

use crate::begin::DaemonInput;

/// Entry logic for the Rorolala Daemon, driven by a Tokio multi-threaded
/// runtime.
///
/// This function builds a new multi-threaded Tokio runtime with all features
/// enabled ([`tokio::runtime::Builder::enable_all`]) and then blocks the
/// calling thread on [`daemon_begin_async`] using
/// [`tokio::runtime::Runtime::block_on`].
///
/// # Panics
///
/// Panics if the multi-threaded Tokio runtime fails to build.
///
/// # Standard Out
///
/// On error, error logs are written to `stderr`.
///
/// # FFI
///
/// `daemon_begin_async` is not supported for use on the FFI side; only the
/// blocking (block-on) mode is supported.
#[lazyffi(export = rola_daemon_begin)]
#[must_use]
pub fn daemon_begin(cwd: &Path, config: &rorolala_vault::Config) -> DaemonExit {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build the runtime")
        .block_on(daemon_begin_async(cwd, config))
}

/// Asynchronous entry point for the Rorolala Daemon, driven by the Tokio
/// runtime.
///
/// Unlike [`daemon_begin`], which builds a multi-threaded Tokio runtime and
/// blocks the calling thread, this function is an `async` entry point meant to
/// be awaited from within an existing Tokio runtime. This makes it suitable for
/// embedding the daemon's logic into an application that already manages its
/// own async executor.
///
/// # Standard Out
///
/// On error, error logs are written to `stderr`.
///
/// # Note
///
/// This function is intended for asynchronous use and is not exposed over the
/// FFI boundary on its own; use [`daemon_begin`] for blocking/FFI callers.
pub async fn daemon_begin_async(cwd: &Path, config: &rorolala_vault::Config) -> DaemonExit {
    let cancel = init_close_channel();

    let (exit, ()) = tokio::join!(
        // Daemon core logic
        crate::begin::daemon(DaemonInput {
            cwd,
            config,
            signal: cancel.get_rx()
        }),
        // Ctrl + C
        cancel.future,
    );

    exit
}

/// A cancellation context that bundles the signalling channel together with
/// the future responsible for listening to the ctrl-c signal.
///
/// The [`rx`](Self::rx) half forms a Tokio mpsc channel used to notify
/// cancellation, while [`future`](Self::future) is a boxed, sendable future
/// that waits for the ctrl-c signal and dispatches the notification.
struct Cancellation {
    /// Receiver half of the cancellation channel, used to observe cancellation
    /// requests.
    rx: Arc<Receiver<bool>>,
    /// Boxed future that listens for the ctrl-c signal and sends a
    /// cancellation notification through the channel.
    future: Pin<Box<dyn Future<Output = ()> + 'static + Send + Sync>>,
}

/// A cancellation signal that only exposes the receiving half of the
/// cancellation channel.
///
/// This type is used by tasks that need to observe cancellation requests
/// without being able to signal cancellation themselves.
struct CancelSignal {
    /// Receiver half of the cancellation channel, used to observe cancellation
    /// requests.
    rx: Arc<Receiver<bool>>,
}

impl Cancellation {
    /// Returns the receiving half of the cancellation channel, wrapping it in a
    /// [`CancelSignal`].
    ///
    /// Clones the shared [`Arc`]-wrapped receiver and wraps it in a
    /// [`CancelSignal`], allowing additional observers of the cancellation
    /// channel without taking ownership away from `self`.
    fn get_rx(&self) -> CancelSignal {
        CancelSignal {
            rx: Arc::clone(&self.rx),
        }
    }
}

/// Initializes the cancellation channel and the ctrl-c listener future.
///
/// Creates a Tokio mpsc channel used to signal cancellation, clones the sender
/// for use inside the listener future, and boxes a future that waits for the
/// `ctrl-c` signal and then sends a cancellation notification.
///
/// # Standard Out
///
/// Writes an error log to `stderr` if listening for the ctrl-c signal fails.
fn init_close_channel() -> Cancellation {
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    let future = Box::pin(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            eprintln!("{}", err_line!("Failed to listen for ctrl-c signal."));
            return;
        }
        let _ = tx.send(true).await;
    });

    Cancellation {
        rx: Arc::new(rx),
        future,
    }
}
