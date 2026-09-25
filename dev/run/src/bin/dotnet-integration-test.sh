#!/bin/sh
# Runs the .NET integration suites under `tests/`. Like the cargo suites beside them, they run the
# built programs and check what they said, so what is checked is the program a caller meets rather
# than the parts it is made of; unlike them they are run by the .NET test host, since the Desktop
# program is a .NET program and its suites are written in C#.
#
# They are a solution of their own, so running them from beside it keeps them out of the workspace
# they test: `dotnet-check` never sees them, and the Desktop program is built here, by what they
# reference, rather than by them.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$DOTNET test "$IT_SOLUTION" -c Release
