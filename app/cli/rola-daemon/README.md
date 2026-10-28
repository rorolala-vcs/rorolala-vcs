# rola-daemon

Serve the Vaults registered for this user, as the machine's daemon.

```bash
rola-daemon                              start the daemon
rola-daemon list                         list the registered namespaces
rola-daemon register <NAMESPACE> [PATH]  serve the Vault at PATH, or the one here
rola-daemon remove <NAMESPACE>           stop serving a namespace
rola-daemon check                        check every namespace reaches a Vault
rola-daemon serve <PATH>                 serve the Vault at PATH over stdin and stdout
```

Naming no `PATH` registers the Vault the current directory is inside — the nearest one above
it — which is the shortest way to serve a Vault from within it. A run that is inside no Vault
and names no path is refused rather than guessed at.

The daemon is independent of the working directory. What it serves is the table at
`~/.local/share/rola/daemon/registry.toml` — one namespace per Vault — and the port is named
where it is started: `--port`, then `ROLA_DEFAULT_PORT`, then `7717`.

`check` prints `[OK] NAMESPACE (PATH)` or `[FAIL] NAMESPACE (PATH)` and ends non-zero when a row
fails. `list` and `check` answer `--json` and `--format`.

`serve <PATH>` is the other way to be reached: it serves one connection over this process's
standard input and output and ends with it, which is what the far side of an `ssh` runs. No port
is bound and the namespace table is not read — `PATH` is the root of the Vault and is taken as
written. Its standard output is the protocol rather than text, so it writes nothing there; what it
has to say goes to the standard error, which `ssh` forwards to whoever started the session. Its
exit code is the daemon's own rather than this program's, since that number is all the caller has
to tell one failure from another.
