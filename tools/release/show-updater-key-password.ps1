# Copies the updater key's password to the clipboard (make-updater-key.ps1 made it and keeps it
# readable by this Windows account alone), for pasting into a password manager. It isn't printed.

param([string]$Folder = (Join-Path $env:USERPROFILE 'VPI updater key'))

$ErrorActionPreference = 'Stop'
$file = Join-Path $Folder 'vpi-updater.key.password'
if (-not (Test-Path $file)) { throw "No key password at $file" }
$secure = Get-Content $file | ConvertTo-SecureString
$plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
Set-Clipboard -Value $plain
$plain = $null
Write-Host 'The password is on the clipboard: paste it into your password manager, then copy something else over it.'
