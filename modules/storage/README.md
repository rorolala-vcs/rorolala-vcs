# rorolala-storage

## Introduction

Content-addressed storage: objects named by the hash of their content, kept through a replaceable local backend and moved between two ends of the same kind over a stream.

## Boundary

It owns the key, the backend traits, packs, manifests, the store lock, the transfer exchange, and the chunking crate it re-exports. It does not own the VCS index, which `modules/vcs` keeps as a store of its own, or the Layout, which is `modules/layout`'s. Its backends are local: moving objects between machines happens only through `TransferableBackend` and the explicit transfer, and where a store places what it keeps is not part of this boundary.
