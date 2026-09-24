#Requires -Version 5.1
# Builds the API documentation, then opens it in a browser.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags doc --workspace --no-deps --all-features --open
Assert-Exit
