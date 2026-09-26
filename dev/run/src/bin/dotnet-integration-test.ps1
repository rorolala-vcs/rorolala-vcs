#Requires -Version 5.1
# Runs the .NET integration suites under `tests/`. Like the cargo suites beside them, they exercise
# the Desktop program the way a caller meets it, so what is checked is the program rather than the
# parts it is made of; unlike them they are run by the .NET test host, since the Desktop program is a
# .NET program and its suites are written in C#.
#
# A suite is a project of its own, and is run on its own rather than through a solution. That is
# deliberate: a project a solution does not list is built with its own default configuration, so a
# solution holding only the suites would build the Desktop program they reference in Debug whatever
# `-c` said. Run this way, the release reaches everything a suite references.
#
# They run one at a time, so a suite that fails is named rather than lost among the output of the
# others — and the ones after it still run, so one run reports every suite that failed.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

$failed = @()
foreach ($suite in Get-ChildItem -Path tests -Recurse -Filter '*.IntegrationTests.csproj' | Sort-Object FullName) {
    $relative = Resolve-Path -Relative $suite.FullName
    Write-Host "==> $relative"

    & $DotnetProgram @DotnetFlags test $suite.FullName -c Release
    if ($LASTEXITCODE -ne 0) { $failed += $relative }
}

if ($failed.Count -gt 0) {
    Write-Host "==> failed: $($failed -join ' ')"
    exit 1
}
