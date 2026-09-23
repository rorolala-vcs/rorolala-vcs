#Requires -Version 5.1
# Runs the test suites. Deliberately left on cargo's default profile: release builds turn
# `debug_assert!` off, which is the opposite of what a test run wants.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags test --workspace --all-features
Assert-Exit
