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

## FFI tests

- The C ABI is tested as C: `tests/ffi-c/` is a suite whose checks are C11 programs, compiled
  against the generated header and linked to the built library, so what is checked is the ABI a
  caller meets rather than the Rust under it. A test in Rust can only meet the same types again.
- A module is a directory `tests/ffi-c/ffi-<area>/` holding a `test.c`; the suite finds every one
  and runs it, so adding a module is adding a directory. What they share — a check, a way to end
  on them — is `tests/ffi-c/harness.h`.
- What a module checks is behaviour: making and locating, a lock held and given back, the
  tag-and-payload of a fallible export and the release that goes with it. Not the values of
  constants, which the header already spells out and a test would only pin a copy of.
- The suite builds `rorolala-ffi` itself, because the release programs do not depend on that
  crate: nothing else in the gate produces the library a C caller links.
- The programs are C11 (`cc -std=c11 -Wall -Wextra -Werror`), and only Unix is spelled out — the
  runner's `.ps1` twins have never been run.

## Subcommands

A command is a file, a node and a shape. What it carries before it is done:

- **Layering.** One file under the module of its area, declared by
  `#[command(node = "...", entry = Entry...)]`. The `#[command]` function parses arguments and
  pre-validates what can be refused on the spot — mutually exclusive modes, missing or unpaired
  positions — and returns `Next` for those. Everything else goes into a `StateXxx` value and a
  `#[chain]` handler: the entry turns arguments into state, the handler does the work.
  `#[chain(routeify)]` is what lets a handler use `?` on a failure the framework knows.
- **Global flags are consumed, not declared.** `--vault`, `--force` and `--offline` are picked out
  of the whole argument list and removed before any command runs. A command declares none of them:
  it asks for `&ResUsingVault`/`&ResForce`/`&ResOffline` and reads them. A command-level declaration
  is dead code, since the global pick eats the token first.
- **Network.** A command whose work is the exchange with the Vault is refused when `--offline` is
  set, with `ErrorOffline` (252), rather than reaching. A command that only fetches the Vault's
  Layout or content on the way to local work goes on without it. Content is brought on demand
  through `crate::fetch::Sources` — never by a bulk store sync — and only the explicit
  `storage ...` commands move whole stores. Check for what is missing immediately before the read
  that wants it, so nothing is fetched ahead of its use.
- **Exit codes.** Every way of failing a caller may branch on gets a constant in
  `src/exit_codes.rs`, with its `exit_codes.<key>` doc comment above it and that key stated in
  `i18n/exit_codes.yml` in both languages. Where the gate runs, an exit status is one byte: no
  constant above 255. Blocks are ten apart, one per subject, and the room is nearly spent.
  `src/exit_codes/explain.rs` is generated from the constants — never edited, always committed with
  them.
- **Help.** `#[help(buffer)]` renders `t!("<area>.help")`, and the usage line names every flag the
  command takes, the global ones it honours included.
- **Description.** `#[metadata(Entry...)]` and `desc_xxx() -> Description` exist, and the text is
  translated: it is what the command is listed and searched by.
- **Completion.** Every command registers `#[completion(Entry...)]` with
  `complete_xxx(ctx: ShellContext, ...) -> Suggest`, answering what only the run can know — bound
  Vault names, accounts, the address history, paths — and `suggest!()` where nothing is known.
- **i18n.** Every key a call site names is stated in `i18n/` in `en` and `zh-CN`, checked by
  `./run.sh i18n`. stdout is the contract, so results are drawn through `t!` while progress and
  errors go to stderr.
- **Errors.** A failure is a `Grouped` type implementing `Failure` (`name`, `reason`), registered
  with `failure!(Type)` and rendered by a `#[renderer(buffer)]` function that sets its exit code. The
  command's `# Errors` doc names every failure it can render.
- **Lints.** A handler's parameters are the resources the framework injects, so their number follows
  the command rather than a signature this project shaped; past seven it wants
  `clippy::too_many_arguments` excused with a reason, at module level, because `#[chain]` copies the
  function's attributes onto the struct it generates.
- **Tests.** A pure helper gets a `#[cfg(test)]` module in its own file; the commands themselves are
  checked by the integration suites under `tests/`, which run the built programs.

## The user

- Push back, with reasons: facts, code, cost, what it breaks. Never just "I do not like it", and
  never do the wrong thing quietly.
- The user designs the interface: never invent a command, flag or option. Ask first.
- The user reviews, runs CI and commits. Report what you verified, and how.

## The work

- Iterate aggressively: this project is young, so large breaking changes and wholly incompatible
  replacements are wanted rather than merely tolerated. Choose the right shape over the compatible
  one, and keep nothing alive only because it is already there.
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
  `clippy::nursery`. The C# side is held to the same standard: `./run.sh dotnet-lint` runs the
  analyzers at their widest (`AnalysisMode=All`) with every diagnostic an error, and the root
  `.editorconfig` names each rule that is excused and the reason it is there. What a rule excuses is
  a rule excused on purpose, not a tier lowered.
- stdout is the contract; errors and progress go to stderr.
- The integration suites are programs rather than `#[test]`s, run against the real binaries through
  `ROLA_BIN_DIR`. The CLI is deliberately lightly tested.
- Scratch files start with `__` and stay inside the project root, and go when done. Git gets
  read-only commands. No `cargo clean`.
- Do not assume the layout: the runner and the caches have moved before, so look before deciding a
  path.
- Compile what you claim. A change to the FFI surface is checked with a real C and C++ compiler
  against the generated header (`cc -std=c17 -Wall -Wextra -Werror -fsyntax-only`); that is what
  caught `struct X;` being nothing but a tag in C. `tests/ffi-c/` is the fuller check: it compiles
  and runs C11 against the header and the library.
