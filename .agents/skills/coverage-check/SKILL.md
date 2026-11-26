---
name: coverage-check
description: Check how much of the Rust code the tests execute, using cargo ci coverage. Covers the prerequisites, the run itself, what is instrumented and ignored, where the summary, the lcov file and the html pages are written, and how to read them. Use when the user asks to check coverage, measure test coverage, or read a coverage report.
---

# Coverage Check

Coverage is measured by running the workspace tests and the integration suites under LLVM
source-based coverage. The number is what those runs actually executed, not what the tests mention.

## Prerequisites

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.8.7 --locked
```

The version is pinned to what CI installs, so a local run and the CI job agree. `cargo ci coverage`
stops with the install advice when either piece is missing.

## Run it

```sh
cargo ci coverage
```

It is not part of `cargo ci check` and is not a gated step. It needs an instrumented build of the
programs and the C ABI, so it takes longer than a normal test run.

The exit status is the only signal. Zero means both phases passed and the report was written. A
non-zero status means a phase failed; the report is still written so the failure can be read.

## What it runs

1. `cargo llvm-cov clean --profraw-only` drops profraw files from earlier runs, so the report is of
   this run only.
2. `cargo llvm-cov show-env` gives the instrumenting environment.
3. `cargo ci test` and `cargo ci integration-test` run as children under that environment, so the
   phases are exactly the gate's own commands.
4. The summary is printed, then `lcov.info` and the html pages are written.

## What is and is not counted

Only `src/` and `modules/` are counted. `app/`, `dev/`, `tests/`, `utils/`,
`modules/protocol-macros`, `build.rs` and registry sources are instrumented but ignored: a line
about them is a line about compilation, not about behaviour. The C# side is not instrumented.

The `ffi-c` and `ffi-cxx` suites are skipped on Windows because their C and C++ toolchains are not
POSIX, and on musl builds because there is no shared library to link. CI measures on Linux, where
those suites run, so the complete picture comes from the Linux job.

## Where the report is

- Terminal: the summary of region, function and line coverage for the workspace.
- `.cache/reports/coverage/html/index.html`: per file and per line, open this to find what is not
  executed.
- `.cache/reports/coverage/lcov.info`: the same data for other tools.
- CI uploads `.cache/reports/coverage/` as the `coverage-report` artifact.

Running `cargo ci coverage` again rewrites all three.

## A trap: hand-written C exports

The workspace report and a single crate report can disagree for a file whose functions are
`#[unsafe(no_mangle)] extern "C"` and are also called from Rust. In the workspace report those
bodies are inlined, the standalone copies never run, and the lines read as uncovered. The same
file measured on its own shows them covered:

```sh
cargo llvm-cov -p <crate> --lib --summary-only
```

Three files are in this state: `modules/workspace/src/ffi.rs`, `modules/vault/src/ffi.rs` and
`modules/desktop-pty/src/pty.rs`. They read 63 to 65 percent in the workspace report while the
single crate run reads about 94 to 98. For these three, the single crate number is the truth.

Do not add `black_box` or `#[inline(never)]`, and do not split a thin wrapper from an inner
function, to chase the workspace number: each was tried and each made the merge report the file
twice and lower it further.
