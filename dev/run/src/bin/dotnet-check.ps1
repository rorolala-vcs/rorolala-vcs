#Requires -Version 5.1
# Builds and tests the .NET side of the workspace — the Desktop program, the translations it reads,
# and the tests of those translations. The solution is built first, and deliberately: `test` builds
# only what a test project depends on, and nothing depends on the Desktop program, so without the
# build a Desktop program that does not compile would be found by neither this script nor `test`.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $DotnetProgram @DotnetFlags build $Solution -c Release
Assert-Exit

& $DotnetProgram @DotnetFlags test $Solution -c Release
Assert-Exit
