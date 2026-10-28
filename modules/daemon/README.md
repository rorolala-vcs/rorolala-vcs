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
rola-daemon serve <PATH>                 serve the Vault at PATH over stdin and stdout
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

## Reached over SSH

A Vault can also be served with no port at all. `rola-daemon serve <PATH>` serves one
connection over the process's own standard input and output, and ends when that connection does:
it is what the far side of an `ssh` runs, and the whole of the transport is the `ssh` program
the caller already has.

`rola` writes the address `user@host:path` — an scp destination with the SSH port between the
host and the path when one is named — and runs

```text
ssh -T -o BatchMode=yes [-p PORT] -l USER -- HOST "rola-daemon serve 'PATH'"
```

`PATH` here is the root of a Vault on the far side, taken exactly as it is written: the table of
namespaces is not read in this mode, and neither is a sub-vault named. What the far side has to
have is `rola-daemon` on its `PATH` (or a shell alias of that name) and permission to run it as
that account.

Two things about the mode are contracts. The standard **output** carries the protocol and nothing
else, so a login script that prints breaks the session — which is what `-T` and the
"standard output must be clean" note in a failure are for. The standard **error** is free: what the
far side says about a failure, and the audit line of a registration, are written there and
forwarded by `ssh` to the caller's terminal.

`BatchMode=yes` is fixed rather than chosen: the caller's standard input is the protocol, so a
password or a passphrase can never be typed into it. A session needs a host key already known and
an agent or a key without a passphrase; a window that has no terminal at all then fails at once
instead of waiting for something that can never be typed.

## This machine's preferences

What the daemon may be asked for by a session is the machine's, not any one Vault's, and is kept
beside the namespace table as `rola/daemon/preference.toml` under the local data directory:

```toml
[security]
ssh-connection = true
auto-register-by-ssh = false
```

A file that is not there is a machine that has said nothing, which is those two values.

- `ssh-connection` — whether a session is served at all. Off refuses it before any Vault is read.
- `auto-register-by-ssh` — whether a connection may write the key it proved into the Vault's own
  keys when the name it uses has none there. Off by default: a Vault whose members are written down
  by hand is the usual one, and a switch that writes into one unasked is one nobody would leave a
  port behind. Note what turning it on means: **deleting a key is no longer a revocation**, since
  the next connection writes it back. Revoking is turning this off, or closing the SSH access —
  with it off, deleting the key is a revocation again.

A registration is only ever made where the Vault's `key_discovery` names its own keys (`vault`),
never for a name in `[auth] admins` — an administrator may move any file in the Vault, so a
connection may not name itself one — and never over a key that is already there under that name.
The names are first come, first served: what protects them is that a name is claimed with a key
the far side proves it holds, which is the handshake's doing and not the registration's.

`ssh-connection` and `auto-register-by-ssh` are policy, not a security boundary. Nothing in a
process can prove it was started by `sshd` — the environment is a program's to write — so what
actually decides what a session may do is who may run `rola-daemon` as this user at all. Pinning
the far side's `authorized_keys` entry to `rola-daemon serve` with `command=` is what makes the
switch mean what it says.

## The port

The daemon binds `--port`, then `ROLA_DEFAULT_PORT`, then `7717`. The port is no longer part of
a Vault's `vault.toml`: one daemon answers for every namespace, so the port is the daemon's.

## Identity

The listener proves itself with the user's own account, found in the user's key directory
(`~/.local/share/rola/keys`) and the keys `ROLA_HOME` names. It has to choose one identity before
it reads which namespace a caller wants, so it cannot prove itself with a Vault's key; what a
Vault admits is still read from that Vault's own configuration, per connection.
