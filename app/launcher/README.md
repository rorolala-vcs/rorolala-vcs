# ROLALauncher

## Introduction

`ROLALauncher` is the small Avalonia window a person starts first. It is a two-column main menu: the Vaults it knows on the left, the Workspaces this machine has opened on the right. The left column reads the `rola-daemon` address book and the addresses this launcher hosts, checks each one with a handshake, and lets a hosted address be added, edited or removed. An address's menu also opens the daemon's window, starts the daemon on the way up, and checks that address again once it is up. The right column creates, imports and opens a Workspace, chooses the account, and opens the Desktop program onto a chosen Workspace.

## Boundary

It owns the list in `workspaces.toml`, the hosted addresses in `vaults.toml`, its own `settings.toml` and the frame style kept there, its language choice, and waiting for the Desktop window to report that it is up. It reads the daemon's address book through `rola-daemon list` and never writes it. It does not own a Workspace, an account, a key or a Vault: those are read and changed through the `rola` beside it, which also checks a connection and opens the window as `rola desktop`. It does not own the Desktop program, which is `RorolalaDesktop`, and compiles that program's configuration and theme sources rather than restating them.
