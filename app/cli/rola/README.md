# rola

## Introduction

`rola` is the command line front end of Rorolala. It reads the run's language, output shape, pager, progress and the Vault or Workspace it stands in, then dispatches the commands that work on Vaults, Workspaces, accounts, keys, content and the Desktop window.

## Boundary

It owns the command tree, the global flags, help, completion, exit codes and the hand-off of a word that names no command to a program beside it, so `rola daemon` runs `rola-daemon`. It does not own the rules those commands apply, which live in `librorolala` and the `modules/` crates, nor the Desktop window, which is `app/desktop`, nor a Vault's protocol, which is `modules/daemon`. Results go to stdout while progress and failures go to stderr.
