#!/bin/sh
# The full gate: the formatting of every crate, the test suites, a build of every crate, clippy, the
# documentation, the integration suites, then the .NET side — the C# linter first, then the build and
# the tests. Composed from the scripts beside this one rather than repeating what they do.
#
# `fmt-check` comes first because it is instant and it is the one that is forgotten: `fmt` rewrites
# the sources in place, so nothing else here would notice a crate that was left unformatted.
set -eu

. "$(dirname "$0")/../lib/common.sh"

again fmt-check
again test
again build
again clippy
again doc
again integration-test
again dotnet-lint
again dotnet-check
