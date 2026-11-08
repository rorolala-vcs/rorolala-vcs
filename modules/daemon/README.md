# rorolala-daemon

## Introduction

The machine-wide daemon: it serves the Vaults registered for a user, one namespace each, over a TCP port or over the standard streams of one SSH session.

## Boundary

It owns the namespace registry, the machine's session preferences, the port it binds, the choice of transport, and running an action on each connection. It does not own the Vault: creating one, its configuration, its key scopes and its root and nested view are `modules/vault`. It does not define what an action is either: the address, the `Action` trait, the context and the framing are `modules/protocol`. The content and index a served Vault holds are `modules/storage` and `modules/vcs`.
