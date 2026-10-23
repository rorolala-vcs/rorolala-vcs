#!/bin/sh
# Migrated to `cargo ci test`; this is the strangler shim kept while both entry points exist. The
# logic lives in `dev/ci` now — see `__PLAN_BUILD_CI.md` in the working tree.
exec cargo run --quiet --manifest-path dev/ci/Cargo.toml --bin rola-ci -- test "$@"
