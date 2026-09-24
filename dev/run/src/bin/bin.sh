#!/bin/sh
# Builds the release command line programs. Compiling them is also what refreshes the completion
# scripts in `$COMPLETION_DIR`.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# A program is named to cargo one at a time, so that a name with a space in it would still be one
# name.
set --
for program in $PROGRAMS; do
	set -- "$@" -p "$program"
done

# shellcheck disable=SC2086
$CARGO build $RELEASE_FLAG --all-features "$@"
