# Packs a release: builds the app's own program and Tauri's NSIS installer around it, with everything
# stage.ps1 gathered installed beside the app; installing for the current user, no administrator
# rights needed; Microsoft's WebView2 fetched where Windows lacks it; the updater's signature made
# with our key, and the updater's latest.json written for the release's page. Then scans the app's
# program as stage.ps1 scanned the rest, and copies everything into dist\release\<version>.
#
#   powershell -ExecutionPolicy Bypass -File tools\release\installer.ps1 [-Jobs 4]
#
# The updater's key: the private key and its password, kept outside the repository
# (make-updater-key.ps1); the password read for this build alone, never shown.

param(
    [int]$Jobs = 4,
    [string]$KeyFolder = (Join-Path $env:USERPROFILE 'VPI updater key'),
    [string]$Words = (Join-Path $env:USERPROFILE 'VPI release\scan-words.txt')
)

$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).PriorityClass = 'BelowNormal'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$version = (Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$targetDir = Join-Path $repo 'target\release-build'
$release = Join-Path $repo "dist\release\$version"
$files = Join-Path $release 'files'
if (-not (Test-Path (Join-Path $files 'vpi-player.exe'))) { throw "stage.ps1 hasn't gathered $files yet" }

# the app's screens
Push-Location (Join-Path $repo 'app\ui')
try {
    cmd /c 'start "" /b /wait /low npm.cmd run build'
    if ($LASTEXITCODE -ne 0) { throw 'building the screens failed' }
} finally {
    Pop-Location
}

# the release's settings over the app's own: the installer on, the gathered folder beside the app
$conf = [ordered]@{
    bundle = [ordered]@{
        active = $true
        targets = @('nsis')
        resources = @{ (($files -replace '\\', '/') + '/') = '' }
        publisher = 'Visual Pinball Infinite'
        copyright = 'The Visual Pinball Infinite contributors'
        shortDescription = 'Plays Visual Pinball X tables and keeps your library'
        homepage = 'https://visualpinballinfinite.org'
        windows = [ordered]@{
            webviewInstallMode = [ordered]@{ type = 'downloadBootstrapper'; silent = $true }
            nsis = [ordered]@{ installMode = 'currentUser'; installerIcon = 'icons/icon.ico' }
        }
    }
}
# JSON written as UTF-8 without the byte-order mark PowerShell 5.1's UTF8 puts first, which JSON
# readers such as the updater's refuse
function Write-Json($value, $path, $depth) {
    [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth $depth), (New-Object Text.UTF8Encoding $false))
}
$confFile = Join-Path $release 'tauri.release.conf.json'
Write-Json $conf $confFile 8

# rustc's flags and the release build's switch as stage.ps1 sets them; the updater's key for this
# build alone
$env:CARGO_TARGET_DIR = $targetDir
$env:CARGO_ENCODED_RUSTFLAGS = @("--remap-path-prefix=$env:USERPROFILE=/home", "--remap-path-prefix=$repo=/build", '-Clink-arg=/PDBALTPATH:%_PDB%', '-Ctarget-feature=+crt-static') -join [char]0x1F
$env:VPI_RELEASE_BUILD = '1'
$env:CARGO_BUILD_JOBS = "$Jobs"
$key = Join-Path $KeyFolder 'vpi-updater.key'
if (-not (Test-Path $key)) { throw "the updater's key isn't at $key" }
$env:TAURI_SIGNING_PRIVATE_KEY = $key
$secure = Get-Content (Join-Path $KeyFolder 'vpi-updater.key.password') | ConvertTo-SecureString
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
$secure = $null
Push-Location (Join-Path $repo 'app')
try {
    # called straight, not through cmd's start, whose quoting splits a folder name at its space; it
    # runs at this script's below-normal priority, which programs started from it inherit
    & (Join-Path $repo 'app\ui\node_modules\.bin\tauri.cmd') build --ci --config $confFile
    if ($LASTEXITCODE -ne 0) { throw 'building the installer failed' }
} finally {
    Pop-Location
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD, Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
}

# the installer, its signature, the app's program, and the updater's latest.json beside them
$nsis = Join-Path $targetDir 'release\bundle\nsis'
$setup = Get-ChildItem $nsis -Filter "*_${version}_x64-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "no installer in $nsis" }
$name = "VisualPinballInfinite-$version-setup.exe"
Copy-Item $setup.FullName (Join-Path $release $name) -Force
Copy-Item "$($setup.FullName).sig" (Join-Path $release "$name.sig") -Force
$app = Join-Path $release 'app'
New-Item -ItemType Directory -Force $app | Out-Null
Copy-Item (Join-Path $targetDir 'release\vpi-app.exe') $app -Force
$latest = [ordered]@{
    version = $version
    notes = ''
    pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = (Get-Content "$($setup.FullName).sig" -Raw).Trim()
            url = "https://github.com/VPinInfinite/VisualPinballInfinite/releases/download/v$version/$name"
        }
    }
}
Write-Json $latest (Join-Path $release 'latest.json') 5

# the app's program scanned as the rest were
& python -I (Join-Path $PSScriptRoot 'scan.py') $app --words $Words
if ($LASTEXITCODE -ne 0) { throw 'the scan found what mustn''t ship in the app (above)' }
Write-Host "Done: $(Join-Path $release $name) ($([math]::Round((Get-Item (Join-Path $release $name)).Length / 1MB, 1)) MB)"
