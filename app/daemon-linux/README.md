# daemon-linux

## Introduction

`daemon-linux` is a small GTK 4 window over the machine's `rola-daemon`, with four tabs: Daemon, Registry, Keys and Admins. It starts and stops the daemon, edits the namespace table, lists and imports keys, and edits the administrators of the registered Vaults.

## Boundary

It owns the drawing, the controls and the log, and finds `rola-daemon` beside itself rather than through a configurable path. It does not own a rule: every action is a `rola-daemon` or `rola` command, run with `--no-color --no-progress`, and a refusal is shown in the program's own words. It generates no keys, which is `rola key generate`, and imports them itself only because the CLI does not import. It does not serve a Vault, which is `rola-daemon`, and does not keep the registry, whose path and format are `modules/daemon`'s.
