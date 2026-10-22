#!/bin/sh
# Scores chunking strategies against the frozen general-purpose yardsticks.
#
# A corpus is a directory of version chains, and what this measures is what cutting them costs: how
# much of a chain the store ends up holding, how much one saved edit costs, and how fast the cut is.
# The tool names its own corpus (the one beside it), its own weights (`score.toml`) and its own
# options; this is only the way to run it, so that a report is produced the same way from a terminal
# and from a machine.
#
# Release rather than the default profile, unlike the test suites: one of the things measured is how
# fast a cut is, and a debug build measures rustc's unoptimised code rather than a chunker's. The
# floors in `score.toml` are numbers about a release build.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO run --quiet $RELEASE_FLAG -p rorolala-dev-chunker-harness -- "$@"
