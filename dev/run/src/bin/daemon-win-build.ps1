[CmdletBinding()]
param(
    [switch]$Check
)

$ErrorActionPreference = 'Stop'

# dev/run/src/bin -> the tree root. The script is self-contained on purpose: it must run in
# this standalone tree and, unchanged, in the rorolala tree after a directory-level merge.
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$sourceDir = Join-Path $root 'app\daemon-win'
$srcDir = Join-Path $sourceDir 'src'
$resDir = Join-Path $sourceDir 'res'

# `BUILD_DIR` is the same override common.ps1 honours; `build/bin` is the export layout.
$buildRoot = if ($env:BUILD_DIR) { $env:BUILD_DIR } else { Join-Path $root 'build' }
if (-not [System.IO.Path]::IsPathRooted($buildRoot)) {
    $buildRoot = Join-Path $root $buildRoot
}
$binDir = Join-Path $buildRoot 'bin'

# Intermediates stay out of the export directory, beside the other toolchains' output.
$objRoot = Join-Path $root '.cache\daemon-win'
$objDir = Join-Path $objRoot $(if ($Check) { 'obj-check' } else { 'obj' })
$bat = Join-Path $objRoot $(if ($Check) { 'check.cmd' } else { 'build.cmd' })
$exe = Join-Path $binDir 'rola-daemon-gui.exe'

if (-not (Test-Path -LiteralPath $srcDir)) {
    throw "Daemon sources were not found at $srcDir."
}

New-Item -ItemType Directory -Force -Path $binDir, $objDir | Out-Null

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw "vswhere.exe was not found. Install Visual Studio Build Tools with the C++ workload."
}

$installation = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installation) {
    throw "A Visual Studio installation with the C++ x64 tools was not found."
}

$vcvars = Join-Path $installation 'VC\Auxiliary\Build\vcvars64.bat'
if (-not (Test-Path -LiteralPath $vcvars)) {
    throw "vcvars64.bat was not found under $installation."
}

$analyze = if ($Check) { '/analyze' } else { '' }
$common = @(
    '/nologo'
    '/c'
    '/W4'
    '/WX'
    '/O2'
    '/std:c11'
    '/utf-8'
    '/DUNICODE'
    '/D_UNICODE'
    '/D_CRT_SECURE_NO_WARNINGS'
    "/I`"$resDir`""
)
if ($analyze) {
    $common += $analyze
}

$lines = @(
    '@echo off'
    "call `"$vcvars`" >nul"
    'if errorlevel 1 exit /b 1'
    "cd /d `"$resDir`""
    "rc /nologo /i `"$resDir`" /fo `"$objDir\app.res`" `"$resDir\app.rc`""
    'if errorlevel 1 exit /b 1'
    "cl $($common -join ' ') /Fo`"$objDir\\`" `"$srcDir\*.c`""
    'if errorlevel 1 exit /b 1'
    "cl /nologo /Fe:`"$exe`" `"$objDir\*.obj`" `"$objDir\app.res`" /link /SUBSYSTEM:WINDOWS /MANIFEST:NO user32.lib gdi32.lib comctl32.lib comdlg32.lib shell32.lib ole32.lib"
    'if errorlevel 1 exit /b 1'
    'exit /b 0'
)

Set-Content -LiteralPath $bat -Value $lines -Encoding ASCII

Push-Location $root
try {
    & cmd.exe /d /c "`"$bat`""
    $code = $LASTEXITCODE
}
finally {
    Pop-Location
}

if ($code -ne 0) {
    Write-Error "Build failed with exit code $code."
    exit $code
}

Write-Host "Built $exe"

# The program looks for rola-daemon.exe beside itself, so an export lays that program down
# first. A plain build may run before that, and says so rather than failing: building the C
# program does not depend on the Rust output.
$daemon = Join-Path $binDir 'rola-daemon.exe'
if (-not (Test-Path -LiteralPath $daemon)) {
    Write-Warning "rola-daemon.exe is not in $binDir yet; put it beside the GUI before running it."
}

exit 0
