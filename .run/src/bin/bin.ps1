#Requires -Version 5.1
# Builds the release command line programs. Compiling them is also what refreshes the completion
# scripts in `$CompletionDir`.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

# A program is named to cargo one at a time, so that a name with a space in it would still be one
# name.
$arguments = @('build', $ReleaseFlag, '--all-features')
foreach ($program in $Programs) {
    $arguments += @('-p', $program)
}

& $CargoProgram @CargoFlags @arguments
Assert-Exit
