# rola-daemon

The background daemon for Rorolala, belonging to the machine rather than to a directory.

It serves the Vaults registered for the user, one namespace each, on one port. The table it
reads is `~/.local/share/rola/daemon/registry.toml` — `rola/daemon/registry.toml` under the
local data directory — and is a flat TOML table:

```toml
alpha = "/home/me/vaults/alpha"
beta = "/home/me/vaults/beta"
```

## Commands

```text
rola-daemon                              start the daemon
rola-daemon list                         list the registered namespaces
rola-daemon register <NAMESPACE> [PATH]  serve the Vault at PATH, or the one here
rola-daemon remove <NAMESPACE>           stop serving a namespace
rola-daemon check                        check every namespace reaches a Vault
```

A `register` that names no path registers the Vault the run is inside: the nearest Vault above
the working directory. A run inside none is refused rather than guessed at.

`check` prints `[OK] NAMESPACE (PATH)` or `[FAIL] NAMESPACE (PATH)` and ends non-zero when any
row fails. `list` and `check` answer `--json` and `--format`.

## Addresses

```text
rola://host:port/            the machine
rola://host:port/NAMESPACE   the Vault registered under that name
rola://host:port/NAMESPACE/SUB   a Vault below that one
```

`host:port/NAMESPACE` is the short form and parses the same way.

## The port

The daemon binds `--port`, then `ROLA_DEFAULT_PORT`, then `7717`. The port is no longer part of
a Vault's `vault.toml`: one daemon answers for every namespace, so the port is the daemon's.

## Identity

The listener proves itself with the user's own account, found in the user's key directory
(`~/.local/share/rola/keys`) and the keys `ROLA_HOME` names. It has to choose one identity before
it reads which namespace a caller wants, so it cannot prove itself with a Vault's key; what a
Vault admits is still read from that Vault's own configuration, per connection.
