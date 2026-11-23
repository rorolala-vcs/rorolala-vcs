---
title: "About the Daemon"
description: "What rola-daemon serves, how a namespace names a Vault, and the two ways a request reaches it."
keywords: daemon, rola-daemon, namespace, registry, port, ssh, transport, concurrency, address book
---

# About the Daemon

`rola-daemon` is the process that makes Vaults reachable. It runs for one user on one machine, serves every Vault registered for that user, and names each one with a namespace.

A Vault is a directory. The daemon is what turns that directory into something another machine can reach, and the registry is what says which namespace names which directory.

## What it serves

A Vault is registered under a namespace:

```sh
rola-daemon register studio ~/Vaults/studio
rola-daemon list
rola-daemon check
rola-daemon remove studio
```

| Command | What it does |
| --- | --- |
| `register <NAMESPACE> [PATH]` | Serve the Vault at `PATH`, or the one the run is in, under the namespace. |
| `list` | List the namespaces this machine serves. |
| `remove <NAMESPACE>` | Stop serving a namespace. |
| `check` | Check that every namespace still reaches a Vault. |
| `serve <PATH>` | Serve one Vault over this process's standard input and output, for an SSH caller. |

The registry is a file under the user's data directory, one namespace to one Vault path. The launcher's address book is read from the same registry with `rola-daemon list`.

## Two transports, one protocol

A request reaches the daemon in one of two ways:

- Over a TCP port. A run with no command starts the daemon, listening on the default port unless `--port` names another. `--concurrency` caps how many transfers are carried at once.
- Over the standard streams of one SSH session. `rola-daemon serve <PATH>` is the far side of that: it serves one Vault over the input and output it was given, and exits when the session ends.

The protocol above either transport is the same one. What differs is only how the bytes get there, so an action does not know whether it arrived over a socket or over SSH.

## What it does not own

The daemon does not own a Vault. Creating one, its configuration, its key scopes and its nested Vaults belong to the Vault itself. What the daemon owns is the namespace registry, the choice of transport, the port, and running one action per connection.

It also does not answer for content. A request for a key is passed to the Vault's store, and a request about versions is passed to its index. The daemon is the door rather than the room.

## The window

`rola-daemon-gui` is a small window over the same program. It shows the daemon and its output, the registry, the keys and the administrators of the registered Vaults, and every action behind it is still a `rola-daemon` or `rola` command.

A window started with `--run` presses Start as it opens. That is what the launcher's Open daemon action asks for, so a Vault can be made reachable from the address book without a second step.

## When nothing is reachable

A daemon that is not running is not a missing Vault. A command that only needs local work carries on, and a command that must exchange with a Vault refuses with the reason `--offline` states. The address check in the launcher reports the same difference: an address that is not answered is a daemon that is not there, or a Vault whose namespace is not registered.
