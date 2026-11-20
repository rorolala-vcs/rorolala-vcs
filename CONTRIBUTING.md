# Contributing ⭐

Thank you for taking an interest in Rorolala!

This page says where a change starts, what the machine
needs and what the gate checks. The rules a change is held to are in `AGENTS.md`.

## 🤖 Ask your agent first

Every area of the tree states what can be contributed there and what it needs, in the
`# Contributing` section of its README. If you work with an agent, ask it:

> "
> Which parts of this project are open to contributions?
> "

The `ask-contributing` skill runs `dev/ask-contributing/list.py`, which collects those sections, so
the answer comes from the tree rather than from a list that goes stale. Reading it yourself is one
command:

```sh
python dev/ask-contributing/list.py
python dev/ask-contributing/list.py app modules
```

## The machine

The sections name what each area needs. These are the ones that apply almost everywhere:

- Rust 1.97 or newer. The workspace is edition 2024.
- `git`. `mingling` is taken from a git revision rather than the crates.io release.
- On Windows, the MSVC build tools and the Windows SDK.
- The .NET SDK that `global.json` pins, for the desktop program, the launcher and the C# binding.
  `cargo` is needed on `PATH` as well, because the binding is generated from the C header.

## Before the first edit

Work on a branch off `main`: `wip/<name>` for work in progress, `fix/<name>` for a fix. Run
`cargo worktree-check --fetch` first, which reports the branch, the commit and the local changes.

## What the gate checks

`cargo ci check` runs every step in the order below and stops at the first failure. Its exit status
is the only thing it signals. The whole gate takes 20 to 30 minutes, so while you work, run only the
steps the change reaches, with `cargo ci <step>`.

| Step               | What it checks                                                                                                                                                                                                 |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `fmt-check`        | Every crate is formatted, without rewriting anything.                                                                                                                                                          |
| `i18n`             | Every translation key a call site names is stated in both languages.                                                                                                                                           |
| `test`             | The workspace's own tests, on the debug profile, where `debug_assert!` still holds.                                                                                                                            |
| `build`            | The library and every program build in release, with all features.                                                                                                                                             |
| `generated`        | The registry and the scores the build writes into the tree agree with their sources. The score half measures whatever corpus is on the machine, and the corpus is not carried.                                 |
| `clippy`           | The workspace lints clean, with warnings as errors.                                                                                                                                                            |
| `doc`              | The API documentation of every crate builds.                                                                                                                                                                   |
| `integration-test` | Every cargo suite, then every .NET suite, run against the built programs. The C ABI's suite compiles C11 against the generated header and runs only where a POSIX C toolchain and a shared library both exist. |
| `dotnet-lint`      | The .NET side rebuilds with every analyzer rule on and any diagnostic failing the run.                                                                                                                         |
| `dotnet-check`     | The .NET solution builds and its tests pass.                                                                                                                                                                   |

## What a change is expected to carry

- A changelog entry, recorded with `cargo changelog`, when the change reaches what ships.
- Both languages for every user-facing string.
- A `#[cfg(test)]` module for a pure helper, and an integration suite for a change to what a program
  does.

`cargo dev export` rebuilds the artifacts at the end of a round, so what you hand over is what you
tested.

## Contributors

<img src="https://contrib.rocks/image?repo=rorolala-vcs/rorolala-vcs" />
