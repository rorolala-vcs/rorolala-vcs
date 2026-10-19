[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

# The C program is checked by the same build with `/analyze` and warnings as errors.
& (Join-Path $PSScriptRoot 'daemon-win-build.ps1') -Check
exit $LASTEXITCODE
