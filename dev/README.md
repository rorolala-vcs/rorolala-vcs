# Contributing

`dev/` holds the tools this project is built and checked with: `cargo ci`, `cargo dev`,
`cargo changelog`, `cargo worktree-check`, the generators, the harnesses and the packaging assets.
Each tool is a workspace of its own rather than a member of the root one.

## What can be contributed

- A check under `dev/ci`, which runs as `cargo ci <name>`. Every step is a command of its own, and
  `check` is what composes them in the order they are read in.
- A build step under `dev/build`, which runs as `cargo dev <name>` and produces what the tree ships.
- A generator: `dev/bindgen` writes the C header, `dev/rsharp-bindgen` the C# binding,
  `dev/book-index` the built-in book's search file and `dev/i18n-check` the translation scan.
- A harness: `dev/chunker-harness`, reached as `cargo dev chunker`, and `dev/sync-harness`.
- A report: `dev/project-orientation` lists what each area is, `dev/ask-contributing` lists what
  each area offers a contributor, and `dev/worktree-check` reports the state of the repository
  before an edit.
- The packaging assets under `dev/packaging`, and the changelog recorded in `dev/CHANGELOG-DETAILS.md`
  through `cargo changelog`.

## What it needs

- Rust 1.85 or newer and `git`.
- Python 3 for `dev/project-orientation/list.py`, `dev/ask-contributing/list.py` and
  `dev/git-logo/fetch.py`, which also uses `curl`.
- Node.js 20 or newer for `cargo dev cli-help-editor`.
- Inno Setup 6.7.3 for the Windows installer, and nFPM 2.48.0 with appimagetool 1.9.1 for the Linux
  packages. A build downloads none of them: each is looked for under `.cache/toolchains` and then on
  `PATH`.
- A C compiler, `pkg-config` and GTK 4.10 or newer on Linux, because the daemon window is built into
  the export. The musl package additionally needs `musl-tools`.

## Checks

`cargo ci check` is the gate. A change to a tool is checked by `cargo ci test` and `cargo ci clippy`
on the tool's own workspace, and by the step it adds or changes. `cargo ci fmt-check` holds the
formatting.
