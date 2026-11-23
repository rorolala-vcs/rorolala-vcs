---
title: "About a Vault"
description: "What a Vault is on disk, what it serves, and how it differs from the Workspace you work in."
keywords: vault, server, daemon, workspace, store, layout, keys, admins, namespace, ownership
---

# About a Vault

A Vault is the server side of Rorolala. The work is recorded in a Workspace, and a Vault is where that work is sent when it is shared.

A directory is a Vault because it holds `vault.toml`. A run locates one by searching upwards from where it stands, so a command made inside a Vault, or inside a directory under one, works on the nearest Vault above it.

## What a Vault holds

The configuration is what makes the directory a Vault. The directories beside it are what the Vault keeps.

| Path           | What it is                                                                                                                                                                 |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `vault.toml`   | The configuration. It holds the name and description, the administrators, how the daemon finds keys, the stores the Vault binds, and the grace period its collection uses. |
| `keys/`        | The Vault's own key pair. A Vault is not served without one, so the directory is made with the Vault.                                                                      |
| `index/`       | The version index: the variants, versions, creators and messages the Vault holds.                                                                                          |
| `storage/`     | The content store, see `vcs-knowledge/storage`.                                                                                                                            |
| `layout/`      | The one Layout the Vault keeps. It is the record of what the Vault holds, and of who owns each entry.                                                                      |
| `vaults/`      | Vaults nested under this one. A root Vault can show them as a single view.                                                                                                 |
| `info/ignore/` | The ignore rules a Workspace receives when it fetches this Vault.                                                                                                          |
| `lock`         | The file a run takes while it changes the Vault.                                                                                                                           |

A Vault holds one Layout rather than many, so `layout/` is the Layout itself and not a directory of names. What a Layout is, and how it is kept, is `concepts/layout`.

## The configuration

`vault.toml` is a file of sections, each one about a part of the Vault:

| Section     | What it decides                                                                                                                             |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `[vault]`   | The name and description. A Vault made by `rola create` is named after its directory and described as a new Vault until someone changes it. |
| `[auth]`    | The administrators. A name here is the name of a public key file, so an administrator is an account the Vault recognises.                   |
| `[daemon]`  | How the daemon finds keys, and how many sessions it serves at once.                                                                         |
| `[storage]` | The stores the Vault writes to. The Vault's own `storage/` is one of them.                                                                  |
| `[gc]`      | The grace period its collection gives content that is no longer named.                                                                      |

The file is written whole and published in one step, so a run that is interrupted leaves the previous configuration rather than half of a new one.

## Making a Vault

```sh
rola create --vault studio
```

Or, in a directory that is not a Vault yet:

```sh
rola init --vault
```

Making a Vault makes the directory, `keys/`, `index/`, `storage/`, `layout/` and `info/ignore/`, writes `vault.toml`, and writes the default ignore rules. The rules matter because a Workspace that fetches this Vault begins by leaving the platforms' own files out rather than reading them as work.

A Vault is named after the directory it is made in. The name is what is written down, not what is read from the path later, so a Vault that is moved keeps the name it was made with.

## A Vault is served, not opened

A Workspace does not work on a Vault directly. The Vault is reached through `rola-daemon`, which serves the Vaults registered for a user under a namespace each, over a TCP port or over one SSH session.

A Vault address names the host and the namespace, and `rola vault handshake <address>` speaks a handshake to it. That is what the launcher's address check does, and what makes an address in the address book read as reachable or without an answer. See `concepts/daemon` and `concepts/address`.

## A Vault and a Workspace

|                   | Workspace                                                   | Vault                          |
| ----------------- | ----------------------------------------------------------- | ------------------------------ |
| What it is        | The directory a person works in                             | The place work is sent to      |
| What makes it one | A `.rola/` directory                                        | A `vault.toml` file            |
| What it holds     | The working tree, a local store, a local index and a Layout | A store, an index and a Layout |
| How it is reached | Directly, from the command line or the Desktop program      | Through `rola-daemon`          |
| Who owns an entry | It reads a fetched copy of the Vault's Layout               | It holds the record            |

Ownership is the clearest difference. The Vault's Layout says who holds each entry, and a Workspace holds a read-only copy of that Layout, fetched from the Vault. A path is never owned locally in a way that the Vault does not know about. See `concepts/ownership`.

## When a Vault cannot be reached

A run that only needs what is on this machine carries on. A run whose work is the exchange itself, a handshake, a claim of ownership or a sync, refuses with the reason `--offline` states rather than waiting for a connection.

Nothing is fetched early. A Workspace asks the Vault for the Layout or the content it needs at the moment it needs it, so an unreachable Vault is a missing answer at that moment rather than a repository that failed to open.
