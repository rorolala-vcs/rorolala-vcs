#Requires -Version 5.1
# Draws the plugin's own pictures from the pinned Material Design icon set.
#
# The set is not vendored: it is a shallow clone under `THIRD-PARTY/`, which git ignores, and every
# picture is drawn from it rather than copied by hand, so what is drawn is what the pinned commit
# holds. What is drawn is committed, so a build needs neither the set nor a renderer — this script is
# what needs them, and `export` runs it so that what ships is drawn from the pin rather than from
# whatever was last in the tree.
#
# The clone is shallow and detached: one commit, no history, and the commit is the one named below.
# A clone left at another commit, or with anything changed in it, is put back, since it is a
# dependency and not a place to work.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

# The commit the pictures are drawn from.
#
# It is the whole of what "the icon set" means here: there is no history to travel, so another set is
# another clone, and another picture is a line in `$Icons` below. The upstream is archived, so this is
# expected to stay where it is.
$IconsCommit = 'af0ed9c0e1276bad43c4d6ca8e8aaa283e425195'
$IconsUrl = 'https://github.com/material-icons/material-icons'

# Where the set is kept, and where the pictures drawn from it live.
$IconsClone = 'THIRD-PARTY/material-icons'
$IconsPictures = 'app/desktop/Plugins/RorolalaVCSPlugin/icons'

# The pictures to draw, by the name the set knows each icon by.
#
# `lock` is what an entry somebody else holds is drawn with, and `create` — the set's pencil — what one
# the reader holds is drawn with, since holding it is what makes it theirs to change. Each is the
# `baseline` family, which is the set's own shape for the icon.
$Icons = @('lock', 'create')

# What a picture is drawn in, and how many pixels wide and tall it is.
#
# The ink is white and the ground transparent, because the Desktop draws a picture as a mask over a
# colour it takes from the theme: white is opaque under either reading of a mask — its alpha, and its
# luminance — so the mark takes the colour the user is looking at rather than one baked into the file.
# The size is four times the set's own 24-unit grid, which is as large as a tile's icon ever gets.
$IconsInk = '#FFFFFF'
$IconsSize = 96

# Runs git in the clone and hands back what it said, saying nothing when it failed.
function Get-InClone {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]] $Arguments)

    & git -C $IconsClone @Arguments 2>$null
}

Write-Host "==> icons at $IconsCommit"

# Puts the clone where this expects it: the named commit, and nothing changed in it.
if (-not (Test-Path "$IconsClone/.git")) {
    Write-Host "==> cloning $IconsUrl"

    if (Test-Path $IconsClone) { Remove-Item -Recurse -Force $IconsClone }
    New-Item -ItemType Directory -Force -Path $IconsClone | Out-Null

    & git -C $IconsClone init -q
    Assert-Exit
    & git -C $IconsClone remote add origin $IconsUrl
    Assert-Exit
}

if ((Get-InClone rev-parse HEAD) -ne $IconsCommit) {
    Write-Host "==> checking out $IconsCommit"

    & git -C $IconsClone fetch -q --depth 1 origin $IconsCommit
    Assert-Exit
    & git -C $IconsClone checkout -q --detach $IconsCommit
    Assert-Exit
}

if (Get-InClone status --porcelain) {
    Write-Host "==> restoring $IconsClone"

    & git -C $IconsClone checkout -q --force $IconsCommit
    Assert-Exit
    & git -C $IconsClone clean -qffd
    Assert-Exit
}

if (-not (Get-Command $Rsvg -ErrorAction SilentlyContinue)) {
    Write-Error "no renderer: ``$Rsvg`` is not on PATH — install librsvg, or name another with RSVG="
}

foreach ($icon in $Icons) {
    $source = "$IconsClone/svg/$icon/baseline.svg"
    $picture = "$IconsPictures/$icon.png"

    if (-not (Test-Path $source)) {
        Write-Error "the icon set holds no ``$icon`` at $IconsCommit"
    }

    # The set draws in black, so the ink is set on the way through rather than left to the renderer,
    # which has no colour of its own to be told. The inked copy is taken away however the renderer
    # ends, so that the next icon starts clean.
    $svg = (Get-Content -Raw $source) -replace '<path ', "<path fill=`"$IconsInk`" "
    $inked = [System.IO.Path]::GetTempFileName()

    try {
        Set-Content -Path $inked -Value $svg -NoNewline
        & $Rsvg -w $IconsSize -h $IconsSize -o $picture $inked
        Assert-Exit
    }
    finally {
        Remove-Item -Force $inked -ErrorAction SilentlyContinue
    }

    Write-Host "==> $picture"
}
