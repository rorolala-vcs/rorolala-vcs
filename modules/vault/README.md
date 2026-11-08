# rorolala-vault

## Introduction

The server-side Vault: a directory made one by its configuration, with its own key directory, index and store.

## Boundary

It owns `vault.toml` (name, description, administrators, key discovery and concurrency), creating a Vault and the directories it needs, the key scopes a search covers, and the view of a root Vault and the Vaults nested under it. It does not serve the protocol: the listener, the registered namespaces and the SSH session mode are `modules/daemon`'s. It does not own the Layout it holds, its store or its index either, which are `modules/layout`, `modules/storage` and `modules/vcs`.
