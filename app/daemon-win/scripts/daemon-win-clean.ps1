[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path

# Only this program's artifacts: the export directory is shared with the Rust programs, and
# they are not this script's to remove.
$buildRoot = if ($env:BUILD_DIR) { $env:BUILD_DIR } else { Join-Path $root 'build' }
if (-not [System.IO.Path]::IsPathRooted($buildRoot)) {
    $buildRoot = Join-Path $root $buildRoot
}

Remove-Item -LiteralPath (Join-Path $root '.cache\daemon-win') -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $buildRoot 'bin\rola-daemon-gui.exe') -Force -ErrorAction SilentlyContinue

Write-Host 'Cleaned daemon-win build outputs.'

