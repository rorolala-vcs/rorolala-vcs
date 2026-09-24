#
# What every script here needs: where the toolchains are, where their output goes, and how this
# project spells the artifacts of a platform.
#
# The PowerShell side of `common.sh`, and read beside it: the same names hold the same things, so a
# script here and its `.sh` twin say the same thing in two languages. Only the spelling differs —
# variables are `$PascalCase` and the helpers are `Verb-Noun`, which is what a PowerShell reader
# expects to see.
#
# This file is dot-sourced, never run, and it is not under `.run/src/bin/` — so the runner does not
# offer it as a script. It is what the scripts are made of, not one of them.

# The toolchains, and any flags an override carries. The shell scripts take `CARGO="cargo --offline"`
# as one variable because a shell splits it; nothing splits a string here, so what may be several
# words is split as it is read.
$CargoProgram = 'cargo'
$CargoFlags = @()
if ($env:CARGO) {
    $words = $env:CARGO -split '\s+'
    $CargoProgram = $words[0]
    if ($words.Count -gt 1) { $CargoFlags = $words[1..($words.Count - 1)] }
}

$DotnetProgram = 'dotnet'
$DotnetFlags = @()
if ($env:DOTNET) {
    $words = $env:DOTNET -split '\s+'
    $DotnetProgram = $words[0]
    if ($words.Count -gt 1) { $DotnetFlags = $words[1..($words.Count - 1)] }
}

# Cargo's target directory, matching `.cargo/config.toml`.
$TargetDir = '.cache/rs-target'
if ($env:TARGET_DIR) { $TargetDir = $env:TARGET_DIR }

# .NET's output directory, matching `Directory.Build.props`. It is under the same `.cache` as
# cargo's, so the two toolchains' output sits in one place.
$CsTargetDir = '.cache/cs-target'
if ($env:CS_TARGET_DIR) { $CsTargetDir = $env:CS_TARGET_DIR }

# The directory `export` lays the hand-off artifacts out in.
$BuildDir = 'build'
if ($env:BUILD_DIR) { $BuildDir = $env:BUILD_DIR }

# The only profile anything here builds. Everything it produces is release, so nothing that ships
# can carry debug assertions or debug info.
$ReleaseFlag = '--release'

# Where cargo writes the release artifacts.
$ReleaseDir = "$TargetDir/release"

# The generated C header, written by the root build script while the library builds.
$Header = "$ReleaseDir/ffi_bindings/rorolala_ffi.h"

# Completion scripts. While the programs compile, mingling's `gen_program!()` writes one per
# supported shell into this directory — deliberately not profile-specific, so the same files serve
# every build.
$CompletionDir = "$TargetDir/mingling"
if ($env:COMPLETION_DIR) { $CompletionDir = $env:COMPLETION_DIR }

# Shells mingling generates a completion script for.
$CompletionShells = @('sh', 'zsh', 'fish', 'ps1')

# The command line programs. Each is a workspace member whose package name is also its binary name,
# so one list drives the build, the export, and the completion scripts — mingling names those
# `<program>_comp.<shell>` and they are exported as
# `scripts/<program>/<program>-completion.<shell>`. The scripts call their command by name and never
# refer to their own file name, so the exported name is free.
$Programs = @('rola', 'rola-daemon')

# The .NET side of the workspace as one solution: the Desktop program, the translations it reads,
# and the tests of those translations.
$Solution = 'RorolalaSharp.sln'

# Where an export puts the completion scripts: one directory per program, so what a shell's setup
# sources is named once per program rather than once per program and shell.
$ScriptsDir = "$BuildDir/scripts"

# The Desktop program: the project it is built from, and where an export lays it out. It is laid out
# in a `desktop` directory of its own beside the command line programs, which is where
# `rola desktop` reaches for it — so an export and that command name one place.
$DesktopProject = 'app/desktop/RorolalaDesktop.csproj'
$DesktopDir = "$BuildDir/bin/desktop"

# PowerShell carries on after a native command that failed, which is the opposite of what a gate
# wants: `set -e` stops the shell scripts, and this is what stops a script here. It is called after
# every native command, which is why the calls are written out rather than wrapped in a helper — a
# wrapper would have to take flags like `--release` as arguments of its own, and there is no need to
# make PowerShell guess.
function Assert-Exit {
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

# Asks the runner for another script, the way a `make` target asked for another target and the way
# `again` does in the shell scripts.
function Invoke-Script {
    param([Parameter(Mandatory = $true, Position = 0)][string] $Name)

    & "$PSScriptRoot/../../../run.ps1" $Name
    Assert-Exit
}

# Publishes the Desktop program into the directory named. `dotnet publish` is the form that produces
# a program with everything beside it that running it needs, which is what an export holds; a plain
# build leaves a program whose dependencies are still only in the build tree.
function Publish-Desktop {
    param([Parameter(Mandatory = $true, Position = 0)][string] $Into)

    & $DotnetProgram @DotnetFlags publish $DesktopProject -c Release -o $Into
    Assert-Exit
}

# `export` and `clean` delete a destination with `Remove-Item -Recurse`, so refuse a path that would
# take something else with it. What counts as absolute is different here: a Windows path may begin
# with a drive rather than a separator, and on this platform one that does is absolute.
function Assert-RelativePath {
    param([Parameter(Mandatory = $true, Position = 0)][string] $Name, [Parameter(Position = 1)][string] $Value)

    if ([string]::IsNullOrEmpty($Value)) {
        Write-Error "$Name must not be empty"
    }

    if ($Value -match '^([A-Za-z]:[\\/]|[\\/])') {
        Write-Error "$Name must be a relative path, got ``$Value'"
    }
}

Assert-RelativePath 'BUILD_DIR' $BuildDir
Assert-RelativePath 'TARGET_DIR' $TargetDir
Assert-RelativePath 'CS_TARGET_DIR' $CsTargetDir

# Platform spellings. Cargo names a Unix shared library with a `lib` prefix and a Windows one with a
# `.dll` suffix, so the exported names are normalised to one shape per platform:
#
#   | cargo artifact               | exported as                     |
#   | ---------------------------- | ------------------------------- |
#   | librorolala.so               | $BuildDir/lib/rorolala.so        |
#   | librorolala.dylib            | $BuildDir/lib/rorolala.dylib     |
#   | rorolala.dll                 | $BuildDir/lib/rorolala.dll       |
#   | rorolala.dll.lib             | $BuildDir/lib/rorolala.dll.lib   |
#   | librorolala.a                | $BuildDir/lib/rorolala.a         |
#   | rorolala.lib                 | $BuildDir/lib/rorolala.lib       |
#   | ffi_bindings/rorolala_ffi.h  | $BuildDir/lib/rorolala.h         |
#   |                              | $BuildDir/lib/rorolala.hpp       |
#   | rola[.exe]                   | $BuildDir/bin/rola[.exe]         |
#   | mingling/<program>_comp.*    | $BuildDir/scripts/<program>/*    |
#
# The header is C and C++ at once — its declarations sit in an `extern "C"` block — so `.hpp` is the
# same file under the name a C++ project includes. The completion scripts are the four shells
# mingling generates: sh, zsh, fish and ps1, each program's written into `$ScriptsDir/<program>/` as
# `<program>-completion.<shell>`.
#
# The import library is the one artifact only Windows has: a DLL is linked through it, and the static
# library of the same crate needs nothing beside it, which is why the two `.lib` files are both
# handed over and named apart.
if ($env:OS -eq 'Windows_NT') {
    $ExeSuffix = '.exe'
    $SharedSuffix = 'dll'
    $StaticSuffix = 'lib'
    $CargoShared = "rorolala.$SharedSuffix"
    $CargoStatic = "rorolala.$StaticSuffix"
    # A DLL is linked through the import library that names what it exports, and cargo writes that
    # one under the DLL's own name — `rorolala.dll.lib` — so that it does not take the static
    # library's.
    $CargoImport = "rorolala.$SharedSuffix.$StaticSuffix"
    $ImportSuffix = "$SharedSuffix.$StaticSuffix"
} else {
    $ExeSuffix = ''
    $SharedSuffix = 'so'
    if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::OSX)) {
        $SharedSuffix = 'dylib'
    }
    $StaticSuffix = 'a'
    $CargoShared = "librorolala.$SharedSuffix"
    $CargoStatic = "librorolala.$StaticSuffix"
    # Nothing to hand over: a linker is given the shared library itself.
    $CargoImport = ''
    $ImportSuffix = ''
}
