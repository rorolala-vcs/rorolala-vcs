#Requires -Version 5.1
# Lays the build out for hand-off under `$BuildDir`, under the names a consumer links, includes and
# sources. That is also what puts the Desktop program where `rola desktop` reaches for it, so an
# export is what makes that command work.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

# The plugin's own pictures first, and deliberately: they are drawn from the pinned icon set rather
# than carried, so what is laid out below is what the pin holds rather than whatever was last in the
# tree. It is the one step here that reaches outside the tree for its input.
Invoke-Script icons

Invoke-Script build

if (Test-Path $BuildDir) { Remove-Item -Recurse -Force $BuildDir }
New-Item -ItemType Directory -Force -Path "$BuildDir/lib", "$BuildDir/bin" | Out-Null

Copy-Item -Force "$ReleaseDir/$CargoShared" "$BuildDir/lib/rorolala.$SharedSuffix"
Copy-Item -Force "$ReleaseDir/$CargoStatic" "$BuildDir/lib/rorolala.$StaticSuffix"

# Only Windows has an import library, and only a DLL is handed over beside one.
if ($CargoImport) {
    Copy-Item -Force "$ReleaseDir/$CargoImport" "$BuildDir/lib/rorolala.$ImportSuffix"
}

# The header is C and C++ at once, so it is handed over under both names.
Copy-Item -Force $Header "$BuildDir/lib/rorolala.h"
Copy-Item -Force $Header "$BuildDir/lib/rorolala.hpp"

foreach ($program in $Programs) {
    Copy-Item -Force "$ReleaseDir/$program$ExeSuffix" "$BuildDir/bin/$program$ExeSuffix"

    New-Item -ItemType Directory -Force -Path "$ScriptsDir/$program" | Out-Null
    foreach ($shell in $CompletionShells) {
        Copy-Item -Force "$CompletionDir/${program}_comp.$shell" "$ScriptsDir/$program/$program-completion.$shell"
    }
}

Publish-Desktop $DesktopDir
Publish-Plugins
