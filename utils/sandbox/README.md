# rorolala-utils-sandbox

## Introduction

The part of running a built program that every integration suite does: a scratch directory, the built programs, and a way to start, serve and stop one.

## Boundary

It owns the sandbox directory and its guard, finding the programs through `ROLA_BIN_DIR`, and turning one into a command, a finished run or a serving process. It does not own what a suite checks or the programs themselves: the suites under `tests/` state their own expectations, and the programs are built by `cargo dev`.
