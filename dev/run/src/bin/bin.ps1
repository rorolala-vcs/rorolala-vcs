#Requires -Version 5.1
# Builds the release command line programs. Compiling them is also what refreshes the completion
# scripts in `$CompletionDir`.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

# A program is named to cargo one at a time, so that a name with a space in it would still be one
# name. The ones with no completions are built with the rest, since what the list drives here is
# which programs there are and not which of them a shell can be completed for.
$arguments = @('build', $ReleaseFlag, '--all-features')
foreach ($program in $Programs + $ProgramsWithoutCompletions) {
    $arguments += @('-p', $program)
}

& $CargoProgram @CargoFlags @arguments
Assert-Exit
