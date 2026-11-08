# rola-daemon

## Introduction

`rola-daemon` serves the Vaults registered for this user, either as a daemon on a port for the machine or as one session over standard input and output for an `ssh` caller. A run that names nothing starts the daemon, and `list`, `check`, `register`, `remove` and `serve` maintain and inspect what it serves.

## Boundary

It owns the commands that read and change the namespace table, the port the daemon listens on, and the words and output shapes those commands answer with. It does not own the registry's path or format, nor the protocol and the content exchange, which are `rorolala-daemon` in `modules/daemon`, nor a Vault's layout and store, which are `librorolala`. It has no stop subcommand, so a caller ends it with the platform's own signal, which is what the `daemon-linux` and `daemon-win` windows do.
