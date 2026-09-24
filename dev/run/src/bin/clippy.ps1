#Requires -Version 5.1
# Runs clippy over the workspace; warnings are errors.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags clippy --workspace --all-targets --all-features -- -D warnings
Assert-Exit
