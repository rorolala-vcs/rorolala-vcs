# Contributing

`modules/` is the domain: the crates that know what a Workspace, a Vault, a Layout, a Store and a
version chain are. Each crate states its `## Introduction` and its `## Boundary`, and the second one
says which crate owns what it does not.

## What can be contributed

- Any crate here, within the boundary it states.
- A chunking policy under `modules/chunking`, with a corpus to measure it against. The corpus is not
  carried, so it is captured from the tool that writes the format, or pointed at with `--corpus`.
- The C# binding under `modules/rola-sharp`. It is generated from the C header by
  `dev/rsharp-bindgen`, so the generated `RorolalaBinding.cs` is not edited by hand.

## What it needs

- Rust 1.85 or newer and `git`.
- For `modules/rola-sharp`, the .NET SDK that `global.json` pins, and `cargo` on `PATH`. The Rust
  side is built first, because the generator reads the header from cargo's output.
- For a chunking policy, files of the format itself. `cargo dev chunker --capture <format>` writes
  them, and `cargo dev chunker --score` rewrites each `SCORE.md`.

## Checks

`cargo ci test`, `cargo ci clippy` and `cargo ci doc` cover the Rust side. `cargo ci generated`
rebuilds the policy registry and every `SCORE.md`, and fails when a committed file disagrees with
its sources. The binding is checked by `cargo ci dotnet-check`.
