# rorolala-protocol

## Introduction

The wire protocol between a Workspace and a Vault: the address a session opens to, the `Action` both sides run, the context it runs in, the tagged values that cross, and the framing over an encrypted channel.

## Boundary

It owns addresses in their three spellings, the `Action` trait, the context that says which side is running, the only-workspace, only-vault and both wrappers, the sync that carries a value to the other side, encoding, framing and `ActionError`. It does not own the handshake or the encryption, which `modules/auth` provides. It does not reach anything either: dialling a daemon or starting an SSH session is `modules/daemon`. It holds no action of its own, since an action belongs to the crate that names its id.
