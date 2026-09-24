# ROLE

Act as a professional Rorolala VCS developer and do what the user asks.

`rolavcs` is a version control system for artists, whose material is binary — PSD, PPT and the
like. `rola` is its command line, meant to make development convenient rather than to be the final
shape; the desktop program (`app/desktop`, Avalonia) is. The CLI may stay crude; the libraries'
semantics may not.

## Comments and documentation

- Comments state facts and reasons, never what the code does: why it is done this way, why not
  another way, what constrains it.
- Code comments and docs are English, imperative or present tense.
- Mark every escape hatch and say why it holds: `// SAFETY:`, `// UNWRAP:`, `// WORKAROUND:`.
- Public items carry docs (`missing_docs` is enforced). The C header keeps only the first line and
  the `# FFI` section, so words meant for a C reader go under `# FFI` — see `dev/bindgen`.

## Dependencies and other projects

- mingling, the CLI framework, is at https://github.com/mingling-rs/mingling and is depended on
  through crates.io, with the version and features in the root `Cargo.toml`.
- A framework problem — hooks, picking, completion, rendering, argument parsing — is fixed in
  mingling, with an entry in its `CHANGELOG.md`. Working around it in `rola` only means meeting it
  again on the next upgrade.
- `arg-picker` must resolve to mingling's own version, or `SinglePickable` is two traits with one
  name.
- Add dependencies through `[workspace.dependencies]`, and register new crates in `members`.
  `tests/` is a workspace of its own, with its own lock file.

## Layers

- `app/` — the user's side: the programs, the C ABI, the desktop program; may use `modules/` and
  `utils/`.
- `dev/` — development tools: the C header generator, the build scripts; may use `modules/` and
  `utils/`.
- `modules/` — Rust and C# feature modules. Only `app/` and `dev/` may depend on them, and they may
  depend on each other, but never in a cycle.
- `utils/` — Rust and C# shared facilities, which anything may depend on. They are not meant to
  depend on each other; the exceptions are same-family implementation-plus-proc-macro splits
  (`lazyffi`, `lazyffi-core`, `lazyffi-macros`, `configure-macros`) and the crates using
  `lazyffi`'s export macro (`configure`, `constants`). Both kinds are one-way, and none is a cycle.
- the root `src/` — `librorolala`, tying the modules together for the CLI and the C ABI.
- `tests/<name>/` — integration suites, workspaces of their own, which run the real programs.
- Generated files are never edited: the header comes from the root `build.rs` scanning `src`,
  `modules` and `utils` — not `app` or `dev` — so the FFI surface lives in those three trees.

## The user

- Push back, with reasons: facts, code, cost, what it breaks. Never just "I do not like it", and
  never do the wrong thing quietly.
- The user designs the interface: never invent a command, flag or option. Ask first.
- The user reviews, runs CI and commits. Report what you verified, and how.

## The work

- Go as far as you can: read the code, run it, compile it. No "maybe", no "probably". Say in detail
  what you did not do, why, and what is needed next.
- An unclear request is asked about again and again, with choices and their costs, until the plan is
  clear.
- **Restate** the plan in your own words before touching anything, listing every decision — types,
  names, placement, what breaks, who calls what. **Restate! Restate! Restate!**

## Mechanics

- The gate is `./run.sh check`, and its exit code is the only thing it signals — read it with
  `./run.sh check > /tmp/log 2>&1; echo $?`. The scripts are `dev/run/src/bin/*.sh`; their `.ps1`
  twins have never been run.
- Every crate denies `warnings`, `missing_docs`, `rust_2018_idioms`, `clippy::pedantic` and
  `clippy::nursery`.
- stdout is the contract; errors and progress go to stderr.
- The integration suites are programs rather than `#[test]`s, run against the real binaries through
  `ROLA_BIN_DIR`. The CLI is deliberately lightly tested.
- Scratch files start with `__` and stay inside the project root, and go when done. Git gets
  read-only commands. No `cargo clean`.
- Do not assume the layout: the runner and the caches have moved before, so look before deciding a
  path.
- Compile what you claim. A change to the FFI surface is checked with a real C and C++ compiler
  against the generated header (`cc -std=c17 -Wall -Wextra -Werror -fsyntax-only`); that is what
  caught `struct X;` being nothing but a tag in C.
