#Requires -Version 5.1
# Formats every crate in the workspace in place.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags fmt --all
Assert-Exit
