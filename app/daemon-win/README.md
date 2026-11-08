# daemon-win

## Introduction

`daemon-win` is a small Win32 window over the machine's `rola-daemon`, the Windows twin of `daemon-linux`, with the same four tabs: Daemon, Registry, Keys and Admins.

## Boundary

It owns the Win32 drawing, including the light or dark look the desktop asks for, and the wiring of each control to a `rola-daemon` or `rola` command, run with `--no-color --no-progress` and found beside the window. It does not own a rule: a refusal from a command is shown as the program said it, and stopping a daemon is `TerminateProcess` because `rola-daemon` has no stop subcommand. It imports keys itself only because the CLI generates keys and does not import them. Key generation is `rola key generate`, and the registry and the serving are `rola-daemon`'s, whose rules live in `modules/daemon`.
