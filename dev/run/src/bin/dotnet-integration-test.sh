#!/bin/sh
# Runs the .NET integration suites under `tests/`. Like the cargo suites beside them, they exercise
# the Desktop program the way a caller meets it, so what is checked is the program rather than the
# parts it is made of; unlike them they are run by the .NET test host, since the Desktop program is a
# .NET program and its suites are written in C#.
#
# A suite is a project of its own, and is run on its own rather than through a solution. That is
# deliberate: a project a solution does not list is built with its own default configuration, so a
# solution holding only the suites would build the Desktop program they reference in Debug whatever
# `-c` said. Run this way, the release reaches everything a suite references.
#
# They run one at a time, so a suite that fails is named rather than lost among the output of the
# others — and the ones after it still run, so one run reports every suite that failed.
set -eu

. "$(dirname "$0")/../lib/common.sh"

failed=
for suite in tests/*/*.IntegrationTests.csproj; do
	echo "==> $suite"
	# shellcheck disable=SC2086
	if ! $DOTNET test "$suite" -c Release; then
		failed="$failed $suite"
	fi
done

if [ -n "$failed" ]; then
	echo "==> failed:$failed"
	exit 1
fi
