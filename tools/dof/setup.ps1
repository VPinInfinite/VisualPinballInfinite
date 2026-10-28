# Builds libdof (dof64.dll) - the VPX team's C++ port of the DirectOutput Framework, GPL-3.0-or-later -
# from source at the commit VPX build 5436 uses, for vpi-dof (engine\dof-host, which loads it).
# vpi-dof calls libdof's C++ members by their exported names, so it's built with Visual Studio, as
# VPX builds it.
#
#   1. Clones libdof at that commit into vpx-build\libdof (git-ignored), on local branch vpi, and
#      applies the patches in .\patches, if any.
#   2. Fetches the headers it compiles against, from the versions it pins (platforms\config.sh):
#      libusb, libserialport, hidapi and libftdi, each into a folder of its own under
#      vpx-build\libdof-deps; only their headers are used.
#   3. Takes those libraries' builds from our own, made from source at the same pinned versions by
#      tools\thirdparty\setup.ps1 (vpx-build\thirdparty\out), else from VPX 5436's bundle, which
#      built them with libdof's own script (vpx-build\downloads), saying which. miniaudio's header,
#      for our SSF patch, comes from libaltsound's source at the commit tools\altsound follows: the
#      0.11.25 the VPX team keeps there, with its global lock made static, checked by its checksum.
#   4. Builds the shared library only with CMake and Visual Studio, at low priority, $Jobs at a
#      time, with tools\cmake\trim-paths.cmake, so the library records its source files' paths from
#      its own folder down, and puts the libraries it loads beside it.
#
# vpi-dof finds the result there (engine\dof-host). Safe to re-run: an existing copy is rebuilt,
# not cloned again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [string]$Generator = 'Visual Studio 18 2026',
    [int]$Jobs = 8
)

$ErrorActionPreference = 'Stop'

$commit = 'eef645d9f5df618290962946c9e3e8ed30886639' # VPX 5436's LIBDOF_SHA (platforms\config.sh)
# miniaudio.h as libaltsound keeps it, at the commit tools\altsound\setup.ps1 follows
$miniaudioUrl = 'https://raw.githubusercontent.com/vpinball/libaltsound/ce11780064a75fe23059fd3b61847ee18d477931/third-party/include/miniaudio/miniaudio.h'
$miniaudioSha256 = 'EAB98ED7C8C987EDD05AF219E84174ABAF6CB1A49AC82CC723FD8CDEDB6E8592'
# the versions libdof pins at that commit (its platforms\config.sh)
$deps = [ordered]@{
    'usb'        = 'https://github.com/libusb/libusb/archive/15a7ebb4d426c5ce196684347d2b7cafad862626.tar.gz'
    'serialport' = 'https://github.com/sigrokproject/libserialport/archive/21b3dfe5f68c205be4086469335fd2fc2ce11ed2.tar.gz'
    'hidapi'     = 'https://github.com/libusb/hidapi/archive/d6b2a974608dec3b76fb1e36c189f22b9cf3650c.tar.gz'
    'ftdi'       = 'https://github.com/jsm174/libftdi/archive/5c2c58e03ea999534e8cb64906c8ae8b15536c30.tar.gz'
}
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$src = Join-Path $BuildRoot 'libdof'
$depRoot = Join-Path $BuildRoot 'libdof-deps'
$bundle = Join-Path $BuildRoot 'downloads\5436-third-party-windows-x64-Release'
$trim = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\cmake\trim-paths.cmake'))

New-Item -ItemType Directory -Force $BuildRoot | Out-Null
if (-not (Test-Path $src)) {
    Write-Host "Cloning libdof at $commit ..."
    git clone -q --no-checkout https://github.com/vpinball/libdof.git $src
    if ($LASTEXITCODE -ne 0) { throw 'clone failed' }
    git -C $src -c advice.detachedHead=false checkout -q $commit
    git -C $src switch -q -c vpi
    # a list even when there's one patch (one alone would be a string, passed letter by letter)
    $patchDir = Join-Path $PSScriptRoot 'patches'
    $patches = @(if (Test-Path $patchDir) { Get-ChildItem $patchDir -Filter '*.patch' | Sort-Object Name | ForEach-Object { $_.FullName } })
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

# where libdof's own script puts them (platforms\win\x64\external.sh)
$third = Join-Path $src 'third-party'
$include = Join-Path $third 'include'
$buildLibs = Join-Path $third 'build-libs\win\x64'
$runtimeLibs = Join-Path $third 'runtime-libs\win\x64'
New-Item -ItemType Directory -Force (Join-Path $include 'libusb-1.0'), (Join-Path $include 'libftdi1'), $buildLibs, $runtimeLibs | Out-Null
Copy-Item (Join-Path (Source 'usb') 'libusb\libusb.h') (Join-Path $include 'libusb-1.0')
Copy-Item (Join-Path (Source 'serialport') 'libserialport.h') $include
Copy-Item -Recurse -Force (Join-Path (Source 'hidapi') 'hidapi') $include
Copy-Item (Join-Path (Source 'ftdi') 'src\ftdi.h') (Join-Path $include 'libftdi1')

# the libraries themselves: our own builds, else VPX 5436's run of libdof's script
$ours = Join-Path $BuildRoot 'thirdparty\out\windows-x64'
function From($folder, $bundleFolder, $file) {
    $mine = Join-Path $ours "$folder\$file"
    if (Test-Path $mine) { return $mine }
    $theirs = Join-Path $bundle "$bundleFolder\windows-x64\$file"
    if (-not (Test-Path $theirs)) { throw "$file is neither in $ours nor in VPX 5436's bundle" }
    Write-Host "$file from VPX 5436's bundle"
    $theirs
}
foreach ($l in 'libserialport64', 'hidapi64', 'libftdi164', 'libusb64-1.0') {
    Copy-Item (From 'lib' 'build-libs' "$l.lib") $buildLibs
}
$runtime = 'libserialport64-0', 'hidapi64', 'libftdi164', 'libusb64-1.0', 'libgcc_s_seh-1', 'libstdc++-6', 'libwinpthread-1'
foreach ($l in $runtime) {
    Copy-Item (From 'bin' 'runtime-libs' "$l.dll") $runtimeLibs
}
# miniaudio (public domain or MIT No Attribution), which plays the SSF impact controller's sounds
# in our patch, compiled into libdof itself
$miniaudio = Join-Path $depRoot 'miniaudio\miniaudio.h'
if (-not (Test-Path $miniaudio)) {
    New-Item -ItemType Directory -Force (Split-Path $miniaudio) | Out-Null
    Write-Host 'Fetching miniaudio.h ...'
    curl.exe -sSL $miniaudioUrl -o $miniaudio
    if ($LASTEXITCODE -ne 0) { throw 'fetching miniaudio.h failed' }
}
$sum = (Get-FileHash $miniaudio -Algorithm SHA256).Hash
if ($sum -ne $miniaudioSha256) { Remove-Item $miniaudio; throw "miniaudio.h isn't the one expected (SHA-256 $sum)" }
New-Item -ItemType Directory -Force (Join-Path $include 'miniaudio') | Out-Null
Copy-Item $miniaudio (Join-Path $include 'miniaudio')

Push-Location $src
try {
    cmake -G $Generator -DPLATFORM=win -DARCH=x64 -DBUILD_SHARED=ON -DBUILD_STATIC=OFF -DPOST_BUILD_COPY_EXT_LIBS=OFF "-DCMAKE_PROJECT_INCLUDE=$trim" -B build
    if ($LASTEXITCODE -ne 0) { throw 'configure failed' }
    # low priority, a few jobs: gentle on the machine
    cmd /c ('start "" /b /wait /low cmake --build build --config Release --target dof_shared -- /m:{0} /v:minimal' -f $Jobs)
    if ($LASTEXITCODE -ne 0) { throw 'build failed' }
} finally {
    Pop-Location
}
# the libraries it loads, beside it
foreach ($l in $runtime) {
    Copy-Item (Join-Path $runtimeLibs "$l.dll") (Join-Path $src 'build\Release')
}
Write-Host "Done: $src\build\Release\dof64.dll"
