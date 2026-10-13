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

# The renderer the icon set is drawn with. It is librsvg's, and it is a build-time tool rather than a
# runtime one: what it draws is committed, so a build needs neither it nor the set it draws from.
$Rsvg = 'rsvg-convert'
if ($env:RSVG) { $Rsvg = $env:RSVG }

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

# The command line programs that bring no completion scripts of their own: `rola-jj` answers with
# jj's command line, which is parsed by clap rather than by mingling, so the scripts
# `gen_program!()` writes are not its to have, and `rola-edit` parses its one position and one flag
# with arg-picker on its own. Both are built and laid out beside the others all the same — `rola jj`
# and `rola edit` reach them where they are — and the two lists are told apart only by what an export
# copies for them.
$ProgramsWithoutCompletions = @('rola-jj', 'rola-edit')

# The .NET side of the workspace as one solution: the Desktop program, the translations it reads,
# and the tests of those translations.
$Solution = 'RorolalaSharp.sln'

# Where an export puts the completion scripts: one directory per program, so what a shell's setup
# sources is named once per program rather than once per program and shell.
$ScriptsDir = "$BuildDir/scripts"

# The Desktop program: the project it is built from, and where an export lays it out. It is laid out
# in a `desktop` directory of its own beside the command line programs, which is where
# `rola desktop` reaches for it — so an export and that command name one place.
$DesktopProject = 'app/desktop/Core/RorolalaDesktop.csproj'
$DesktopDir = "$BuildDir/bin/desktop"

# The platform the Desktop program is laid out for, as .NET spells one.
#
# A publish that names no platform carries the native libraries of every platform NuGet holds them for
# — sixteen of them and half a gigabyte, of which one is ever loaded — so the layout names the one it is
# for. It is the machine's own, asked of the SDK rather than worked out here, because the SDK is what
# knows the machine: `win-x64` on Windows, `linux-musl-arm64` on the Alpine that spells it so.
#
# Nothing carries the .NET runtime either way: the program names the framework rather than including
# it, so what a platform decides is which native libraries travel with it and not whether a runtime
# does. Override it to lay out another platform, the way the toolchains above are overridden.
$DotnetRid = ''
if ($env:DOTNET_RID) {
    $DotnetRid = $env:DOTNET_RID
} else {
    $said = & $DotnetProgram @DotnetFlags --info 2>$null
    $rid = $said | Select-String -Pattern '^\s*RID:\s*(\S+)' | Select-Object -First 1
    if ($rid) { $DotnetRid = $rid.Matches[0].Groups[1].Value }
}

# The plugins that ship with the Desktop program, taken from the tree rather than listed anywhere.
#
# A plugin is a project under `Plugins/`, one directory to one plugin, and the name of the directory is
# also the assembly's name — which is what says which files to take from the output it was built into.
# The program finds its plugins by looking at what is beside it, so the tree is the one place a plugin
# is declared: a directory added there is one these scripts build and lay down, and none of them has to
# be told about it.
#
# The source tree spells the directory `Plugins` and the program spells the directory it scans
# `plugins`; the two are deliberately not made to match. What a program looks in is its own business,
# and the tree's spelling is the tree's.
function Get-DesktopPlugins {
    Get-ChildItem -Path 'app/desktop/Plugins' -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        # A project is named after the directory it sits in, so anything else in there is not one.
        $project = Join-Path $_.FullName "$($_.Name).csproj"

        if (Test-Path $project) {
            $project
        }
    }
}

# Restores and builds the whole solution once for everything the .NET side of an export is made of.
#
# A restore is per solution rather than per project, and a project restored on its own is the same
# packages resolved over again — which one export pays once per project, four times over, for nothing.
# And what the projects are is the solution's own list, so nothing here names them: the host, the
# plugins and the C# bindings are all projects the solution already has, and a plugin added to the tree
# is one it already names.
function Build-Solution {
    & $DotnetProgram @DotnetFlags restore $Solution
    Assert-Exit

    & $DotnetProgram @DotnetFlags build $Solution -c Release --no-restore
    Assert-Exit
}

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

    # Four levels, not three: `$PSScriptRoot` inside a function defined here is this file's own
    # directory — `dev/run/src/lib` — while in the body of a script that dot-sources it, it is that
    # script's directory. The shell side needs no such count, since `RUN=./run.sh` is resolved
    # against the working directory the runner has already set to the repository root.
    & "$PSScriptRoot/../../../../run.ps1" $Name
    Assert-Exit
}

# Publishes the Desktop program into the directory named. `dotnet publish` is the form that produces
# a program with everything beside it that running it needs, which is what an export holds; a plain
# build leaves a program whose dependencies are still only in the build tree.
#
# One platform rather than all of them: published for a RID, what is laid down beside the program is
# that platform's native libraries and no other's — the drawing and text-shaping libraries a window
# needs, which NuGet otherwise carries for every platform it has ever been built for.
function Publish-Desktop {
    param([Parameter(Mandatory = $true, Position = 0)][string] $Into)

    if (-not $DotnetRid) {
        Write-Error "the host's RID could not be read; name the platform this layout is for in DOTNET_RID"
        exit 1
    }

    # Restored for the RID before it is published: the restore `Build-Solution` made answers for the
    # platform the build is made on, and a publish that names another one finds an assets file without
    # a target for it.
    & $DotnetProgram @DotnetFlags restore $DesktopProject -r $DotnetRid
    Assert-Exit

    & $DotnetProgram @DotnetFlags publish $DesktopProject -c Release --no-restore -r $DotnetRid --self-contained false -o $Into
    Assert-Exit

    # The native library the Desktop's own bindings reach for, beside the program that binds it: a
    # plugin asks the C ABI through `RolaSharp`, which `DllImport`s `rorolala`, and the runtime looks
    # for that in the program's own directory before anywhere else.
    Copy-Item -Force "$ReleaseDir/$CargoShared" "$Into/"
}

# Builds each plugin that ships with the Desktop program and lays it where the program looks for it:
# its assembly, its translations and any private dependency under `plugins/` beside the program. The
# contract and Avalonia the plugin was built against are the host's own copies, delegated to rather
# than carried, so laying them down would be laying down a second of each — but a dependency the host
# does not carry is the plugin's own and travels with it.
# One MSBuild over the solution is what builds the host and the plugins; what is left here is laying
# each plugin where the program looks for it, which is files: its assembly, its translations and any
# private dependency under `plugins/` beside the program.
function Publish-Plugins {
    foreach ($project in Get-DesktopPlugins) {
        $name = [System.IO.Path]::GetFileNameWithoutExtension($project)
        $output = "$CsTargetDir/$name/bin/Release/net8.0"

        New-Item -ItemType Directory -Force -Path "$DesktopDir/plugins" | Out-Null
        Copy-Item -Force "$output/$name.dll" "$DesktopDir/plugins/"

        if (Test-Path "$output/i18n") {
            Copy-Item -Recurse -Force "$output/i18n" "$DesktopDir/plugins/"
        }

        if (Test-Path "$output/RolaSharp.dll") {
            Copy-Item -Force "$output/RolaSharp.dll" "$DesktopDir/plugins/"
        }
    }
}

# The C# bindings to the C ABI: a module of its own rather than part of the Desktop program, because
# what reaches the ABI is not the desktop's concern. An export hands the assembly over beside the
# native library it binds, since a consumer of the two takes them together.
$RsharpProject = 'modules/rola-sharp/RolaSharp.csproj'
$RsharpDll = "$CsTargetDir/RolaSharp/bin/Release/net8.0/RolaSharp.dll"

# Lays the C# bindings down, beside the native library they bind, under `lib/` where a consumer of the
# C ABI looks for both. Nothing is built here: it is a project of the solution, and built rather than
# published even there — it is a library with nothing beside it.
function Publish-Rsharp {
    Copy-Item -Force $RsharpDll "$BuildDir/lib/RolaSharp.dll"
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
