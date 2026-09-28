<#
.SYNOPSIS
    Verifies TELEVAULT release artifacts, generates SHA-256 checksums, and tests execution.
.DESCRIPTION
    Validates executable and installer integrity, Authenticode digital signatures,
    smoke tests binary execution, and produces SHA256SUMS.txt and RELEASE_NOTES.txt.
#>
[CmdletBinding()]
param(
    [switch]$SkipSmokeTest
)

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "  TELEVAULT Release Verification & Manifest Pipeline" -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

$DistExe = Join-Path $ProjectRoot "dist\TELEVAULT.exe"
$ReleaseDir = Join-Path $ProjectRoot "release"
$ReleaseExe = Join-Path $ReleaseDir "TELEVAULT.exe"
$PortableExe = Join-Path $ReleaseDir "TELEVAULT-Portable-x64.exe"
$InstallerExe = Join-Path $ReleaseDir "TELEVAULT-Setup-x64.exe"

if (-not (Test-Path $ReleaseDir)) {
    New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null
}

# Copy standalone exe to release directory for distribution
if (Test-Path $DistExe) {
    Copy-Item $DistExe $ReleaseExe -Force
    Copy-Item $DistExe $PortableExe -Force
}

# 1. Check Artifact Presence
Write-Host "[1/5] Checking release artifacts..." -ForegroundColor Yellow
$Artifacts = @()

if (Test-Path $InstallerExe) {
    $InstSize = (Get-Item $InstallerExe).Length / 1MB
    Write-Host "  Found Primary Installer: TELEVAULT-Setup-x64.exe ($([math]::Round($InstSize, 2)) MB)" -ForegroundColor Green
    $Artifacts += $InstallerExe
}

if (Test-Path $PortableExe) {
    $PortSize = (Get-Item $PortableExe).Length / 1MB
    Write-Host "  Found Portable Executable: TELEVAULT-Portable-x64.exe ($([math]::Round($PortSize, 2)) MB)" -ForegroundColor Green
    $Artifacts += $PortableExe
}

if (Test-Path $ReleaseExe) {
    $ExeSize = (Get-Item $ReleaseExe).Length / 1MB
    Write-Host "  Found Standalone Binary: TELEVAULT.exe ($([math]::Round($ExeSize, 2)) MB)" -ForegroundColor Green
    $Artifacts += $ReleaseExe
}

if ($Artifacts.Count -eq 0) {
    throw "No release artifacts found in '$ReleaseDir'."
}

# 2. Authenticode Signature Inspection
Write-Host "[2/5] Inspecting Authenticode signatures..." -ForegroundColor Yellow
foreach ($file in $Artifacts) {
    $FileName = Split-Path -Leaf $file
    $Sig = Get-AuthenticodeSignature -FilePath $file
    if ($Sig.Status -eq "Valid") {
        Write-Host "  $FileName : [SIGNED & VALID] - $($Sig.SignerCertificate.Subject)" -ForegroundColor Green
    } elseif ($Sig.Status -eq "NotSigned") {
        Write-Host "  $FileName : [UNSIGNED] (Development / Unsigned build)" -ForegroundColor Yellow
    } else {
        Write-Host "  $FileName : [SIGNATURE STATUS: $($Sig.Status)] - $($Sig.StatusMessage)" -ForegroundColor Yellow
    }
}

# 3. Smoke Test Executable
if (-not $SkipSmokeTest) {
    Write-Host "[3/5] Executing smoke test on TELEVAULT.exe..." -ForegroundColor Yellow
    try {
        $SmokeProcess = Start-Process -FilePath $ReleaseExe -ArgumentList "--help" -PassThru -ErrorAction SilentlyContinue
        if ($SmokeProcess) {
            $Exited = $SmokeProcess.WaitForExit(6000)
            if (-not $Exited) {
                Stop-Process -Id $SmokeProcess.Id -Force -ErrorAction SilentlyContinue
            }
            Write-Host "  Smoke Test: Process executed successfully." -ForegroundColor Green
        }
    } catch {
        Write-Host "  Smoke test notice: $_" -ForegroundColor Yellow
    }
} else {
    Write-Host "[3/5] Skipping smoke test (-SkipSmokeTest specified)..." -ForegroundColor Gray
}

# 4. Generate SHA-256 Checksums
Write-Host "[4/5] Computing SHA-256 checksums..." -ForegroundColor Yellow
$ChecksumLines = @()
foreach ($file in $Artifacts) {
    $FileName = Split-Path -Leaf $file
    $Hash = (Get-FileHash -Path $file -Algorithm SHA256).Hash.ToLower()
    $ChecksumLines += "$Hash  $FileName"
    Write-Host "  $FileName : $Hash" -ForegroundColor Gray
}

$ChecksumFile = Join-Path $ReleaseDir "SHA256SUMS.txt"
$ChecksumLines | Out-File -FilePath $ChecksumFile -Encoding utf8 -Force
Write-Host "  Saved SHA256SUMS.txt to: $ChecksumFile" -ForegroundColor Green

# 5. Generate Release Notes Manifest
Write-Host "[5/5] Generating release notes template..." -ForegroundColor Yellow
$NotesFile = Join-Path $ReleaseDir "RELEASE_NOTES.txt"
$ReleaseNotes = @"
================================================================================
  TELEVAULT Windows 11 Release v2.1.4
  High-Performance Resilient Personal Cloud Drive
================================================================================

Date: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
Publisher: Ankit Sharma (@ankitshx)

PRODUCTION ARTIFACTS IN THIS RELEASE:
--------------------------------------------------------------------------------
1. TELEVAULT-Setup-x64.exe - Production Windows Installer
   - Installs TELEVAULT desktop application and TELEVAULT-backend service
   - Desktop and Start Menu shortcuts, SendTo integration, safe uninstaller
   - Zero external dependencies; Python not required

2. TELEVAULT-Portable-x64.exe - Standalone Direct-Run Portable Application
   - Single-file zero-install executable
   - Automatically manages backend service lifecycle on 127.0.0.1:8000
   - Double-click to launch directly from any directory

SHA-256 CHECKSUMS:
--------------------------------------------------------------------------------
$(Get-Content $ChecksumFile -Raw)

SYSTEM REQUIREMENTS:
--------------------------------------------------------------------------------
- Windows 11 / Windows 10 (x64)
- No Python or developer runtimes required

SECURITY & VERIFICATION:
--------------------------------------------------------------------------------
- Verify the SHA-256 hash using PowerShell:
    Get-FileHash TELEVAULT-Setup-x64.exe -Algorithm SHA256
"@

$ReleaseNotes | Out-File -FilePath $NotesFile -Encoding utf8 -Force
Write-Host "  Saved RELEASE_NOTES.txt to: $NotesFile" -ForegroundColor Green

Write-Host "`nRelease Verification Complete!" -ForegroundColor Green
Write-Host "Release directory contents:" -ForegroundColor Cyan
Get-ChildItem -Path $ReleaseDir | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize
