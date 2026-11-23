# Builds rom2altsound (BSD-3-Clause, by Sylvain Gargasson) from source at the release we follow, for
# the app's AltSound packs made from a table's own ROM. It plays every sound command of a ROM's sound
# board through PinMAME, built into it from its author's fork (Le-Syl21/pinmame, branch
# bsmt2000-lle; PinMAME's terms: BSD-3-Clause for new code, the old MAME terms for the rest), and
# writes each sound out as a ready AltSound pack. It runs as a program of its own, as PinMAME runs
# beside the player, so PinMAME's older terms stay outside the engine.
#
#   1. Clones rom2altsound at that release, with its PinMAME, into vpx-build\rom2altsound (git-ignored).
#   2. Builds its command line alone (not its window program) with Cargo, CMake and Visual Studio:
#      the crates its lock file pins, and the user's folder and the repository's taken out of what's
#      compiled in, as tools\release\stage.ps1 builds our own. Microsoft's C runtime is the DLL its
#      PinMAME is built against, which the installer carries beside the app.
#
# The app finds rom2altsound.exe there, else beside itself. Safe to re-run: an existing copy is
# rebuilt, not cloned again.

param(
    [string]$BuildRoot = (Join-Path $PSScriptRoot '..\..\vpx-build'),
    [int]$Jobs = 8
)

$ErrorActionPreference = 'Stop'

$tag = 'v0.4.0'
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$src = Join-Path $BuildRoot 'rom2altsound'

New-Item -ItemType Directory -Force $BuildRoot | Out-Null
if (-not (Test-Path $src)) {
    Write-Host "Cloning rom2altsound $tag and its PinMAME ..."
    git -c advice.detachedHead=false clone -q --depth 1 --branch $tag https://github.com/Le-Syl21/rom2altsound.git $src
    if ($LASTEXITCODE -ne 0) { throw 'clone failed' }
    # its PinMAME at the commit the release names
    git -C $src submodule update -q --init --depth 1
    if ($LASTEXITCODE -ne 0) { throw 'its PinMAME could not be cloned' }
} else {
    Write-Host "Source already at $src, skipping clone."
}

# rustc's flags one per entry (paths with spaces in them): the folders remapped, the debug file
# named without its folder
$flags = @("--remap-path-prefix=$env:USERPROFILE=/home", "--remap-path-prefix=$repo=/build", '-Clink-arg=/PDBALTPATH:%_PDB%')
$env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1F
# its PinMAME's source paths trimmed to rom2altsound's own folder down (trim-paths.cmake), through
# the toolchain file the cmake crate hands CMake
$env:CMAKE_TOOLCHAIN_FILE = Join-Path $PSScriptRoot 'trim-paths-toolchain.cmake'
$env:VPI_TRIM_ROOT = $src
Push-Location $src
try {
    # every crate its lock file names, its window program's too, so the release's notices can list
    # the command line's crates without the network (tools\release\notices.py)
    cargo fetch --locked
    if ($LASTEXITCODE -ne 0) { throw 'fetching its crates failed' }
    # PinMAME configured afresh: CMake reads a toolchain file only into a new build folder
    cargo clean --release -p rom2altsound
    # low priority: gentle on the machine
    cmd /c "start `"`" /b /wait /low cargo build --release --locked -p rom2altsound -j $Jobs"
    if ($LASTEXITCODE -ne 0) { throw 'build failed' }
} finally {
    Pop-Location
    Remove-Item Env:CARGO_ENCODED_RUSTFLAGS, Env:CMAKE_TOOLCHAIN_FILE, Env:VPI_TRIM_ROOT
}
$exe = Join-Path $src 'target\release\rom2altsound.exe'
if (-not (Test-Path $exe)) { throw 'rom2altsound.exe not found after the build' }
Write-Host "Done: $exe"
