# Lays an export out and packages it, which is what a person is handed: what an export is made of is
# programs, and what someone who is not a developer wants is something that puts them where a shell and a
# file manager will find them.
#
# Two packages, because the two platforms do not agree on what one is. Windows is given an installer — a
# program that copies the layout, writes the two things that make a program reachable (the profile and the
# path) and registers the one that makes it convenient (the context menu), and that can be asked to undo all
# of it. Unix is given an archive and the script that installs it, since what a Unix hands over is a tree and
# a script and the packaging of them is the distribution's business rather than this project's.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

$Version = Get-WorkspaceVersion
$PackageDir = $BuildDir

# The installer's own file name, which the script that builds it is told rather than naming out of its own
# head: what a package is called is decided here, so that it and an archive read alike.
$Setup = "rorolala-$Version-setup"

Invoke-Script export

# The programs of a Unix install are the ones an archive holds, and what tells them where to go is the script
# beside them; a Windows install is a program, so it is built here.
if ($env:OS -eq 'Windows_NT') {
    Install-Inno

    # The script is told the four things it cannot work out for itself: what version this is, what the
    # installer is to be called, and where the layout it packages was laid and is to be written.
    $iscc = "$InnoDir/ISCC.exe"
    $script = (Resolve-Path 'dev/packaging/windows/rorolala.iss').Path
    $source = (Resolve-Path $BuildDir).Path
    $out = (Resolve-Path $PackageDir).Path

    & $iscc /Q "/DVersion=$Version" "/DSetup=$Setup" "/DSourceDir=$source" "/DOutputDir=$out" $script
    Assert-Exit

    Get-Item "$PackageDir/$Setup.exe" | Select-Object FullName, Length
}
else {
    # The layout and the script that installs it, as one archive: what a Unix is given is the tree as it was
    # laid out, so nothing here changes it beyond adding the script a reader runs. What the archive is named
    # for is the platform it is for, which is where this is running.
    $Archive = "$PackageDir/rorolala-$Version-$((& uname -s).ToLower())-$(& uname -m).tar.gz"

    Copy-Item -Force 'dev/packaging/unix/install.sh' "$BuildDir/install.sh"

    tar -czf $Archive -C $BuildDir .
    Assert-Exit

    Get-Item $Archive | Select-Object FullName, Length
}
