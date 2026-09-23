#Requires -Version 5.1
# Verifies that every crate is formatted, without rewriting anything.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags fmt --all -- --check
Assert-Exit
