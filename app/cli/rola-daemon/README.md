# rola-daemon

Serve the Vaults registered for this user, as the machine's daemon.

```bash
rola-daemon                              # start the daemon
rola-daemon list                         # list the registered namespaces
rola-daemon register <NAMESPACE> <PATH>  # serve the Vault at PATH under NAMESPACE
rola-daemon remove <NAMESPACE>           # stop serving a namespace
rola-daemon check                        # check every namespace reaches a Vault
```

The daemon is independent of the working directory. What it serves is the table at
`~/.local/share/rola/daemon/registry.toml` — one namespace per Vault — and the port is named
where it is started: `--port`, then `ROLA_DEFAULT_PORT`, then `7717`.

`check` prints `[OK] NAMESPACE (PATH)` or `[FAIL] NAMESPACE (PATH)` and ends non-zero when a row
fails. `list` and `check` answer `--json` and `--format`.
