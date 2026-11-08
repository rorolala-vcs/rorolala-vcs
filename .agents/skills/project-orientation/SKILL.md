---
name: project-orientation
description: Orient in rorolala before writing in an unfamiliar area. Runs `dev/project-orientation/list.py` to list every app, modules, utils and dev README's `## Introduction`, then gives the read order, the path of one command end to end, the fast checks and the known traps. Use on the first change in an area, or when the user asks how the project is organized or where something lives.
---

# Project Orientation

Load this skill the first time you write in, or need to understand, an area you have not read.
The READMEs state what each crate owns and where its boundary is. This skill tells you which ones
to read, and in what order.

## First command

Run the list from the repository root:

```sh
python dev/project-orientation/list.py
python dev/project-orientation/list.py modules
python dev/project-orientation/list.py app utils
python dev/project-orientation/list.py --json
```

The script reads the `## Introduction` section of every README under `app/`, `modules/`, `utils/`
and `dev/`. It prints the path of the module and its one paragraph. Use it to find the crate that
owns the thing you are about to change. The README beside the code is the authority; the script only
gathers the paragraphs.

## Read in this order

1. `book/vcs-knowledge/version.md`, `variant.md`, `manifest.md`, `storage.md`: the objects a version
   control system works in.
2. `modules/layout/README.md`, `modules/storage/README.md`, `modules/tree/README.md`: the three
   foundations a Workspace and a Vault are read through.
3. The README of the crate you will change, then its `src/lib.rs` module list.
4. The command's own file under `app/cli/rola/src/`, its top doc comment first.

Stop there unless the task needs more. The READMEs of the other crates are one command away.

## Layers

| Layer | Path | What lives there |
| --- | --- | --- |
| Programs | `app/` | `rola`, `rola-daemon`, `git-rola`, the C ABI, the Desktop, the launcher, the two daemon windows |
| Features | `modules/` | The domain: auth, layout, storage, tree, vcs, workspace, vault, protocol, daemon |
| Facilities | `utils/` | Shared conveniences: paths, configuration, FFI, CLI drawing, progress, test sandbox |
| Tools | `dev/` | `cargo ci`, `cargo dev`, the header and binding generators, the harnesses, the worktree check |

Only `app/` and `dev/` may depend on `modules/`. Anything may depend on `utils/`. The root `src/`
is `librorolala`, which ties the modules together for the command and the C ABI.

## One command end to end

`rola checkin` is a useful trace because it crosses most layers:

1. `app/cli/rola/src/cmd_checkin.rs` declares `#[command(node = "checkin", entry = EntryCheckin)]`.
   The function `checkin` picks the operands that can be refused on the spot and returns `Next` for
   those. The rest becomes a `StateCheckin` value.
2. `#[chain(routeify)] handle_checkin` receives `StateCheckin` and the resources the framework
   injects, such as `ResWorkspace`, `ResVCSIndex` and `ResRorolalaStorage`.
3. The handler reads `librorolala::layout`, `librorolala::vcs`, `librorolala::storage`,
   `librorolala::tree` and `librorolala::workspace`, and brings content through
   `crate::fetch::Sources`.
4. A `#[renderer(buffer)]` function draws a failure and sets its exit code.
5. The words come from `app/cli/rola/i18n/` through `t!`. Results go to stdout; progress and errors
   go to stderr.

A command that is one file, one node and one shape is the rule this follows. Use `cmd_checkin.rs` as
the example when the command you are reading is smaller.

## Fast checks

Start with the crate, not the gate:

```sh
cargo check -p <crate>
cargo ci i18n
cargo ci test
cargo ci check
cargo dev export
```

`cargo check -p` is the seconds-long answer. `cargo ci check` is the gate and is the only one whose
exit status matters. `cargo dev export` rebuilds the artifacts for hand-off at the end of a round.

## Traps

- `--vault`, `--force` and `--offline` are picked out of the whole argument list before a command
  runs. A command asks for `&ResUsingVault`, `&ResForce` or `&ResOffline`; declaring one is dead
  code.
- Generated files are never edited: the C header, `app/cli/rola/src/exit_codes/explain.rs`, the
  built-in book's search file, the C# `RorolalaBinding.cs`, and the icon READMEs and pictures.
- `dev/ci`, `dev/build`, `dev/changelog` and `dev/worktree-check` are workspaces of their own, not
  members of the root workspace. `tests/` is a workspace of its own as well.
- `cargo clean` is not used. Scratch files start with `__` and stay inside the project.
- A crate README has `## Introduction` and `## Boundary` and nothing else. The first says what the
  crate is for, the second what it does not own.

## Before the first edit

Load the `worktree-check` skill and run:

```sh
cargo worktree-check --fetch
```

It reports the branch, `HEAD`, the local changes and the official remote, and records when the
fetch happened.
