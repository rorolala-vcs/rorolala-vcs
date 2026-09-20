#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::redundant_pub_crate)]

use std::{path::Path, pin::Pin};

use rorolala_auth::{KeyLocateRule, SigningKey, locate_accounts};
use rorolala_utils_cli_theme::err_line;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_vault::KEYS_DIR;
use tokio::sync::watch;

use crate::begin::DaemonInput;

mod action;
mod begin;
mod exit;
mod wire;

pub use action::*;
pub use exit::*;

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
/// The Vault's own identity is resolved here, once, before anything is served: a
/// daemon that cannot prove itself cannot serve at all.
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
    let Some(identity) = vault_identity(cwd) else {
        return DaemonExit::default();
    };

    let cancel = init_close_channel();

    let (exit, ()) = tokio::join!(
        // Daemon core logic
        crate::begin::daemon(DaemonInput {
            cwd,
            config,
            identity,
            signal: cancel.get_rx()
        }),
        // Ctrl + C
        cancel.future,
    );

    exit
}

/// The identity the Vault proves, read from the first account under its keys
/// directory.
///
/// Which key a Vault is is host setup, not something the Vault *side* of a
/// protocol touches: there, a peer is a [`Member`](rorolala_auth::Member) and
/// nothing else. Resolving it here keeps that side free of the Workspace's
/// notion of an account, and hands on the bare key it needs.
///
/// # Standard Error
///
/// Writes an error log when the Vault holds no account, or its key cannot be
/// read.
fn vault_identity(cwd: &Path) -> Option<SigningKey> {
    let keys = vec![cwd.join(KEYS_DIR)];
    let accounts = locate_accounts(&keys, &KeyLocateRule::new());

    let Some(account) = accounts.iter().next() else {
        eprintln!(
            "{}",
            err_line!(
                "The Vault holds no account under {} to prove itself with.",
                (keys[0].display())
            )
        );
        return None;
    };

    match account.get_key() {
        Ok(identity) => Some(identity),
        Err(error) => {
            eprintln!(
                "{}",
                err_line!("The Vault's own key could not be read: {error}")
            );
            None
        }
    }
}

/// A cancellation context that bundles the signalling channel together with
/// the future responsible for listening to the ctrl-c signal.
///
/// The channel is a *watch*, not a queue: cancellation is one event that every
/// observer should see, so a receiver clones rather than takes, and the last
/// value sent is the one that stands.
struct Cancellation {
    /// Receiver half of the cancellation channel, used to observe cancellation
    /// requests.
    rx: watch::Receiver<bool>,
    /// Boxed future that listens for the ctrl-c signal and sends a
    /// cancellation notification through the channel.
    future: Pin<Box<dyn Future<Output = ()> + 'static + Send + Sync>>,
}

/// A cancellation signal that only exposes the receiving half of the
/// cancellation channel.
///
/// This type is used by tasks that need to observe cancellation requests
/// without being able to signal cancellation themselves. It is cheap to clone,
/// so every task the daemon spawns can watch the same signal.
#[derive(Clone)]
struct CancelSignal {
    /// Receiver half of the cancellation channel, used to observe cancellation
    /// requests.
    rx: watch::Receiver<bool>,
}

impl Cancellation {
    /// Returns the receiving half of the cancellation channel, wrapping it in a
    /// [`CancelSignal`].
    ///
    /// Clones the receiver and wraps it in a [`CancelSignal`], allowing additional
    /// observers of the cancellation channel without taking it away from `self`.
    fn get_rx(&self) -> CancelSignal {
        CancelSignal {
            rx: self.rx.clone(),
        }
    }
}

/// Initializes the cancellation channel and the ctrl-c listener future.
///
/// Creates a Tokio *watch* channel used to signal cancellation, keeps the receiving
/// half, and boxes a future that waits for the `ctrl-c` signal and then sends `true`
/// through the channel.
///
/// # Standard Error
///
/// Writes an error log to `stderr` if listening for the ctrl-c signal fails.
fn init_close_channel() -> Cancellation {
    let (tx, rx) = watch::channel(false);

    let future = Box::pin(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            eprintln!("{}", err_line!("Failed to listen for ctrl-c signal."));
            return;
        }
        // A send fails only when nothing is listening any more, in which case
        // there is nobody left to cancel.
        let _ = tx.send(true);
    });

    Cancellation { rx, future }
}
