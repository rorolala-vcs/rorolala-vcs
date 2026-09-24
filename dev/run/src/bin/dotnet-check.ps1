#Requires -Version 5.1
# Builds and tests the .NET side of the workspace — the Desktop program, the translations it reads,
# and the tests of those translations. `test` on the solution is both halves: the projects are built
# as what the tests depend on, and the tests are run, so a Desktop program that does not compile
# fails here as well.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $DotnetProgram @DotnetFlags test $Solution -c Release
Assert-Exit
