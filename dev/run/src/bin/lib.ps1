#Requires -Version 5.1
# Builds the C ABI artifact — a release cdylib and staticlib — and, as a side effect of the root
# build script, the C header that describes it.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $CargoProgram @CargoFlags build $ReleaseFlag --all-features -p rorolala-ffi
Assert-Exit
