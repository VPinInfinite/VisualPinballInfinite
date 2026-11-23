# Builds libaltsound (altsound64.dll) - PinMAME's alternate sound as a library, BSD-3-Clause - from
# source at the latest commit we follow (newer than the one VPX build 5436 uses), with
# our patches, for the PinMAME host
# (engine\pinmame-host, which loads it). The first makes it read every folder of a PinSound
# pack on Windows: VPX's build reads only each kind's first. The second pairs two bytes per command
# on the older boards without a case of their own, as PinMAME's own AltSound does — the latest reads
# one byte at a time there, and packs made with PinMAME's (Gorgar's) go silent — unless the pack
# keys every sound by one byte, as packs made by rom2altsound do.
#
#   1. Clones libaltsound at that commit into vpx-build\libaltsound (git-ignored), on local branch
#      vpi, and applies the patches in .\patches.
#   2. Builds it with CMake and Visual Studio, as VPX's own build script does
#      (platforms\windows-x64\external.sh): shared library only, at low priority with 8 jobs.
#
# The host finds the result there (engine\pinmame-host\src\main.rs). Safe to re-run: an existing copy
# is rebuilt, not cloned again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [string]$Generator = 'Visual Studio 18 2026'
)

$ErrorActionPreference = 'Stop'

$commit = 'ce11780064a75fe23059fd3b61847ee18d477931' # one byte per command on older boards, System 11's two boards (VPX 5436's: f4b790a)
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$src = Join-Path $BuildRoot 'libaltsound'

New-Item -ItemType Directory -Force $BuildRoot | Out-Null
if (-not (Test-Path $src)) {
    Write-Host "Cloning libaltsound at $commit ..."
    git clone -q --no-checkout https://github.com/vpinball/libaltsound.git $src
    if ($LASTEXITCODE -ne 0) { throw 'clone failed' }
    git -C $src -c advice.detachedHead=false checkout -q $commit
    git -C $src switch -q -c vpi
    # a list even when there's one patch (one alone would be a string, passed letter by letter)
    $patches = @(Get-ChildItem (Join-Path $PSScriptRoot 'patches') -Filter '*.patch' | Sort-Object Name | ForEach-Object { $_.FullName })
    if ($patches.Count -gt 0) {
        Write-Host 'Applying patches ...'
        git -C $src -c user.name=VPinInfinite -c user.email=340424849+VPinInfinite@users.noreply.github.com am -q @patches
        if ($LASTEXITCODE -ne 0) { throw 'applying patches failed (git am --abort to undo)' }
    }
} else {
    Write-Host "Source already at $src, skipping clone and patches."
}

Push-Location $src
try {
    cmake -G $Generator -DPLATFORM=win -DARCH=x64 -DBUILD_SHARED=ON -DBUILD_STATIC=OFF -B build
    if ($LASTEXITCODE -ne 0) { throw 'configure failed' }
    # low priority, 8 jobs: gentle on the machine
    cmd /c 'start "" /b /wait /low cmake --build build --config Release --target altsound_shared -- /m:8 /v:minimal'
    if ($LASTEXITCODE -ne 0) { throw 'build failed' }
} finally {
    Pop-Location
}
Write-Host "Done: $src\build\Release\altsound64.dll"
