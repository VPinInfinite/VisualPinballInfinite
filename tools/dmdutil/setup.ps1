# Builds libdmdutil (dmdutil64.dll) - the VPX team's library for real DMD panels, GPL-3.0-or-later -
# from source at the commit VPX build 5436 uses, with our patch, for vpi-dmd (engine\dmd-host, which
# loads it). The patch makes its Serum part optional, as its VNI part already is, and this builds it
# with both off: libserum and libvni ship the GPL's version 2 text alone, so they never share a
# program with our GPL-3.0-or-later code (they run in vpi-colour).
#
#   1. Clones libdmdutil at that commit into vpx-build\libdmdutil (git-ignored), on local branch vpi,
#      and applies the patches in .\patches.
#   2. Fetches the headers it compiles against, from the versions it pins (platforms\config.sh, and
#      libzedmd's): ZeDMD, libframeutil, libserialport, sockpp, libusb and libpupdmd, each into a
#      folder of its own under vpx-build\libdmdutil-deps; only their headers are used.
#   3. Takes those libraries' builds from ours, made from source at the same pinned versions by
#      tools\thirdparty\setup.ps1 (vpx-build\thirdparty\out); else from VPX 5436's bundle, which built
#      them with libdmdutil's own script (vpx-build\downloads), saying which.
#   4. Builds the shared library only (ENABLE_SERUM and ENABLE_VNI off) with CMake and Visual Studio,
#      at low priority, $Jobs at a time, and puts the libraries it loads beside it.
#
# vpi-dmd finds the result there (engine\dmd-host). Safe to re-run: an existing copy is rebuilt,
# not cloned again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [string]$Generator = 'Visual Studio 18 2026',
    [int]$Jobs = 8
)

$ErrorActionPreference = 'Stop'

$commit = '3485e2e0e1e9252148914cd613510ccad7bb56b5' # VPX 5436's LIBDMDUTIL_SHA (platforms\config.sh)
# the versions libdmdutil pins (its platforms\config.sh, and libzedmd's for the three it brings)
$deps = [ordered]@{
    'zedmd'      = 'https://github.com/PPUC/libzedmd/archive/e8466d252a71539a50bc8ac6271a00fd2d48884c.tar.gz'
    'usb'        = 'https://github.com/libusb/libusb/archive/15a7ebb4d426c5ce196684347d2b7cafad862626.tar.gz'
    'pupdmd'     = 'https://github.com/PPUC/libpupdmd/archive/4a1123220e6dce73c87cc584494df2ac82cb6f4c.tar.gz'
    'serialport' = 'https://github.com/sigrokproject/libserialport/archive/21b3dfe5f68c205be4086469335fd2fc2ce11ed2.tar.gz'
    'sockpp'     = 'https://github.com/fpagliughi/sockpp/archive/e6c4688a576d95f42dd7628cefe68092f6c5cd0f.tar.gz'
    'frameutil'  = 'https://github.com/ppuc/libframeutil/archive/6bd462a3fcae77ee481c8b1cc863bd54cc40456f.tar.gz'
}
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$src = Join-Path $BuildRoot 'libdmdutil'
$depRoot = Join-Path $BuildRoot 'libdmdutil-deps'
$bundle = Join-Path $BuildRoot 'downloads\5436-third-party-windows-x64-Release'

New-Item -ItemType Directory -Force $BuildRoot | Out-Null
if (-not (Test-Path $src)) {
    Write-Host "Cloning libdmdutil at $commit ..."
    git clone -q --no-checkout https://github.com/vpinball/libdmdutil.git $src
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

# the pinned sources, each unpacked in a folder of its own (only their headers are read)
foreach ($name in $deps.Keys) {
    $dir = Join-Path $depRoot $name
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Force $dir | Out-Null
        Write-Host "Fetching $name ..."
        curl.exe -sSL $deps[$name] -o (Join-Path $dir 'src.tar.gz')
        if ($LASTEXITCODE -ne 0) { throw "fetching $name failed" }
        # through cmd, its complaints left there: libusb's archive holds links Windows' tar can't make
        # (its README among them); only headers are used, and copying them below fails if one is missing
        cmd /c "tar -xzf `"$(Join-Path $dir 'src.tar.gz')`" -C `"$dir`" 2>nul"
    }
}
function Source($name) { (Get-ChildItem (Join-Path $depRoot $name) -Directory | Select-Object -First 1).FullName }

$third = Join-Path $src 'third-party'
$include = Join-Path $third 'include'
$buildLibs = Join-Path $third 'build-libs\win\x64'
$runtimeLibs = Join-Path $third 'runtime-libs\win\x64'
New-Item -ItemType Directory -Force (Join-Path $include 'libusb-1.0'), $buildLibs, $runtimeLibs | Out-Null
Copy-Item (Join-Path (Source 'zedmd') 'src\ZeDMD.h') $include
Copy-Item (Join-Path (Source 'frameutil') 'include\*') $include
Copy-Item (Join-Path (Source 'serialport') 'libserialport.h') $include
Copy-Item (Join-Path (Source 'pupdmd') 'src\pupdmd.h') $include
Copy-Item -Recurse -Force (Join-Path (Source 'sockpp') 'include\sockpp') $include
Copy-Item (Join-Path (Source 'usb') 'libusb\libusb.h') (Join-Path $include 'libusb-1.0')

# the libraries themselves: our own builds, else VPX 5436's run of libdmdutil's script
$ours = Join-Path $BuildRoot 'thirdparty\out\windows-x64'
function From($folder, $bundleFolder, $file) {
    $mine = Join-Path $ours "$folder\$file"
    if (Test-Path $mine) { return $mine }
    $theirs = Join-Path $bundle "$bundleFolder\windows-x64\$file"
    if (-not (Test-Path $theirs)) { throw "$file is neither in $ours nor in VPX 5436's bundle" }
    Write-Host "$file from VPX 5436's bundle"
    $theirs
}
foreach ($l in 'cargs64', 'zedmd64', 'libusb64-1.0', 'libserialport64', 'sockpp64', 'pupdmd64') {
    Copy-Item (From 'lib' 'build-libs' "$l.lib") $buildLibs
}
$runtime = 'cargs64', 'zedmd64', 'libusb64-1.0', 'libserialport64-0', 'sockpp64', 'pupdmd64', 'libgcc_s_seh-1', 'libstdc++-6', 'libwinpthread-1'
foreach ($l in $runtime) {
    Copy-Item (From 'bin' 'runtime-libs' "$l.dll") $runtimeLibs
}

Push-Location $src
try {
    cmake -G $Generator -DPLATFORM=win -DARCH=x64 -DBUILD_SHARED=ON -DBUILD_STATIC=OFF -DPOST_BUILD_COPY_EXT_LIBS=OFF -DENABLE_SERUM=OFF -DENABLE_VNI=OFF -B build
    if ($LASTEXITCODE -ne 0) { throw 'configure failed' }
    # low priority, $Jobs at a time: gentle on the machine
    cmd /c "start `"`" /b /wait /low cmake --build build --config Release --target dmdutil_shared -- /m:$Jobs /v:minimal"
    if ($LASTEXITCODE -ne 0) { throw 'build failed' }
} finally {
    Pop-Location
}
# the libraries it loads, beside it (cargs is only for libdmdutil's own tools)
foreach ($l in $runtime | Where-Object { $_ -ne 'cargs64' }) {
    Copy-Item (Join-Path $runtimeLibs "$l.dll") (Join-Path $src 'build\Release')
}
Write-Host "Done: $src\build\Release\dmdutil64.dll (no Serum, no VNI)"
