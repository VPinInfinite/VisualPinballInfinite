# Builds libarchive (archive.dll; BSD-2-Clause, by Tim Kientzle and its contributors) from source at
# the release we follow, for the table library's RAR archives (engine\library\src\rar.rs). Its RAR and RAR 5 readers are written from the format by Andres Mejia and Grzegorz
# Antoniak (BSD-2-Clause), with Igor Pavlov's public-domain PPMd: none of RARLAB's own code, whose
# licence isn't free.
#
#   1. Clones libarchive at that release into vpx-build\libarchive (git-ignored).
#   2. Builds the library alone with CMake and Visual Studio, everything that needs another library
#      switched off: its RAR readers need none (zip and 7-Zip archives are read by Rust crates).
#
# The table library finds archive.dll there, else beside itself. Safe to re-run: an existing copy is
# rebuilt, not cloned again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [string]$Generator = 'Visual Studio 18 2026'
)

$ErrorActionPreference = 'Stop'

$tag = 'v3.8.10'
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$src = Join-Path $BuildRoot 'libarchive'

New-Item -ItemType Directory -Force $BuildRoot | Out-Null
if (-not (Test-Path $src)) {
    Write-Host "Cloning libarchive $tag ..."
    git -c advice.detachedHead=false clone -q --depth 1 --branch $tag https://github.com/libarchive/libarchive.git $src
    if ($LASTEXITCODE -ne 0) { throw 'clone failed' }
} else {
    Write-Host "Source already at $src, skipping clone."
}

$off = @('TEST', 'TAR', 'CPIO', 'CAT', 'UNZIP', 'OPENSSL', 'MBEDTLS', 'NETTLE', 'CNG', 'LIBB2', 'LZ4', 'LZO', 'LZMA', 'ZSTD', 'ZLIB', 'BZip2',
    'LIBXML2', 'EXPAT', 'WIN32_XMLLITE', 'PCREPOSIX', 'PCRE2POSIX', 'LIBGCC', 'ACL', 'XATTR', 'ICONV') | ForEach-Object { "-DENABLE_$_=OFF" }
Push-Location $src
try {
    # no regular expressions either: only its command-line tools use them. Built in vpi-build: its
    # own build folder holds its CMake modules.
    cmake -G $Generator -A x64 -B vpi-build -DBUILD_SHARED_LIBS=ON -DPOSIX_REGEX_LIB=NONE @off
    if ($LASTEXITCODE -ne 0) { throw 'configure failed' }
    # low priority, 8 jobs: gentle on the machine
    cmd /c 'start "" /b /wait /low cmake --build vpi-build --config Release --target archive -- /m:8 /v:minimal'
    if ($LASTEXITCODE -ne 0) { throw 'build failed' }
} finally {
    Pop-Location
}
$dll = Get-ChildItem (Join-Path $src 'vpi-build') -Recurse -Filter 'archive.dll' | Select-Object -First 1
if (-not $dll) { throw 'archive.dll not found after the build' }
Write-Host "Done: $($dll.FullName)"
