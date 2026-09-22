# rorolala-storage

Storage for Rorolala: objects named by the hash of their content, read and written through a
replaceable backend.

## The boundary

- [`StorageBackend`] is local only. It hashes a file, keeps the content, and hands it back.
  Nothing it does ever reaches another machine.
- [`TransferableBackend`] is the local half of moving content between two ends of the *same kind*:
  which keys are held, and reading or storing content by key. The exchange itself is driven over
  any stream by [`transfer::initiate`] and [`transfer::respond`], which compare [`ProtocolMagic`]
  before a key is named.
- What moves is content, named by [`Key`] — never a file, and never a layout: content kept as
  chunks crosses whole, and the other end writes it the way that end writes anything.

Encoding — how an object is laid out, and whether it is compressed — is the backend's own
business, so what a caller hands over and gets back does not change when the backend does.

Where a store puts what it keeps is its own business too: the paths are not part of this boundary.
What a test has to see of the placing is in [`internals`](internals), which is a store's own view of
itself rather than something a caller asks for.
