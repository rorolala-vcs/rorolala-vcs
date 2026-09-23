#Requires -Version 5.1
# The full gate: the formatting of every crate, the test suites, a build of every crate, clippy, the
# documentation, the integration suites, then the .NET side. Composed from the scripts beside this
# one rather than repeating what they do.
#
# `fmt-check` comes first because it is instant and it is the one that is forgotten: `fmt` rewrites
# the sources in place, so nothing else here would notice a crate that was left unformatted.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

Invoke-Script fmt-check
Invoke-Script test
Invoke-Script build
Invoke-Script clippy
Invoke-Script doc
Invoke-Script integration-test
Invoke-Script dotnet-check
