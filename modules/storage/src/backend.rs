use std::future::Future;
use std::path::Path;

use crate::{Key, Presence};

/// Local, content-addressed storage.
///
/// A backend reads and writes objects kept under [`Key`]s and nothing else. It never speaks to
/// another machine — moving objects between two ends is [`TransferableBackend`]'s job — and it
/// never decides what an object *means*: a caller hands over a path to store, or a key and a
/// path to put the content at.
///
/// What a backend does decide is everything about how content is *kept*: where the bytes go,
/// whether they are compressed, and how they are cut. None of it is visible above, since a key is
/// the hash of the original content rather than of the stored bytes — and none of it is part of
/// the key, so a file re-written another way keeps the name it had.
///
/// The trait is deliberately not object safe — `impl Future` in a trait cannot be reached
/// through `dyn` — because which backend a caller uses is decided where the backend is instead
/// of behind a pointer.
pub trait StorageBackend: Send + Sync {
    /// What a failed operation reports.
    type Error: std::error::Error + Send + Sync + 'static;

    /// How this backend says a file should be written: its encoding and how it is cut.
    ///
    /// It is a value rather than a set of methods so that choosing is something that happens
    /// once, in front of the file, and writing is the plain business of laying down what the
    /// choice says.
    type AlgorithmChoice: Send;

    /// Chooses how `file` is to be written.
    ///
    /// This is the one place a backend looks at content and decides: whether what is in front of
    /// it is worth compressing, whether it is big enough to cut. Nothing about the answer is
    /// written down except the result of it, so the same file may be written differently on
    /// another day without any key changing.
    fn choose_algorithm(&self, file: &Path) -> Self::AlgorithmChoice;

    /// Stores the content of `file`, written with `choice`, and answers with its key.
    ///
    /// The key is the hash of `file`'s content, so storing the same content twice writes the
    /// same object and answers with the same key either time, whatever `choice` says.
    ///
    /// A caller that has no opinion about the choice wants [`store_file`], which makes it.
    ///
    /// # Errors
    ///
    /// [`Io`](crate::Error::Io) if `file` cannot be read — a file that is not there is an I/O
    /// failure like any other, since it names no key to be [`NotFound`](crate::Error::NotFound)
    /// under — and whatever the backend itself fails with.
    fn write_file(
        &self,
        file: &Path,
        choice: Self::AlgorithmChoice,
    ) -> impl Future<Output = Result<Key, Self::Error>> + Send;

    /// Writes the content stored under `key` to `path`.
    ///
    /// Whatever the backend did to the content on the way in is undone on the way out, so what
    /// lands at `path` is what was stored. A backend that can check it does: content that does
    /// not hash back to `key` is [`Corrupt`](crate::Error::Corrupt) rather than handed over.
    ///
    /// # Errors
    ///
    /// [`NotFound`](crate::Error::NotFound) if nothing is stored under `key`, and whatever the
    /// backend itself fails with.
    fn extract_file(
        &self,
        key: &Key,
        path: &Path,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Which of `keys` this backend holds.
    ///
    /// "Held" is whether the entries are there: content is held when it is stored whole, and a
    /// content kept as a manifest is held when every chunk it names is stored. It is not a promise
    /// that they *read* — content that is there but does not hold together is found on the read that
    /// asks for it, not here, since finding it would mean reading everything this is asked about.
    ///
    /// The answer is a [`Presence`] **exactly as long as `keys`**, one byte per key in the order it
    /// was asked about.
    ///
    /// # Errors
    ///
    /// Whatever the backend itself fails with.
    fn contains_keys(
        &self,
        keys: &[Key],
    ) -> impl Future<Output = Result<Presence, Self::Error>> + Send;

    /// Every key the backend knows of, whether or not it holds it.
    ///
    /// This is what is *there*: the objects it holds, the manifests it holds, and the keys in its
    /// packs. A manifest whose chunks have gone is named here and held nowhere — see
    /// [`list_exist_keys`](Self::list_exist_keys) for the ones that are held.
    ///
    /// # Errors
    ///
    /// Whatever the backend itself fails with.
    fn list_all_keys(&self) -> impl Future<Output = Result<Vec<Key>, Self::Error>> + Send;

    /// Every key the backend holds.
    ///
    /// This is [`list_all_keys`](Self::list_all_keys) with the keys that are not held taken out —
    /// the same answer as asking [`contains_keys`](Self::contains_keys) of each one, and the same
    /// answer as a transfer would get from this end.
    ///
    /// # Errors
    ///
    /// Whatever the backend itself fails with.
    fn list_exist_keys(&self) -> impl Future<Output = Result<Vec<Key>, Self::Error>> + Send;

    /// Drops the object stored under `key`.
    ///
    /// # Errors
    ///
    /// [`NotFound`](crate::Error::NotFound) if nothing is stored under `key`, and whatever the
    /// backend itself fails with.
    fn remove(&self, key: &Key) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// The width, in bytes, of the password two ends exchange.
pub const PROTOCOL_MAGIC_LEN: usize = 8;

/// The password two ends compare before anything moves between them.
pub type ProtocolMagic = [u8; PROTOCOL_MAGIC_LEN];

/// Moving the content of a store to another end of the same kind.
///
/// The trait is the **local half** of a transfer: what one end has to be able to answer, and to
/// take, when the other end asks. What carries the answers — a socket, a pipe, a pair of buffers in
/// one process — is not here, because storage is at the bottom of the crate stack and knows nothing
/// of sockets; the exchange itself is driven over whatever stream the caller hands it, by
/// [`transfer::initiate`](crate::transfer::initiate) and
/// [`transfer::respond`](crate::transfer::respond).
///
/// Both ends must be the *same* backend — a vault store syncs with another vault store and with
/// nothing else — and before a single object is read or written they exchange
/// [`PROTOCOL_MAGIC`](Self::PROTOCOL_MAGIC), a fixed-width password that says the two are the same
/// kind. A password that does not match is refused rather than guessed at, which is what keeps a
/// store from being read by something that is not a store of its kind. It names no version: a
/// protocol that changes while keeping its password changes its meaning for both ends at once.
///
/// What moves is content named by [`Key`], never a file and never a layout: a content kept as
/// chunks comes back as the whole of it and is written by the other end the way that end writes
/// anything, so a transfer never has to know how its peer keeps what it is sent.
pub trait TransferableBackend: StorageBackend {
    /// The password this kind of backend recognises its peer by.
    const PROTOCOL_MAGIC: ProtocolMagic;

    /// The content stored under `key`, or `None` when there is none.
    ///
    /// This is the content, not the entry: a content kept as chunks is answered with the whole of
    /// it. Nothing about how it is kept goes over the wire.
    ///
    /// `None` is content that is not there, which is what lets a sender leave out a key it can no
    /// longer produce. Content that is there but does not hold together is an error instead, and an
    /// error ends the exchange rather than being quietly read as "not there": a store that hands
    /// back something other than what it holds is a store to be seen to, not one to pass over.
    ///
    /// # Errors
    ///
    /// Whatever the backend itself fails with, including content that is there but does not read.
    fn content(
        &self,
        key: &Key,
    ) -> impl Future<Output = Result<Option<Vec<u8>>, Self::Error>> + Send;

    /// Stores `content` under `key`, which is the hash it has to come to.
    ///
    /// How it is kept — which codec, whether to cut it — is this end's own business, the same
    /// business as any other write, so two stores that keep things differently still end up with
    /// the same content under the same key.
    ///
    /// # Errors
    ///
    /// Whatever the backend itself fails with, including content that does not hash to `key`.
    fn accept(
        &self,
        key: &Key,
        content: Vec<u8>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Stores `file`, deciding how it is written the way `backend` decides.
///
/// This is a write as a caller meets it: the *how* is the backend's to choose from the file in
/// front of it — see [`StorageBackend::choose_algorithm`] — and once it has, the write itself is
/// the plain business of putting the content where its key says. A caller that has already made
/// the choice wants [`StorageBackend::write_file`] instead.
///
/// # Errors
///
/// Whatever [`StorageBackend::write_file`] fails with.
pub async fn store_file<Backend>(
    backend: &Backend,
    file: impl AsRef<Path> + Send,
) -> Result<Key, Backend::Error>
where
    Backend: StorageBackend + Sync,
{
    let file = file.as_ref();
    let choice = backend.choose_algorithm(file);

    backend.write_file(file, choice).await
}
