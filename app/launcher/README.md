# ROLALauncher

## Introduction

`ROLALauncher` is the small Avalonia window a person starts first: it lists the Workspaces this machine has opened, creates or imports one, chooses the account, checks a Vault address, and opens the Desktop program onto a chosen Workspace.

## Boundary

It owns the list in `workspaces.toml`, its own `settings.toml` and the frame style kept there, the banner, its language choice, and waiting for the Desktop window to report that it is up. It does not own a Workspace, an account, a key or a Vault: those are read and changed through the `rola` beside it, which also checks a connection and opens the window as `rola desktop`. It does not own the Desktop program, which is `RorolalaDesktop`, and compiles that program's configuration and theme sources rather than restating them.
