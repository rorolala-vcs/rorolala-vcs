#Requires -Version 5.1
# Everything that ships: the library and the programs.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

Invoke-Script lib
Invoke-Script bin
