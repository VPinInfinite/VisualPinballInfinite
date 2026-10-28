# Builds libpinmame (pinmame64.dll) - PinMAME as a library, BSD-3-Clause and the older MAME licence -
# from source at the commit VPX build 5436 uses, with our patches, for the PinMAME host
# (engine\pinmame-host, which loads it). The patches hand out a DMD's identify frame, the raw
# shades PinUP's frame triggers and colour files are matched against, which the legacy callback API
# gives only instead of the brightness frame; and add a switch for PinMAME's samples, older
# machines' recorded sounds, which libpinmame always leaves off (`PinmameSetUseSamples`). Patches 4
# to 7 are PinMAME's own later fixes, by Le-Syl21 (vpinball/pinmame#717, #722, #723): the sound
# commands logged as the sound board takes them on Bally's MPU-17, MPU-35 and 6803 boards, and not
# Whirlwind's solenoid overlay, so an AltSound pack keyed by those commands plays.
#
#   1. Clones PinMAME at that commit into vpx-build\pinmame-5436 (git-ignored), on local branch vpi,
#      and applies the patches in .\patches.
#   2. Builds it with CMake and Visual Studio, as VPX's own build script does
#      (platforms\windows-x64\external.sh): the library's CMakeLists.txt copied to the top, shared
#      library only, at low priority, $Jobs at a time, with tools\cmake\trim-paths.cmake, so the
#      library records its source files' paths from its own folder down, not the folder it was
#      built in.
#
# The host finds the result there (engine\pinmame-host\src\main.rs). Safe to re-run: an existing copy
# is rebuilt, not cloned again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [string]$Generator = 'Visual Studio 18 2026',
    [int]$Jobs = 8
)

$ErrorActionPreference = 'Stop'

$commit = '23321ec7de6dfd563a1c64153bc75f26e5059b9f' # VPX 5436's PINMAME_SHA (platforms\config.sh)
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$src = Join-Path $BuildRoot 'pinmame-5436'
$trim = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\cmake\trim-paths.cmake'))

New-Item -ItemType Directory -Force $BuildRoot | Out-Null
if (-not (Test-Path $src)) {
    Write-Host "Cloning PinMAME at $commit ..."
    git clone -q --filter=blob:none --no-checkout https://github.com/vpinball/pinmame.git $src
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

Copy-Item (Join-Path $src 'cmake\libpinmame\CMakeLists.txt') (Join-Path $src 'CMakeLists.txt') -Force
Push-Location $src
try {
    cmake -G $Generator -DPLATFORM=win -DARCH=x64 -DBUILD_SHARED=ON -DBUILD_STATIC=OFF "-DCMAKE_PROJECT_INCLUDE=$trim" -B build
    if ($LASTEXITCODE -ne 0) { throw 'configure failed' }
    # low priority, a few jobs: gentle on the machine
    cmd /c ('start "" /b /wait /low cmake --build build --config Release --target pinmame_shared -- /m:{0} /v:minimal' -f $Jobs)
    if ($LASTEXITCODE -ne 0) { throw 'build failed' }
} finally {
    Pop-Location
}
Write-Host "Done: $src\build\Release\pinmame64.dll"
