#Requires -Version 5.1
# The Desktop program on its own, laid out where an export lays it out.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

Publish-Desktop $DesktopDir
