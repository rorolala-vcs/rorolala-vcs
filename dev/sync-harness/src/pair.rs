//! The channel a sync crosses, and how many bytes cross it.
//!
//! A sync crosses an encrypted session and nothing else — a plain stream has no way into an
//! [`ActionContext`](rorolala_protocol::ActionContext) — so what the harness measures is a handshaken
//! [`SecureStream`] over a pair of buffers in this process. That leaves
//! out what a network would add and leaves in what the session costs: the records, the sealing and
//! the wake-ups each value becomes.
//!
//! Each end's socket counts what it writes, which is the wire: a byte one side wrote is a byte the
//! other read, and counting both would count every byte twice.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

use rorolala_auth::{KeyAlgorithm, SecureStream, SigningKey};
use rorolala_protocol::{Channel, Socket};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf, duplex};

/// How many bytes a side has written into the buffers the two ends share.
pub type Counter = Arc<AtomicU64>;

/// A channel at each end of a sync, and how many bytes each end moved.
pub struct Pair {
    /// The Workspace's end.
    pub workspace: Channel,
    /// The Vault's end.
    pub vault: Channel,
    /// What the Workspace wrote.
    pub up: Counter,
    /// What the Vault wrote.
    pub down: Counter,
}

/// A handshaken channel at each end, over a pair of buffers of `capacity` bytes.
///
/// # Panics
///
/// Panics if the handshake does not complete. Two ends of one process always can handshake, so a
/// failure here is a build that cannot do what it says rather than a case a measurement reports.
pub async fn handshaken(capacity: usize) -> Pair {
    let client = signing(1);
    let server = signing(2);
    let server_public = server.public_key();

    let (client_io, server_io) = duplex(capacity);
    let up: Counter = Arc::new(AtomicU64::new(0));
    let down: Counter = Arc::new(AtomicU64::new(0));

    let client_socket = Box::new(Counting {
        inner: client_io,
        written: Arc::clone(&up),
    }) as Box<dyn Socket>;
    let server_socket = Box::new(Counting {
        inner: server_io,
        written: Arc::clone(&down),
    }) as Box<dyn Socket>;

    let (workspace, vault) = tokio::join!(
        SecureStream::connect(client_socket, &client, &server_public),
        SecureStream::accept(server_socket, &server),
    );

    Pair {
        workspace: workspace.expect("the Workspace handshakes"),
        vault: vault.expect("the Vault handshakes"),
        up,
        down,
    }
}

/// A signing key from a fixed seed, so a measurement names the same two identities every run.
///
/// # Panics
///
/// Panics if the algorithm refuses a seed of its own width, which would be a build that cannot make
/// an identity at all.
fn signing(seed: u8) -> SigningKey {
    SigningKey::from_bytes(KeyAlgorithm::Ed25519, [seed; 32]).expect("a seed names a key")
}

/// A socket that counts what is written through it.
struct Counting {
    /// The buffer the bytes go into.
    inner: DuplexStream,
    /// How many bytes have been written.
    written: Counter,
}

impl AsyncRead for Counting {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Counting {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let me = self.get_mut();
        let answered = Pin::new(&mut me.inner).poll_write(cx, data);

        if let Poll::Ready(Ok(wrote)) = answered {
            me.written.fetch_add(wrote as u64, Ordering::Relaxed);
        }

        answered
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
