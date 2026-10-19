#Requires -Version 5.1
# The full gate: the formatting of every crate, the test suites, a build of every crate, clippy, the
# documentation, the integration suites, the Daemon window's C, then the .NET side — the C# linter
# first, then the build and the tests. Composed from the scripts beside this one rather than
# repeating what they do.
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
# The C program is held to warnings as errors and `/analyze` the same way the Rust and C# sides are
# held to theirs. It is the only thing here the other checks do not look at at all.
Invoke-Script daemon-win-check
Invoke-Script dotnet-lint
Invoke-Script dotnet-check
