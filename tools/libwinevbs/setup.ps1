# Builds libwinevbs - Wine's VBScript engine as a library (LGPL-2.1-or-later), which runs existing
# tables' scripts - from source, with our patches, using MSYS2's UCRT64 toolchain
# (tools\setup\install-dev-tools.ps1 installs it).
#
#   1. Clones libwinevbs at the commit we follow into vpx-build\libwinevbs (git-ignored), on local
#      branch vpi, and applies the patches in .\patches.
#   2. Builds winevbs64.dll with CMake inside MSYS2's UCRT64 shell, from the folder's space-free
#      short path (as libwinevbs's own automated build does).
#   3. Copies MSYS2's libwinpthread-1.dll beside it: the DLL needs it.
#
# The player finds the result there (engine\player\src\script\winevbs.rs). Safe to re-run: an
# existing copy is rebuilt, not cloned again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [string]$Msys = 'C:\msys64',
    [int]$Jobs = [Environment]::ProcessorCount
)

$ErrorActionPreference = 'Stop'
# below normal for this and everything it starts: gentle on the machine
(Get-Process -Id $PID).PriorityClass = 'BelowNormal'

$commit = 'e8f15a759c345f8ff535bacf2fe6045cc59f5f8c' # atl: return typeinfo from GetClassInfo (0.10.2, Wine 11.20's code)
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$src = Join-Path $BuildRoot 'libwinevbs'
$bash = Join-Path $Msys 'usr\bin\bash.exe'
if (-not (Test-Path $bash)) { throw "MSYS2 not found at ${Msys}; run tools\setup\install-dev-tools.ps1 first" }

New-Item -ItemType Directory -Force $BuildRoot | Out-Null
if (-not (Test-Path $src)) {
    Write-Host "Cloning libwinevbs at $commit ..."
    git clone -q https://github.com/vpinball/libwinevbs.git $src
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

# Bison and the build tools mishandle spaces in paths: build from the folder's 8.3 short name,
# written the MSYS way (C:\MYFOLD~1\... -> /c/MYFOLD~1/...).
$short = (New-Object -ComObject Scripting.FileSystemObject).GetFolder($src).ShortPath
$msysPath = '/' + $short.Substring(0, 1).ToLower() + ($short.Substring(2) -replace '\\', '/')
# with tools\cmake\trim-paths.cmake, so the library records its source files' paths from its own
# folder down, not the folder it was built in (CMake takes the path's spaces)
$trim = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\cmake\trim-paths.cmake')).Replace('\', '/')
Write-Host "Building in $msysPath ..."
$env:MSYSTEM = 'UCRT64'
& $bash -lc "cd $msysPath && cmake -DCMAKE_BUILD_TYPE=Release -DPLATFORM=win-mingw -DARCH=x64 '-DCMAKE_PROJECT_INCLUDE=$trim' -B build && cmake --build build -- -j$Jobs"
if ($LASTEXITCODE -ne 0) { throw 'build failed' }

Copy-Item (Join-Path $Msys 'ucrt64\bin\libwinpthread-1.dll') (Join-Path $src 'build') -Force
Write-Host "Done: $src\build\winevbs64.dll (with libwinpthread-1.dll beside it)"
