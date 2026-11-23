# Visual Pinball Infinite - developer tool setup (Windows)
#
# Installs everything needed to build the project, using winget:
#   Machine-wide (one administrator prompt):
#     GitHub CLI, CMake, Ninja, Node.js LTS, Visual Studio Build Tools (C++),
#     plus C++ ATL, which VPX's own Windows build needs (for physics reference runs)
#   For the current user (no prompt):
#     Rust, via rustup
#
# Usage, from the repository root:
#   powershell -ExecutionPolicy Bypass -File tools\setup\install-dev-tools.ps1
#
# Safe to re-run: winget skips anything already installed.
# Output is logged to tools\setup\install-dev-tools.log.

param([switch]$MachineOnly)

$log = Join-Path $PSScriptRoot 'install-dev-tools.log'
$wingetArgs = @('--exact', '--source', 'winget',
                '--accept-source-agreements', '--accept-package-agreements',
                '--disable-interactivity')

function Install-WingetPackage([string]$Id, [string]$Name, [string[]]$Extra = @('--silent')) {
    Write-Output ''
    Write-Output "=== $Name ($Id) ==="
    $out = & winget install --id $Id @wingetArgs @Extra
    $out | Where-Object { $_ -match '\S' } | Select-Object -Last 2 | ForEach-Object { Write-Output "  $_" }
    Write-Output "  winget exit code: $LASTEXITCODE"
}

if ($MachineOnly) {
    Start-Transcript -Path $log -Append | Out-Null
    Install-WingetPackage 'GitHub.cli'        'GitHub CLI'
    Install-WingetPackage 'Kitware.CMake'     'CMake'
    Install-WingetPackage 'Ninja-build.Ninja' 'Ninja'
    Install-WingetPackage 'OpenJS.NodeJS.LTS' 'Node.js LTS'
    # The Windows C/C++ compiler (MSVC) and Windows SDK. Rust links with these, so they go first.
    Install-WingetPackage 'Microsoft.VisualStudio.BuildTools' 'Visual Studio Build Tools (C++)' @(
        '--override', '--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.ATL --includeRecommended')
    # winget leaves an existing Build Tools install alone, so add ATL through the
    # Visual Studio Installer as well. It does nothing if ATL is already there.
    $installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer'
    $buildTools = & (Join-Path $installer 'vswhere.exe') -products Microsoft.VisualStudio.Product.BuildTools -latest -property installationPath
    if ($buildTools) {
        Write-Output ''
        Write-Output '=== C++ ATL for Visual Studio Build Tools ==='
        $modify = Start-Process (Join-Path $installer 'setup.exe') -Wait -PassThru -ArgumentList `
            "modify --installPath `"$buildTools`" --add Microsoft.VisualStudio.Component.VC.ATL --passive --norestart"
        Write-Output "  installer exit code: $($modify.ExitCode)"
    }
    Stop-Transcript | Out-Null
    exit
}

# Machine-wide tools: elevate once and wait for that half to finish.
Write-Output 'Requesting administrator rights for the machine-wide tools...'
Start-Process powershell.exe -Verb RunAs -Wait -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`" -MachineOnly"

# Rust installs per user, so it runs without elevation.
Start-Transcript -Path $log -Append | Out-Null
Install-WingetPackage 'Rustlang.Rustup' 'Rust (rustup)'
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
if (Get-Command rustup -ErrorAction SilentlyContinue) {
    & rustup default stable
}

# MSYS2 with its UCRT64 GCC, CMake, Bison and Wine tools, to build libwinevbs (tools\libwinevbs)
# and the libraries built with MinGW (tools\thirdparty). Its self-extracting archive needs no
# administrator rights; the download is checked against the checksum MSYS2 publishes.
Write-Output ''
Write-Output '=== MSYS2 (C:\msys64) ==='
$msys = 'C:\msys64'
if (-not (Test-Path "$msys\usr\bin\bash.exe")) {
    $release = '2026-06-11'
    $file = 'msys2-base-x86_64-20260611.sfx.exe'
    $sha256 = 'c105946e64e08f099ac0e4647461ce762b95333ad211777666476a9a41451d65'
    $dir = Join-Path $env:TEMP 'vpi-msys2'
    New-Item -ItemType Directory -Force $dir | Out-Null
    $sfx = Join-Path $dir $file
    & curl.exe -sSL -o $sfx "https://github.com/msys2/msys2-installer/releases/download/$release/$file"
    if ((Get-FileHash $sfx -Algorithm SHA256).Hash -ne $sha256) { throw 'the MSYS2 download does not match its published checksum' }
    Start-Process $sfx -ArgumentList '-y', '-oC:\' -Wait
}
$env:MSYSTEM = 'MSYS'
& "$msys\usr\bin\bash.exe" -lc ' '
# twice: the first pass may update MSYS2's own core and stop
& "$msys\usr\bin\bash.exe" -lc 'pacman --noconfirm -Syuu'
& "$msys\usr\bin\bash.exe" -lc 'pacman --noconfirm -Syuu'
& "$msys\usr\bin\bash.exe" -lc 'pacman -S --noconfirm --needed bison mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-cmake mingw-w64-ucrt-x86_64-tools'
Write-Output "  pacman exit code: $LASTEXITCODE"

Write-Output ''
Write-Output '=== Done ==='
Stop-Transcript | Out-Null
