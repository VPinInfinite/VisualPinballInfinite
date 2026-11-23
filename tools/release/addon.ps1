# The achievements add-on on its own: a zip anyone can drop into VPX 10.8.1's add-on folder.
#   1. Built for release as stage.ps1 builds the programs: in target\release-build, this PC's user
#      folder and the repository remapped out of what's compiled in (rustc's --remap-path-prefix),
#      the debug file named without its folder, the C runtime built in; VPI_RELEASE_BUILD set.
#   2. Gathered into dist\addon\<version>\vpi-achievements: the library under the name its
#      plugin.cfg gives it, the plugin.cfg, the README (tools\release\addon\README.txt), the
#      licence and the notices for what's built into it (notices.py --addon).
#   3. Scanned as a release's files are (security_scan.py, with the private word list where it's
#      kept), then zipped beside the folder. Nothing is published.
#
#   powershell -ExecutionPolicy Bypass -File tools\release\addon.ps1 [-Jobs 4]
param([int]$Jobs = 8)
$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).PriorityClass = 'BelowNormal'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$version = (Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$targetDir = Join-Path $repo 'target\release-build'
$out = Join-Path $repo "dist\addon\$version"
$folder = Join-Path $out 'vpi-achievements'

# 1. the add-on, built for release
$env:CARGO_TARGET_DIR = $targetDir
$flags = @("--remap-path-prefix=$env:USERPROFILE=/home", "--remap-path-prefix=$repo=/build", '-Clink-arg=/PDBALTPATH:%_PDB%', '-Ctarget-feature=+crt-static')
$env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1F
$env:VPI_RELEASE_BUILD = '1'
Push-Location $repo
try {
    cmd /c "start `"`" /b /wait /low cargo build --release --locked -p vpx-addon -j $Jobs"
    if ($LASTEXITCODE -ne 0) { throw "the add-on's release build failed" }
} finally {
    Pop-Location
}

# 2. its folder, as VPX loads it
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $folder | Out-Null
Copy-Item (Join-Path $targetDir 'release\vpi_vpx_addon.dll') (Join-Path $folder 'plugin-vpi-achievements64.dll')
# its plugin.cfg, as the app writes it (library::vpx_addon::cfg)
$cfg = "[configuration]`r`nid = `"VPIAchievements`"`r`nname = `"Achievements`"`r`ndescription = `"A table's achievements followed as it plays, for Visual Pinball Infinite`"`r`nauthor = `"Visual Pinball Infinite`"`r`nversion = `"$version`"`r`n`r`n[libraries]`r`nwindows.x64 = `"plugin-vpi-achievements64.dll`"`r`n"
[IO.File]::WriteAllText((Join-Path $folder 'plugin.cfg'), $cfg)
Copy-Item (Join-Path $PSScriptRoot 'addon\README.txt') (Join-Path $folder 'README.txt')
Copy-Item (Join-Path $repo 'LICENSE') (Join-Path $folder 'LICENSE.txt')
python -I (Join-Path $PSScriptRoot 'notices.py') --addon $folder
if ($LASTEXITCODE -ne 0) { throw "the add-on's notices weren't written (above)" }

# 3. scanned as a release's files are, then zipped
$scanArgs = @((Join-Path $PSScriptRoot 'security_scan.py'), (Join-Path $out 'scan.txt'), $folder)
$words = Join-Path $env:USERPROFILE 'VPI release\scan-words.txt'
if (Test-Path $words) { $scanArgs += @('--words', $words) }
$key = Join-Path $env:USERPROFILE 'VPI updater key'
if (Test-Path $key) { $scanArgs += @('--key-folder', $key) }
python -I @scanArgs
if ($LASTEXITCODE -ne 0) { throw "the scan found what mustn't go out: $(Join-Path $out 'scan.txt')" }
$zip = Join-Path $out "vpi-achievements-$version.zip"
Compress-Archive -Path $folder -DestinationPath $zip -Force
Write-Host "The add-on on its own: $zip"
