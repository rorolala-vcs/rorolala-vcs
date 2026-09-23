#Requires -Version 5.1
# Builds the API documentation for every crate in the workspace.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags doc --workspace --no-deps --all-features
Assert-Exit
