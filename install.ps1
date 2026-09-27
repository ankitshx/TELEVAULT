# TELEVAULT 1-Click Automated Windows Installer & Quick Launcher
# Usage: irm https://raw.githubusercontent.com/ankitshx/TELEVAULT/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host " 🚀 TELEVAULT Windows 1-Click Setup & Launcher" -ForegroundColor Cyan
Write-Host "    Private, append-only, resilient cloud drive via MTProto" -ForegroundColor DarkCyan
Write-Host "    Created with ❤️ by Ankit Sharma (@ankitshx)" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Cyan

$CurrentScriptDir = $PSScriptRoot
if (-not $CurrentScriptDir) {
    $CurrentScriptDir = (Get-Location).Path
}

$InstallDir = "$env:LOCALAPPDATA\Programs\TELEVAULT"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# -----------------------------------------------------------------------------
# 1. Locate or Download TELEVAULT Installer / Executable
# -----------------------------------------------------------------------------
Write-Host "`n[1/3] Resolving application package..." -ForegroundColor Yellow

$LocalCandidates = @(
    (Join-Path $CurrentScriptDir "release\TELEVAULT-Setup.exe"),
    (Join-Path $CurrentScriptDir "TELEVAULT-Setup.exe"),
    (Join-Path $CurrentScriptDir "dist\TELEVAULT.exe"),
    (Join-Path $CurrentScriptDir "TELEVAULT.exe")
)

$LocalInstaller = $null
foreach ($cand in $LocalCandidates) {
    if (Test-Path $cand) {
        $LocalInstaller = $cand
        break
    }
}

$TargetSetupExe = "$InstallDir\TELEVAULT-Setup.exe"
$TargetStandaloneExe = "$InstallDir\TELEVAULT.exe"

if ($LocalInstaller) {
    Write-Host "      ✓ Found local package: $(Split-Path -Leaf $LocalInstaller)" -ForegroundColor Green
    if ($LocalInstaller -like "*Setup.exe") {
        Copy-Item -Path $LocalInstaller -Destination $TargetSetupExe -Force
        $ExecTarget = $TargetSetupExe
    } else {
        Copy-Item -Path $LocalInstaller -Destination $TargetStandaloneExe -Force
        $ExecTarget = $TargetStandaloneExe
    }
} else {
    Write-Host "      ⬇ Downloading latest TELEVAULT-Setup.exe from GitHub..." -ForegroundColor Cyan
    $DownloadUrl = "https://github.com/ankitshx/TELEVAULT/releases/latest/download/TELEVAULT-Setup.exe"
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $TargetSetupExe -UseBasicParsing
        $ExecTarget = $TargetSetupExe
        Write-Host "      ✓ Download complete ($([math]::Round((Get-Item $TargetSetupExe).Length / 1MB, 2)) MB)" -ForegroundColor Green
    } catch {
        Write-Host "      [!] Setup download notice: GitHub release may be initializing. Trying standalone fallback..." -ForegroundColor Gray
        $FallbackUrl = "https://github.com/ankitshx/TELEVAULT/releases/latest/download/TELEVAULT.exe"
        try {
            Invoke-WebRequest -Uri $FallbackUrl -OutFile $TargetStandaloneExe -UseBasicParsing
            $ExecTarget = $TargetStandaloneExe
            Write-Host "      ✓ Downloaded standalone executable ($([math]::Round((Get-Item $TargetStandaloneExe).Length / 1MB, 2)) MB)" -ForegroundColor Green
        } catch {
            throw "Failed to download TELEVAULT from GitHub Releases. Check your internet connection or releases page: https://github.com/ankitshx/TELEVAULT/releases"
        }
    }
}

# -----------------------------------------------------------------------------
# 2. Windows Defender Unblock (Eliminates spinning wait cursor)
# -----------------------------------------------------------------------------
Write-Host "`n[2/3] Unblocking executable with Windows Defender..." -ForegroundColor Yellow
if (Get-Command Unblock-File -ErrorAction SilentlyContinue) {
    Get-ChildItem -Path $InstallDir -Filter "*.exe" | ForEach-Object {
        Unblock-File -Path $_.FullName -ErrorAction SilentlyContinue
    }
    Write-Host "      ✓ Executables unblocked for instant startup." -ForegroundColor Green
}

# -----------------------------------------------------------------------------
# 3. Install & Launch Application
# -----------------------------------------------------------------------------
Write-Host "`n[3/3] Launching TELEVAULT..." -ForegroundColor Yellow

if ($ExecTarget -like "*Setup.exe") {
    Write-Host "      Running installer wizard..." -ForegroundColor Cyan
    Start-Process -FilePath $ExecTarget
} else {
    # Standalone mode: ensure Desktop shortcut exists
    try {
        $WshShell = New-Object -ComObject WScript.Shell
        $DesktopPath = [Environment]::GetFolderPath("Desktop")
        $Shortcut = $WshShell.CreateShortcut("$DesktopPath\TELEVAULT.lnk")
        $Shortcut.TargetPath = $TargetStandaloneExe
        $Shortcut.WorkingDirectory = $InstallDir
        $Shortcut.IconLocation = "$TargetStandaloneExe,0"
        $Shortcut.Description = "TELEVAULT: High-Performance Personal Cloud Drive"
        $Shortcut.Save()
        Write-Host "      ✓ Desktop shortcut created." -ForegroundColor Green
    } catch {
        # ignore shortcut creation error
    }
    Start-Process -FilePath $TargetStandaloneExe
}

Write-Host "`n================================================================" -ForegroundColor Green
Write-Host " ✅ TELEVAULT Ready! Enjoy your private Telegram Cloud Drive." -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
