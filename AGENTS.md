# Role

Act as a professional Rorolala VCS developer and carry out the user's request.

- `rorolala` is a version control system for artists, whose material is binary files: PSD, PPT and the like.
- `rola` is the command line program, serving developers, debuggers, scripts and AI agents.
- `RorolalaDesktop` is the desktop program (`app/desktop`, Avalonia), and it is the final form.

To learn more about the project, load the `project-orientation` SKILL.

## Language

Match the language the user speaks: never answer a Chinese speaker in English, nor an English speaker in Chinese.

## Comments and docs

- Comments state facts and reasons (WHY), never what the code does (WHAT): why something is or is not done, and what constrains it.
- Code comments and docs use English, imperative or present tense, never describing how things used to be, focused on the present.
- Public items must be documented (`missing_docs` is enabled). A C header keeps only its first line and the `# FFI` section, so words for C readers go under `# FFI`; see `dev/bindgen`.

### README

- Each crate's README states only its `## Introduction` and `## Boundary`, nothing else. The former explains what the crate is for, the latter what it does not handle and which crate does. `dev/project-orientation/list.py` reads the former.

### Escapes

- Mark every escape hatch and say why it holds, using the marker comments in the table below, and write what it bypasses and when it can go:

  | Marker           | Where used                  | States                                |
  | ---------------- | --------------------------- | ------------------------------------- |
  | `// SAFETY:`     | `unsafe`                    | why the constraints of this code hold |
  | `// UNWRAP:`     | panicking `unwrap`/`expect` | why it will not panic                 |
  | `// PANIC:`      | panic paths                 | why panic here                        |
  | `// WORKAROUND:` | detours                     | what it bypasses, when it can go      |

## Search

- Prefer `rg`, then `grep`, always excluding caches and history (`.cache`, `tests/.cache` and `.git`).
- **Always** set a TIMEOUT, never above 20s (the user's maximum tolerance).

So think clearly about what you want to find this time, narrow it with directory and file type limits, then let it finish.
A search that needs a longer timeout shows the search was not clever enough: **narrow it instead of waiting**.

```sh
rg --timeout 15 -g '*.rs' 'fn main' modules/
rg --timeout 15 -g '!{.cache,tests/.cache,.git}' -t md 'Vault' dev/
```

```sh
grep -rn --include='*.rs' -e 'fn main' modules/
grep -rn --include='*.md' --exclude-dir={.cache,tests/.cache,.git} -e 'Vault' dev/
```

## Text and punctuation

This closed list of sentence punctuation covers what users read at runtime: the i18n files under `app/cli/*/i18n/` and `app/desktop/**/i18n/`, and any string the program prints or draws. Code, comments and developer docs are outside this list.

The banned symbols apply to every newly written sentence, wherever it is written: user-facing copy, comments, docs and commit messages. Existing text is not rewritten merely because of this rule. Typographic dashes or decorative symbols do not make text easier to read; they only draw the eye and annoy the reader.

- **Before writing user-facing copy, load the `write-documents` skill.**
- Sentence punctuation in user-facing copy comes only from the following set. Any other sentence punctuation here is a defect:
  - Chinese: `：` `，` `。` `、` `“` `”` `；` `！` `？`
  - English: `:` `,` `.` `"` `;` `!` `?`
- Banned in all languages and all newly written text: the dash family (`—`, `——`, `–`, `‒`, `―`, `−`, `⸺`, `⸻`) and decorative symbols (`…`, `·`, `•`, `〜`, `～`, and the like).
- Backticks wrap code, identifiers, paths and commands in pairs. Markdown markers, brackets, parentheses, hyphens and apostrophes inside words are not sentence punctuation.
- Before delivering, run `.agents/skills/write-documents/scripts/check_symbols.py` on every file you wrote or changed.

## Dependencies and other projects

- The CLI framework mingling lives at https://github.com/mingling-rs/mingling; it is taken from a git revision on the 0.6 branch, not the crates.io release, and the revision and features are written in the root `Cargo.toml`.
- Framework problems (hooks, picking, completion, rendering, argument parsing) are fixed in mingling, with an entry in its `CHANGELOG.md`. A detour in `rola` only means hitting it again at the next upgrade.
- Dependencies are added through `[workspace.dependencies]`, new crates are registered in `members`. `tests/` is a separate workspace with its own lock file.

## Layering

- `app/`, the user side: programs, the C ABI, the desktop program; may use `modules/` and `utils/`.
- `dev/`, development tools: two build crates (`dev/build` is `cargo dev`, producing artifacts; `dev/ci` is `cargo ci`, checking the tree), the C header generator, and other tools both run; may use `modules/` and `utils/`.
- `modules/`, functional modules in Rust and C#. Only `app/` and `dev/` may depend on them; they may depend on each other but must never form a cycle.
- `utils/`, shared infrastructure in Rust and C#, which anything may depend on. They are not supposed to depend on each other; the exceptions are an implementation plus proc-macro split of one family (`lazyffi`, `lazyffi-core`, `lazyffi-macros`, `configure-macros`), and the crates using `lazyffi` export macros (`configure`, `constants`). Both kinds are one-directional; none is a cycle.
- The root `src/`, that is `librororala`, wires the modules together for the CLI and the C ABI.
- `tests/<name>/`, integration test suites, each its own workspace, run as real programs.
- Generated files are never edited: headers are produced by the root `build.rs` scanning `src`, `modules` and `utils`, not `app` or `dev`, so the FFI surface lives in those three trees.

## Before writing code

- Run `cargo worktree-check --fetch` before the first edit. It reports how long ago the last recorded fetch was, the `HEAD` branch and commit, whether it is detached, staged, unstaged and untracked paths, and the remotes, identifying the one at `github.com/rorolala-vcs/rorolala-vcs`. The record is written to `dev/worktree-check/__last_fetch`, which git ignores.
- Work on a branch off `main`: `wip/<name>` for work in progress, `fix/<name>` for fixes. First look for an issue in the official repository, using `gh` when installed, otherwise `curl` or a fetch tool against `https://api.github.com/search/issues?q=repo:rorolala-vcs/rorolala-vcs+<terms>`.
- When the user asks to merge, squash or rebase, fast-forward. No merge commits: `main` stays linear. The `worktree-check` skill has the corresponding commands.

## FFI tests

- The C ABI is tested the C way: `tests/ffi-c/` is a suite whose checks are C11 programs, compiled against the generated header and linked to the built library, so what is checked is the ABI the caller faces, not the Rust underneath. Tests written in Rust can only meet the same types again.
- A module is a directory `tests/ffi-c/ffi-<area>/` containing `test.c`; the suite finds and runs every one, so adding a module is adding a directory. What they share (a check, an ending) is in `tests/ffi-c/harness.h`.
- A module checks behaviour: creation and locating, one lock and its return, the tag and payload of a fallible export and their corresponding release. It does not check constant values, the header already states them and a test would only copy them.
- The suite builds `rorolala-ffi` itself, because the release program does not depend on that crate: nothing else in the gate produces the library the C caller links to.
- The programs are C11 (`cc -std=c11 -Wall -Wextra -Werror`) and only explicitly Unix: the suite compiles with `cc` and links with `-Wl,-rpath,`.

## Subcommands

A command is one file, one node and one shape. Before it is done it needs:

- **Layering.** One file under its domain's module, declared with `#[command(node = "...", entry = Entry...)]`. The `#[command]` function parses arguments and pre-validates what can be rejected on the spot (mutually exclusive modes, missing or unpaired positions), returning `Next` for those. Everything else goes into a `StateXxx` value and a `#[chain]` handler: the entry turns arguments into state, the handler does the actual work. `#[chain(routeify)]` lets the handler use `?` on failures the framework understands.
- **Global flags are consumed, not declared.** `--vault`, `--force` and `--offline` are picked out of the whole argument list and removed before any command runs. A command declares none of them: it requests `&ResUsingVault`/`&ResForce`/`&ResOffline` and reads them. A command-level declaration is dead code, because global picking eats that token first.
- **Network.** A command whose work is exchanging with the Vault rejects with `ErrorOffline` (252) when `--offline` is set, rather than connecting. A command that only fetches Vault Layout or content on the way to local work carries on without it. Content is fetched on demand through `crate::fetch::Sources`, never through a whole-repository sync, only explicit `storage ...` commands move the whole repository. Check what is missing right before it is needed to read, so nothing is fetched early.
- **Exit codes.** Every failure mode a caller might branch on gets a constant in `src/exit_codes.rs`, with an `exit_codes.<key>` doc comment above it, and that key written in both languages in `i18n/exit_codes.yml`. Where the gate runs, the exit status is one byte: no constant above 255. Blocks are ten apart, one per topic, and space is nearly exhausted. `src/exit_codes/explain.rs` is generated from the constants. Never edit it, and commit it when the constants change.
- **Help.** `#[help(buffer)]` renders `t!("<area>.help")`, and the usage line lists every flag the command accepts, including the global flags it honours.
- **Description.** `#[metadata(Entry...)]` and `desc_xxx() -> Description` exist, with translated text: they are how a command is listed and searched.
- **Completion.** Every command registers `#[completion(Entry...)]` with `complete_xxx(ctx: ShellContext, ...) -> Suggest`, answering what only the runtime knows (bound Vault names, accounts, address history, paths), and using `suggest!()` when it knows nothing.
- **i18n.** Every key a call site names is written in both `en` and `zh-CN` under `i18n/`, checked by `cargo ci i18n`. stdout is the contract, so results are drawn through `t!`, while progress and errors go to stderr.
- **Errors.** A failure is a `Grouped` type implementing `Failure` (`name`, `reason`), registered with `failure!(Type)`, and rendered by a `#[renderer(buffer)]` function that sets its exit code. The command's `# Errors` doc lists every failure it can render.
- **Lint.** Handler parameters are framework-injected resources, so their count depends on the command, not on a signature this project shapes; above seven, `clippy::too_many_arguments` needs a module-level exception with a reason, because `#[chain]` copies the function's attributes onto the struct it generates.
- **Tests.** Pure helper functions have a `#[cfg(test)]` module in their own file; the commands themselves are checked by the integration suites under `tests/`, which run the built programs.

## User

- Disagree with reasons: facts, code, cost, what it would break. Never just "I do not like it", and never quietly do the wrong thing.
- The user designs the interface: never invent commands, flags or options. Ask first.
- The user handles review, running CI and committing. Report what you verified and how.

## Asking

If the agent you work as offers no Question tool and you need one, overwrite your questions in `__QUESTION.md` in the user's native language, then tell the user and stop the session.

Template:

```markdown
# Questions?

1. Question text (single choice / multiple choice)

- [ ] Option Alpha (recommended)
- [ ] Option Beta
- [ ] Option Delta
- [ ] Other:
```

## Work

- Iterate aggressively: the project is young, so large breaking changes and wholly incompatible replacements are expected, not merely tolerated. Choose the right shape over the compatible shape, and do not keep something merely because it already exists.
- Go as far as you can: read the code, run it, compile it. Do not say "maybe", do not say "probably". State in detail what you did not do, why, and what is needed next.
- Keep asking back on unclear requests, offering options and their costs, until the plan is clear.
- Before starting, **restate** the plan in your own words, listing every decision: types, names, locations, what breaks, who calls whom. **Restate! Restate! Restate!**

- **Lay the build out for hand-off every round, so the user can test directly**, unless the user explicitly says not to. `cargo dev export` lays out everything under `build/`. When only part of the tree changed, export just that part to save debugging time:
  - `export-lib`: the C ABI into `build/lib`, with the C++ include tree, the CMake package, the C# bindings and the two `NuGet` packages
  - `export-cli`: the programs into `build/bin`
  - `export-desktop`: the Desktop program, its host and its launcher into `build/bin/desktop`. It builds against the C ABI already there and neither builds the plugins or the daemon window nor redraws the icons
  - `export-book`: the book's search file into `build/docs`
  - `export-daemon-gui`: the window over the daemon into `build/bin`
  - `export-desktop-plugin`: the plugins into `build/bin/desktop/plugins`

  For anything further, read `dev/build/help.txt` rather than guessing.

- **Each `export-*` builds only the part it names and drops it into `build/`.** It makes no promise that the rest of the layout is present or consistent; that is the full `export`'s job. The point of the per-part commands is a short loop for a change to that one part.
- Each part has a build command and an export command: `lib`, `cli`, `book`, `desktop`, `daemon-gui`, `desktop-plugin` build; `export-*` builds and lays out. A part placed only in the export, the book and the daemon window, is written there by both.
- **A build command is a partial build and ignores the heavy dependencies.** `desktop` builds only the three Desktop programs (the window, the host and the launcher) against the Rust output already there, and `desktop-plugin` builds only the plugin projects; neither rebuilds the C ABI or the programs. That is fast and may fail when that output is stale or absent, and a failure is the signal to take the full `export` path, which rebuilds what it needs.

## Mechanics

- The gate is `cargo ci check`, and its exit code is the only signal it emits; read it like this: `cargo ci check > /tmp/log 2>&1; echo $?`. The steps are the commands of `dev/ci` and `dev/build`.
- The full gate `cargo ci check` runs `fmt`, `fmt-check`, `i18n`, `test`, `clippy`, `doc`, `generated`, `integration-test`, `dotnet-lint` and `dotnet-check` in order, usually taking 20 to 30 minutes. Running it every round is too slow. While iterating, run only the steps the change touches, against the surface it touches; run the full gate when the user asks, or when the work is ready to commit.
- Every crate denies `warnings`, `missing_docs`, `rust_2018_idioms`, `clippy::pedantic` and `clippy::nursery`. The C# side follows the same standard: `cargo ci dotnet-lint` runs the analyzers in their widest mode (`AnalysisMode=All`), every diagnostic is an error, and the root `.editorconfig` states each exempted rule with its reason. What a rule exempts is a rule exempted with reason, not a lowered bar.
- stdout is the contract; errors and progress go to stderr.
- Integration suites are programs rather than `#[test]`, running against the real binary through `ROLA_BIN_DIR`. The CLI is deliberately tested lightly.
- Temporary files start with `__` and stay inside the project root, deleted once used. git is used only for read-only commands. Do not `cargo clean`.
- Do not assume the directory layout: the runner and cache have moved before, so look before deciding a path.
- Compile what you claim. Changes to the FFI surface are checked with real C and C++ compilers against the generated header (`cc -std=c17 -Wall -Wextra -Werror -fsyntax-only`); this is exactly what caught that `struct X;` is only a tag in C. `tests/ffi-c/` is the fuller check: it compiles and runs C11 against the header and the library.
