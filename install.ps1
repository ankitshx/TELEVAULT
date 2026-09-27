# TeleCloud 1-Click Automated Windows Installer
# Usage: irm https://raw.githubusercontent.com/ankitshx/TELEVAULT/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

Write-Host "================================================" -ForegroundColor Cyan
Write-Host " 🚀 Installing TeleCloud Desktop Application..." -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan

$InstallDir = "$env:LOCALAPPDATA\Programs\TeleCloud"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$ExePath = "$InstallDir\telecloud-desktop.exe"
$DownloadUrl = "https://raw.githubusercontent.com/ankitshx/TELEVAULT/main/telecloud-desktop.exe"

Write-Host "[1/3] Downloading latest binary (27 MB)..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $DownloadUrl -OutFile $ExePath -UseBasicParsing

Write-Host "[2/3] Whitelisting binary with Windows Defender..." -ForegroundColor Yellow
if (Get-Command Unblock-File -ErrorAction SilentlyContinue) {
    Unblock-File -Path $ExePath
}

Write-Host "[3/3] Creating Desktop Shortcut..." -ForegroundColor Yellow
$WshShell = New-Object -ComObject WScript.Shell
$DesktopPath = [Environment]::GetFolderPath("Desktop")
$Shortcut = $WshShell.CreateShortcut("$DesktopPath\TeleCloud.lnk")
$Shortcut.TargetPath = $ExePath
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "TeleCloud: High-Performance Personal Cloud Drive"
$Shortcut.Save()

Write-Host ""
Write-Host "✅ Installation Complete! Launching TeleCloud..." -ForegroundColor Green
Start-Process -FilePath $ExePath
