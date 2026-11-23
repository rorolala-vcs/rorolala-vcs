# Contributing

`tests/` holds the integration suites: programs that run the built programs the way a caller meets
them. A directory that holds a `Cargo.toml` is a cargo suite, a project whose name ends in
`.IntegrationTests` is a .NET suite, `tests/ffi-c` checks the C ABI as C, and `tests/ffi-cxx`
checks the C++ layer as C++.

## What can be contributed

- A cargo suite, by adding a directory with a `Cargo.toml`. It is a program rather than a `#[test]`,
  it is run with `ROLA_BIN_DIR` naming the built programs, and it ends non-zero when what came back
  was not what it expected.
- A .NET suite, by adding a directory with a project whose name ends in `.IntegrationTests`.
- A module under `tests/ffi-c`, by adding a directory `ffi-<area>/` with a `test.c`. The suite finds
  every one, and `harness.h` is what they share.
- A module under `tests/ffi-cxx`, by adding a directory `cxx-<area>/` with a `test.cpp`. The suite
  finds every one, and `harness.hpp` is what they share.

## What it needs

- Rust 1.85 or newer, `git`, and `openssl` for the suites that make a key pair.
- `sh` for the suite that stands in for an ssh server.
- A POSIX C compiler for `tests/ffi-c`, which is skipped on Windows and on a musl build, where there
  is no shared library to link.
- A POSIX C++23 compiler for `tests/ffi-cxx`, which is skipped for the same two reasons.

## Checks

`cargo ci integration-test` runs every cargo suite and then every .NET suite, and names each one
that failed.
