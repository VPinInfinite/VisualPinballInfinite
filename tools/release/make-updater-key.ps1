# Makes the key the one-click updater's releases are signed with: Tauri's updater installs a new version only when its signature
# matches the public key built into the app, so only someone holding the private key can ship one.
#
# The key goes in "<your user folder>\VPI updater key", never in the repository: the private key
# (vpi-updater.key), locked with a password made here at random; that password, kept so only this
# Windows account on this PC can read it (vpi-updater.key.password, Windows' DPAPI); the public key
# (vpi-updater.key.pub), which goes into app\tauri.conf.json; and a README saying what to back up.
# The password is never shown: show-updater-key-password.ps1 puts it on the clipboard for a password
# manager. Run it once; it never overwrites a key that's there (a lost key means existing installs
# can't be updated any more).

param([string]$Folder = (Join-Path $env:USERPROFILE 'VPI updater key'))

$ErrorActionPreference = 'Stop'
$key = Join-Path $Folder 'vpi-updater.key'
if (Test-Path $key) { throw "A key is there already: $key. It's never overwritten." }
New-Item -ItemType Directory -Force $Folder | Out-Null

# 32 random bytes as letters and digits: the key's password
$bytes = New-Object byte[] 32
[Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
$password = ([Convert]::ToBase64String($bytes) -replace '[^A-Za-z0-9]', '')

Push-Location (Join-Path $PSScriptRoot '..\..\app\ui')
try {
    npx tauri signer generate --ci -p $password -w $key | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'making the key failed' }
} finally {
    Pop-Location
}
if (-not (Test-Path $key) -or -not (Test-Path "$key.pub")) { throw 'the key files weren''t written' }

# the password, readable by this Windows account on this PC alone
ConvertTo-SecureString $password -AsPlainText -Force | ConvertFrom-SecureString | Set-Content -Encoding ascii "$key.password"
$password = $null

@"
Visual Pinball Infinite's updater key
=====================================

These files sign each new version of Visual Pinball Infinite, so the app's one-click updater
installs only versions that really came from us.

  vpi-updater.key           the private key. Keep it secret, and back it up.
  vpi-updater.key.password  its password, readable only by this Windows account on this PC.
  vpi-updater.key.pub       the public key. Not secret: it's built into the app.

Back up now, in two places (a password manager and a USB stick, say):
  - vpi-updater.key itself, and
  - its password: run tools\release\show-updater-key-password.ps1 in the project, which copies it
    to the clipboard, and paste it into your password manager.

If the key or its password is lost, versions already installed can't be updated any more: people
would have to download the next version by hand. If someone else gets both, they could sign a
version of their own, so never share them, and never put them in the project or on GitHub.
"@ | Set-Content -Encoding utf8 (Join-Path $Folder 'README.txt')

Write-Host "Made: $Folder"
Write-Host "Public key (for app\tauri.conf.json):"
Get-Content "$key.pub"
