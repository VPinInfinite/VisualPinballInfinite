# Gathers a release: every program built for release, with everything it ships with, into one
# folder - exactly what the installer installs beside the app - and checks it file by file.
#
#   1. Builds the programs for release in a target folder of their own (target\release-build), so
#      everyday builds aren't redone, with this PC's folders taken out of what's compiled in: the
#      user's folder and the repository's are remapped (rustc's --remap-path-prefix) and the debug
#      files named without their folder (/PDBALTPATH); the C runtime built into each program, as
#      .cargo\config.toml has it (flags given here take the place of that file's); and
#      VPI_RELEASE_BUILD set, so the programs look for what they load beside themselves alone, not
#      in the repository's build folders as development builds also do. All at below-normal
#      priority, $Jobs at a time.
#   2. Copies into dist\release\<version>\files what files.json lists: the programs, the libraries
#      they load (built from source by the tools\*\setup.ps1 scripts) and Microsoft's C++ runtime,
#      from Visual Studio's redistributable files; then the licences and notices (notices.py).
#   3. Checks that every program and library there finds each library it loads, there or in Windows.
#   4. Scans every file for what mustn't ship (scan.py): this PC's user name, user folder and name,
#      the repository's folder and the libraries' build folder, and the words in a list kept outside
#      the repository (-Words).
#   5. Writes dist\release\<version>\FILES.txt: each file, what it is, where it's from, its licence.
#
# installer.ps1 builds the app's own program and packs it with this folder.

param(
    [int]$Jobs = 4,
    [switch]$NoBuild,
    [string]$Words = (Join-Path $env:USERPROFILE 'VPI release\scan-words.txt')
)

$ErrorActionPreference = 'Stop'
# below normal for this and everything it starts
(Get-Process -Id $PID).PriorityClass = 'BelowNormal'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$list = Get-Content (Join-Path $PSScriptRoot 'files.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$version = (Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$targetDir = Join-Path $repo 'target\release-build'
$release = Join-Path $repo "dist\release\$version"
$files = Join-Path $release 'files'
Write-Host "Release $version -> $files"

# rustc's flags for a release build, one per 0x1F so folders with spaces stay whole, and the
# release build's switch
function Set-ReleaseFlags {
    $env:CARGO_TARGET_DIR = $targetDir
    $flags = @("--remap-path-prefix=$env:USERPROFILE=/home", "--remap-path-prefix=$repo=/build", '-Clink-arg=/PDBALTPATH:%_PDB%', '-Ctarget-feature=+crt-static')
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1F
    $env:VPI_RELEASE_BUILD = '1'
}

# 1. the programs
if (-not $NoBuild) {
    Set-ReleaseFlags
    $packages = ($list.programs | ForEach-Object { "-p $($_.package)" }) -join ' '
    Push-Location $repo
    try {
        cmd /c "start `"`" /b /wait /low cargo build --release --locked $packages -j $Jobs"
        if ($LASTEXITCODE -ne 0) { throw 'the release build failed' }
    } finally {
        Pop-Location
    }
}

# 2. the folder
Remove-Item -Recurse -Force $files -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $files | Out-Null
foreach ($p in $list.programs) {
    Copy-Item (Join-Path $targetDir "release\$($p.file)") $files
}
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products * -property installationPath
$redist = Get-ChildItem (Join-Path $vs 'VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT') -Directory | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $redist) { throw "Visual Studio's redistributable C++ runtime isn't in $vs" }
foreach ($l in $list.libraries) {
    $from = if ($l.from -eq 'msvc-redist') { $redist.FullName } else { Join-Path $repo $l.from }
    $file = Join-Path $from $l.file
    if (-not (Test-Path $file)) { throw "$($l.file) isn't built: $file" }
    Copy-Item $file $files
}
foreach ($d in $list.folders) {
    $to = Join-Path $files $d.folder
    New-Item -ItemType Directory -Force $to | Out-Null
    $found = Get-ChildItem (Join-Path $repo $d.from) -Filter $d.pattern -File
    if (-not $found) { throw "nothing in $($d.from)" }
    $found | Copy-Item -Destination $to
}
& python -I (Join-Path $PSScriptRoot 'notices.py') --release $release
if ($LASTEXITCODE -ne 0) { throw 'the notices failed' }

# 3. what each program and library loads: there, or a part of Windows (not a runtime Windows
# lacks until something installs it)
$dumpbin = Get-ChildItem (Join-Path $vs 'VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe') | Sort-Object FullName -Descending | Select-Object -First 1
$here = @{}
Get-ChildItem $files -File | ForEach-Object { $here[$_.Name.ToLower()] = $true }
$installedByOthers = '^(msvcp|vcruntime|concrt|vccorlib|msvcr|ucrtbased|d3dx|xinput1_[123]|xaudio2_[0-7])'
$missing = @()
foreach ($f in Get-ChildItem $files -File | Where-Object { $_.Extension -in '.exe', '.dll' }) {
    $loads = & $dumpbin.FullName /nologo /dependents $f.FullName | Where-Object { $_ -match '^\s+(\S+\.dll)\s*$' } | ForEach-Object { $Matches[1].ToLower() }
    foreach ($d in $loads) {
        if ($here[$d] -or $d -like 'api-ms-win-*' -or $d -like 'ext-ms-win-*') { continue }
        if ((Test-Path (Join-Path $env:WINDIR "System32\$d")) -and $d -notmatch $installedByOthers) { continue }
        $missing += "$($f.Name) loads $d"
    }
}
if ($missing) { throw ("missing beside the programs:`n" + ($missing -join "`n")) }
Write-Host 'Every program and library finds what it loads.'

# 4. what mustn't ship
& python -I (Join-Path $PSScriptRoot 'scan.py') $files --words $Words
$scanned = $LASTEXITCODE

# 5. the list, for reading through
# a program's licence: its own, else the workspace's
function Get-Licence($package) {
    $toml = Get-Content (Join-Path $repo "engine\$package\Cargo.toml") -Raw
    if ($toml -match '(?m)^license = "([^"]+)"') { $Matches[1] } else { 'GPL-3.0-or-later' }
}
$lines = @("Visual Pinball Infinite ${version}: the files the installer puts beside the app (vpi-app.exe)", '')
foreach ($p in $list.programs) {
    $size = (Get-Item (Join-Path $files $p.file)).Length
    $lines += "{0,-24} {1,10:N0} bytes  ours ({2}): {3}" -f $p.file, $size, (Get-Licence $p.package), $p.what
}
foreach ($l in $list.libraries) {
    $size = (Get-Item (Join-Path $files $l.file)).Length
    $lines += "{0,-24} {1,10:N0} bytes  {2} ({3}): {4}" -f $l.file, $size, $l.name, $l.licence, $l.what
}
foreach ($d in $list.folders) {
    $count = (Get-ChildItem (Join-Path $files $d.folder) -File).Count
    $lines += "{0,-24} {1,10} files  {2} ({3}): {4}" -f "$($d.folder)\", $count, $d.name, $d.licence, $d.what
}
$lines += ''
$lines += 'And: LICENSE.txt (the GPL, version 3, our programs'' licence), THIRD-PARTY-NOTICES.txt (everything in them made by others, with its licence), licenses\ (each of those licences in full).'
if ($list.held) {
    $lines += ''
    $lines += 'Not in this release yet:'
    foreach ($h in $list.held) { $lines += "  $($h.file): $($h.what) - $($h.why)" }
}
$lines | Set-Content -Encoding UTF8 (Join-Path $release 'FILES.txt')
Write-Host "Written: $(Join-Path $release 'FILES.txt')"
if ($scanned -ne 0) { throw 'the scan found what mustn''t ship (above)' }
