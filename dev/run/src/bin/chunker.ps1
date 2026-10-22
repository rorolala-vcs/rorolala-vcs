#Requires -Version 5.1
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
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags run --quiet $ReleaseFlag -p rorolala-dev-chunker-harness -- @args
Assert-Exit
