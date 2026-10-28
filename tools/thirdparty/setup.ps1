# Builds the libraries the installer ships that other projects make, from source, at the versions
# VPX build 5436 uses, into vpx-build\thirdparty (git-ignored):
#
#   - FFmpeg (LGPL-2.1-or-later), for PinUP packs' and Media Player's sound and videos and the
#     preview videos (engine\player\src\pup\av.rs), as VPX 5436 configures it;
#   - libserum and libvni (GPL-2.0), for vpi-colour's colour files (engine\colour-host);
#   - libusb, libserialport, hidapi and libftdi, which libdof and libdmdutil reach a cabinet's
#     boards and panels through, as libdof's own script builds them;
#   - libzedmd (GPL-3.0), with the sockpp (BSD-3-Clause) and libframeutil (GPL-3.0) it uses, and
#     libpupdmd (GPL-3.0), which libdmdutil drives real DMD panels with, and cargs (MIT) for its
#     own tools, as libdmdutil's and libzedmd's own scripts build them;
#   - the MinGW runtime and the small libraries FFmpeg loads (zlib, bzip2, xz, libiconv).
#
# Everything ends up in vpx-build\thirdparty\out\windows-x64: bin (the libraries), lib (their
# import libraries), include, and each one's licence as LICENSE-<name>.txt. The MinGW-built ones
# come from build-mingw.sh in MSYS2's UCRT64 shell (MSYS2 at C:\msys64, its build tools added with
# its own package manager when missing); the rest are built with Visual Studio, as their own
# scripts build them. It all runs at below-normal priority, $Jobs at a time. Safe to re-run: what's
# built is only collected again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [string]$Generator = 'Visual Studio 18 2026',
    [string]$Msys = 'C:\msys64',
    [int]$Jobs = 8
)

$ErrorActionPreference = 'Stop'
# below normal for this and everything it starts: gentle on the machine
(Get-Process -Id $PID).PriorityClass = 'BelowNormal'

$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$root = Join-Path $BuildRoot 'thirdparty'
$src = Join-Path $root 'src'
$out = Join-Path $root 'out\windows-x64'
New-Item -ItemType Directory -Force $src, (Join-Path $out 'bin'), (Join-Path $out 'lib'), (Join-Path $out 'include') | Out-Null
$bash = Join-Path $Msys 'usr\bin\bash.exe'
if (-not (Test-Path $bash)) { throw "MSYS2 isn't at $Msys" }

# MSYS2's build tools and the libraries FFmpeg finds, as VPX's build had them
$packages = 'make diffutils mingw-w64-ucrt-x86_64-autotools mingw-w64-ucrt-x86_64-nasm mingw-w64-ucrt-x86_64-zlib mingw-w64-ucrt-x86_64-bzip2 mingw-w64-ucrt-x86_64-xz mingw-w64-ucrt-x86_64-libiconv'
Write-Host 'MSYS2: the build tools ...'
& $bash -lc "pacman -S --needed --noconfirm $packages"
if ($LASTEXITCODE -ne 0) { throw 'installing MSYS2 packages failed' }

# the MinGW-built libraries, through paths without spaces (Windows' short names), which make and
# libtool need
$env:MSYSTEM = 'UCRT64'
$fso = New-Object -ComObject Scripting.FileSystemObject
function Unix($path) { $p = $path; '/' + $p.Substring(0, 1).ToLower() + $p.Substring(2).Replace('\', '/') }
$unixRoot = Unix $fso.GetFolder($root).ShortPath
$unixScript = Unix $fso.GetFile((Join-Path $PSScriptRoot 'build-mingw.sh')).ShortPath
& $bash -l $unixScript $unixRoot
if ($LASTEXITCODE -ne 0) { throw 'the MinGW builds failed' }

# a pinned source, unpacked once into its own folder
function Fetch($name, $url) {
    $dir = Join-Path $src $name
    if (-not (Test-Path $dir)) {
        Write-Host "Fetching $name ..."
        $tgz = Join-Path $src "$name.tar.gz"
        curl.exe -sSL $url -o $tgz
        if ($LASTEXITCODE -ne 0) { throw "fetching $name failed" }
        $tmp = Join-Path $src "$name.tmp"
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force $tmp | Out-Null
        tar -xzf $tgz -C $tmp
        if ($LASTEXITCODE -ne 0) { throw "unpacking $name failed" }
        Move-Item (Get-ChildItem $tmp -Directory | Select-Object -First 1).FullName $dir
        Remove-Item -Force $tmp
    }
    $dir
}

# a CMake project built with Visual Studio: configured with `$options` (by default as libdmdutil's
# script configures PPUC's libraries: shared only), one target or all; with tools\cmake\trim-paths.cmake,
# so the library records its source files' paths from its own folder down and its debug file by
# name, not the folders it was built in
$trim = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\cmake\trim-paths.cmake'))
function Build-Msvc($dir, [string[]]$options = @('-DPLATFORM=win', '-DARCH=x64', '-DBUILD_SHARED=ON', '-DBUILD_STATIC=OFF'), [string]$target = '') {
    Push-Location $dir
    try {
        cmake -G $Generator @options "-DCMAKE_PROJECT_INCLUDE=$trim" -B build
        if ($LASTEXITCODE -ne 0) { throw "configuring $dir failed" }
        $only = if ($target) { @('--target', $target) } else { @() }
        cmake --build build --config Release @only -- "/m:$Jobs" /v:minimal
        if ($LASTEXITCODE -ne 0) { throw "building $dir failed" }
    } finally {
        Pop-Location
    }
}

# a line of a source's own build file changed as its maker's script changes it (the library's
# file named for 64-bit Windows, as what loads it expects); once
function Edit-Once($file, $from, $to) {
    $text = [IO.File]::ReadAllText($file)
    if ($text.Contains($to)) { return }
    if (-not $text.Contains($from)) { throw "$file has no '$from'" }
    [IO.File]::WriteAllText($file, $text.Replace($from, $to))
}

# libserum (GPL-2.0), libdmdutil's pinned commit (VPX 5436's LIBDMDUTIL_SHA's platforms\config.sh)
$serum = Fetch 'libserum' 'https://github.com/PPUC/libserum/archive/5046e20351e7d4359e976b1edcdce34937c9289f.tar.gz'
if (-not (Test-Path (Join-Path $serum 'build\Release\serum64.dll'))) { Build-Msvc $serum }
Copy-Item (Join-Path $serum 'build\Release\serum64.dll') (Join-Path $out 'bin')
Copy-Item (Join-Path $serum 'build\Release\serum64.lib') (Join-Path $out 'lib')
Copy-Item (Join-Path $serum 'src\serum.h'), (Join-Path $serum 'src\serum-decode.h') (Join-Path $out 'include')

# libvni (GPL-2.0), the same, with the libframeutil headers its own script fetches
$vni = Fetch 'libvni' 'https://github.com/PPUC/libvni/archive/cef652e8e543ced5ec9af7663014b240a2a99ebd.tar.gz'
$frameutil = Fetch 'libframeutil' 'https://github.com/ppuc/libframeutil/archive/28f2bae0dabcbd5c599e6f62211f009e078c1f96.tar.gz'
New-Item -ItemType Directory -Force (Join-Path $vni 'third-party\include') | Out-Null
Copy-Item (Join-Path $frameutil 'include\*') (Join-Path $vni 'third-party\include') -Recurse -Force
if (-not (Test-Path (Join-Path $vni 'build\Release\vni64.dll'))) { Build-Msvc $vni }
Copy-Item (Join-Path $vni 'build\Release\vni64.dll') (Join-Path $out 'bin')
Copy-Item (Join-Path $vni 'build\Release\vni64.lib') (Join-Path $out 'lib')
Get-ChildItem $vni -Recurse -Filter 'vni.h' | Select-Object -First 1 | Copy-Item -Destination (Join-Path $out 'include')

# cargs (MIT) and sockpp (BSD-3-Clause), at the versions libzedmd pins (its platforms\config.sh),
# their files named cargs64 and sockpp64 as its script names them
$cargs = Fetch 'cargs' 'https://github.com/likle/cargs/archive/0698c3f90333446d0fc2745c1e9ce10dd4a9497a.tar.gz'
Edit-Once (Join-Path $cargs 'CMakeLists.txt') 'set_target_properties(cargs PROPERTIES DEFINE_SYMBOL CAG_EXPORTS)' "set_target_properties(cargs PROPERTIES DEFINE_SYMBOL CAG_EXPORTS)`nset_target_properties(cargs PROPERTIES OUTPUT_NAME cargs64)"
if (-not (Test-Path (Join-Path $cargs 'build\Release\cargs64.dll'))) { Build-Msvc $cargs @('-DBUILD_SHARED_LIBS=ON') }
$sockpp = Fetch 'sockpp' 'https://github.com/fpagliughi/sockpp/archive/e6c4688a576d95f42dd7628cefe68092f6c5cd0f.tar.gz'
Edit-Once (Join-Path $sockpp 'CMakeLists.txt') 'set(SOCKPP_SHARED_LIBRARY sockpp)' 'set(SOCKPP_SHARED_LIBRARY sockpp64)'
if (-not (Test-Path (Join-Path $sockpp 'build\Release\sockpp64.dll'))) { Build-Msvc $sockpp @() }

# libzedmd (GPL-3.0) at libdmdutil's pinned commit, its third-party folder filled as its own script
# fills it: libserialport's (built above), sockpp's, cargs' and libframeutil's at its own pin
$zedmd = Fetch 'libzedmd' 'https://github.com/PPUC/libzedmd/archive/e8466d252a71539a50bc8ac6271a00fd2d48884c.tar.gz'
$zframeutil = Fetch 'libframeutil-zedmd' 'https://github.com/ppuc/libframeutil/archive/6bd462a3fcae77ee481c8b1cc863bd54cc40456f.tar.gz'
$zinclude = Join-Path $zedmd 'third-party\include'
$zlibs = Join-Path $zedmd 'third-party\build-libs\win\x64'
$zruntime = Join-Path $zedmd 'third-party\runtime-libs\win\x64'
New-Item -ItemType Directory -Force $zinclude, $zlibs, $zruntime | Out-Null
Copy-Item (Join-Path $out 'include\libserialport.h'), (Join-Path $cargs 'include\cargs.h') $zinclude
Copy-Item (Join-Path $zframeutil 'include\*') $zinclude -Recurse -Force
Copy-Item (Join-Path $sockpp 'include\sockpp') $zinclude -Recurse -Force
Copy-Item (Join-Path $out 'lib\libserialport64.lib'), (Join-Path $sockpp 'build\Release\sockpp64.lib'), (Join-Path $cargs 'build\Release\cargs64.lib') $zlibs
Copy-Item (Join-Path $out 'bin\libserialport64-0.dll'), (Join-Path $sockpp 'build\Release\sockpp64.dll'), (Join-Path $cargs 'build\Release\cargs64.dll') $zruntime
if (-not (Test-Path (Join-Path $zedmd 'build\Release\zedmd64.dll'))) {
    Build-Msvc $zedmd @('-DPLATFORM=win', '-DARCH=x64', '-DBUILD_SHARED=ON', '-DBUILD_STATIC=OFF', '-DPOST_BUILD_COPY_EXT_LIBS=OFF') 'zedmd_shared'
}

# libpupdmd (GPL-3.0) at libdmdutil's pinned commit
$pupdmd = Fetch 'libpupdmd' 'https://github.com/PPUC/libpupdmd/archive/4a1123220e6dce73c87cc584494df2ac82cb6f4c.tar.gz'
if (-not (Test-Path (Join-Path $pupdmd 'build\Release\pupdmd64.dll'))) { Build-Msvc $pupdmd -target 'pupdmd_shared' }

foreach ($l in @(@($cargs, 'cargs64'), @($sockpp, 'sockpp64'), @($zedmd, 'zedmd64'), @($pupdmd, 'pupdmd64'))) {
    Copy-Item (Join-Path $l[0] "build\Release\$($l[1]).dll") (Join-Path $out 'bin')
    Copy-Item (Join-Path $l[0] "build\Release\$($l[1]).lib") (Join-Path $out 'lib')
}
Copy-Item (Join-Path $cargs 'include\cargs.h'), (Join-Path $zedmd 'src\ZeDMD.h'), (Join-Path $pupdmd 'src\pupdmd.h') (Join-Path $out 'include')
Copy-Item (Join-Path $sockpp 'include\sockpp') (Join-Path $out 'include') -Recurse -Force
Copy-Item (Join-Path $zframeutil 'include\*') (Join-Path $out 'include') -Recurse -Force

# the MinGW-built libraries without their debugging information, which only a debugger reads
# (FFmpeg's avcodec carries some 80 MB of it); the Visual Studio ones keep theirs in separate files
$objdump = Join-Path $Msys 'ucrt64\bin\objdump.exe'
$strip = Join-Path $Msys 'ucrt64\bin\strip.exe'
foreach ($dll in Get-ChildItem (Join-Path $out 'bin') -Filter '*.dll') {
    if (& $objdump -h $dll.FullName 2>$null | Select-String '\.debug_') {
        & $strip --strip-debug $dll.FullName
        if ($LASTEXITCODE -ne 0) { throw "stripping $($dll.Name) failed" }
    }
}

# each one's licence beside them, as the installer's notices need: LICENSE-<name>.txt, and
# LICENSE-<name>-<file>.txt for each more
$licences = [ordered]@{
    'libserum'      = @('LICENSE*')
    'libvni'        = @('LICENSE*')
    'libframeutil'  = @('LICENSE*')
    'cargs'         = @('LICENSE*')
    'sockpp'        = @('LICENSE*')
    'libzedmd'      = @('LICENSE*')
    'libpupdmd'     = @('LICENSE*')
    'libusb'        = @('COPYING*')
    'libserialport' = @('COPYING*')
    'hidapi'        = @('LICENSE*.txt')
    'libftdi'       = @('COPYING.LIB')
    'ffmpeg'        = @('LICENSE.md', 'COPYING.LGPLv2.1')
}
Get-ChildItem $out -Filter 'LICENSE-*.txt' | Remove-Item
function Put-Licences($name, $files) {
    $n = 0
    foreach ($f in $files) {
        $suffix = if ($n -eq 0) { '' } else { "-$($f.BaseName)" }
        Copy-Item $f.FullName (Join-Path $out "LICENSE-$name$suffix.txt")
        $n++
    }
}
foreach ($name in $licences.Keys) {
    $dir = Join-Path $src $name
    if (-not (Test-Path $dir)) { continue }
    Put-Licences $name @($licences[$name] | ForEach-Object { Get-ChildItem $dir -Filter $_ -File })
}
# the MinGW runtime's and the small libraries' licences, as MSYS2's packages carry them
foreach ($package in 'gcc', 'libgcc', 'libwinpthread', 'winpthreads', 'zlib', 'bzip2', 'xz', 'libiconv') {
    $dir = Join-Path $Msys "ucrt64\share\licenses\$package"
    if (Test-Path $dir) { Put-Licences "mingw-$package" @(Get-ChildItem $dir -File -Recurse) }
}
Write-Host "Done: $out"
Get-ChildItem (Join-Path $out 'bin') | Select-Object Name, Length | Format-Table -AutoSize
