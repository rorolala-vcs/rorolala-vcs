#Requires -Version 5.1
# Removes everything the build ever produced: cargo's output, whatever else `$TargetDir` holds (the
# generated header, mingling's completion scripts), .NET's output, the Daemon window's intermediates
# and the exported `$BuildDir`.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

Invoke-Script cargo-clean

foreach ($directory in @($TargetDir, $CsTargetDir, '.cache/daemon-win', $BuildDir)) {
    if (Test-Path $directory) { Remove-Item -Recurse -Force $directory }
}
