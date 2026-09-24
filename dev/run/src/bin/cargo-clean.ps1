#Requires -Version 5.1
# Removes cargo's build output. `$BuildDir` survives: it holds an export, not a build.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags clean
Assert-Exit
