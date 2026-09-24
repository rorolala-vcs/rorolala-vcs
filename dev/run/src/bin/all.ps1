#Requires -Version 5.1
# The full gate, and nothing besides: the gate already builds everything that ships, so asking for a
# build after it would be asking twice.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

Invoke-Script check
