# rola-ci

## Introduction

Runs every check of this tree as one command per check, and is reached as `cargo ci`, with `cargo ci check` running them in order and stopping at the first failure.

## Boundary

It owns the order, the exit status, and the one report a run leaves in `.cache/reports/`. It does not produce the artifacts a check runs against: `rola-build`, reached as `cargo dev`, is a library dependency here, so a check and the build that made the artifact agree on where it is. The checks it composes are other programs: the i18n scan is `rorolala-dev-i18n-check`, and the formatter, Clippy, the documentation build and the .NET analyzers are the tools it invokes.
