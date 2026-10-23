# rola-ci

What rola is checked with.

This is the gate: one program, one command per check, one exit status. It is reached as
`cargo ci <command>`, through the alias in `.cargo/config.toml`. What *produces* the artifacts is
`rola-build` (`cargo dev`), which this crate depends on as a library — a check that needs an artifact
is run against the same code that produces it, so the two cannot disagree about where it is.

Like `rola-build`, it is a workspace of its own (see that crate's README for why).

## What is where

- `src/task/` — one module per check, in the order the gate runs them: `fmt`, `fmt-check`, `clippy`,
  `test`, `doc`, `doc-open`, `i18n`, `generated`, `integration-test`, `dotnet-check`, `dotnet-lint`,
  `dotnet-integration-test`. `check` (also reached as `all`) runs them in that order and stops at
  the first failure.
- `src/cmd/` — the commands that are about a run rather than about the tree: `report-collect`,
  `report-clean`, and the optional `git-lock`/`git-unlock`.
- `src/reporter.rs` — what a run leaves behind in `.cache/reports/`, and how it is rendered as one
  page.
- `src/git.rs` — the mechanics of the optional lock and unlock.

## What the gate does

`cargo ci check` composes the checks rather than repeating them: `fmt-check` first because it is
instant and the one most easily forgotten, `i18n` next for the same reason, then the tests, the
build of everything that ships, `generated` (which asks git whether the build changed the files it
generates), `clippy`, `doc`, the integration suites, and the two .NET checks. A failed step stops
the run with its own status, and what the run recorded is rendered afterwards, so a failed gate is
still one page.

The old entry point is still there: `./run.sh <name>` reaches the same command, through the shim
beside it.
