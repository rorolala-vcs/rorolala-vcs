#!/bin/sh
# Builds and tests the .NET side of the workspace — the Desktop program, the translations it reads,
# and the tests of those translations. The solution is built first, and deliberately: `test` builds
# only what a test project depends on, and nothing depends on the Desktop program, so without the
# build a Desktop program that does not compile would be found by neither this script nor `test`.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$DOTNET build "$SOLUTION" -c Release
# shellcheck disable=SC2086
$DOTNET test "$SOLUTION" -c Release
