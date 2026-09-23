#!/bin/sh
# Builds and tests the .NET side of the workspace — the Desktop program, the translations it reads,
# and the tests of those translations. `test` on the solution is both halves: the projects are built
# as what the tests depend on, and the tests are run, so a Desktop program that does not compile
# fails here as well.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$DOTNET test "$SOLUTION" -c Release
