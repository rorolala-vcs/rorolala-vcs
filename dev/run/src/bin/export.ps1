#Requires -Version 5.1
# Builds the tree and lays the result out for hand-off under `$BuildDir`, under the names a consumer
# links, includes and sources. That is also what puts the Desktop program where `rola desktop` reaches
# for it, so an export is what makes that command work.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

# Any running `rola`, `rola-daemon` or `rola-daemon-gui` is stopped before anything is built: a
# process would hold the binaries the build is about to write, and a file that is still in use
# cannot be overwritten. The window holds its own executable the same way, so it is stopped too.
Get-Process -Name 'rola', 'rola-daemon', 'rola-daemon-gui' -ErrorAction SilentlyContinue | ForEach-Object {
    Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}
Get-Process -Name 'rola', 'rola-daemon', 'rola-daemon-gui' -ErrorAction SilentlyContinue | ForEach-Object { $_.WaitForExit(5000) }

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

# Every program is laid down beside the others, so that a word of `rola`'s reaches the program it
# names — `rola jj` is `rola-jj` — and the completions are laid down for the programs that have them.
foreach ($program in $Programs + $ProgramsWithoutCompletions) {
    Copy-Item -Force "$ReleaseDir/$program$ExeSuffix" "$BuildDir/bin/$program$ExeSuffix"
}

foreach ($program in $Programs) {
    New-Item -ItemType Directory -Force -Path "$ScriptsDir/$program" | Out-Null
    foreach ($shell in $CompletionShells) {
        Copy-Item -Force "$CompletionDir/${program}_comp.$shell" "$ScriptsDir/$program/$program-completion.$shell"
    }
}

# The whole solution is restored and built once for everything below rather than a project at a time:
# the restore is shared, and one MSBuild over the solution is what builds the host, the plugins and the
# bindings together.
Build-Solution

Publish-Desktop $DesktopDir
Publish-Launcher $DesktopDir
Publish-Plugins

# The C# bindings beside the native library they bind, under `lib/` where a consumer of the C ABI looks
# for both.
Publish-Rsharp

# And the words that go with it: what the hand-off is, and what it is handed over under. They are the
# tree's own at its top rather than files written for a layout, because a second copy is a copy that
# goes stale — what an export carries is what the project says about itself, said once.
Copy-Item -Force 'README.md', 'LICENSE' "$BuildDir/"

# And the window over the daemon's own answers. It is built last of the programs because
# `$BuildDir` is removed and laid out above — a window built before that would be wiped — and
# before the integrity level below, so the copy that is handed over is the one that carries it.
Invoke-Script daemon-win-build

# The export tree is handed to whoever runs it, so it is given the integrity level that lets it run
# rather than being left at whatever the tree was checked out with.
icacls $BuildDir /setintegritylevel '(OI)(CI)M' /T /C /Q
