<#
.SYNOPSIS
    Complete automated end-to-end Windows production release pipeline for TELEVAULT.
.DESCRIPTION
    Runs:
      1. Application Validation (pytest suite)
      2. PyInstaller Build (dist/TELEVAULT.exe)
      3. Installer Build (release/TELEVAULT-Setup.exe)
      4. Code Signing (Authenticode SHA-256 with timestamp)
      5. Verification & Manifest Generation (release/SHA256SUMS.txt)
.PARAMETER Mode
    'Production' (default) or 'Development'.
.PARAMETER SkipTests
    Skips the pytest suite before building.
.PARAMETER StrictSigning
    Fails the release if no valid Authenticode certificate is configured.
#>
[CmdletBinding()]
param(
    [ValidateSet("Development", "Production")]
    [string]$Mode = "Production",

    [switch]$SkipTests,

    [switch]$StrictSigning
)

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  TELEVAULT Production Windows Release Orchestrator" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan

# Step 1: Build Windows Executable
& "$ScriptDir\build_windows.ps1" -Mode $Mode -SkipTests:$SkipTests
if ($LASTEXITCODE -ne 0) { throw "Step 1 (Build Executable) failed." }

# Step 2: Build Installer
& "$ScriptDir\build_installer.ps1"
if ($LASTEXITCODE -ne 0) { throw "Step 2 (Build Installer) failed." }

# Step 3: Sign Binaries
& "$ScriptDir\sign_release.ps1" -Strict:$StrictSigning
if ($LASTEXITCODE -ne 0) { throw "Step 3 (Code Signing) failed." }

# Step 4: Verify Release Artifacts & Generate Checksums
& "$ScriptDir\verify_release.ps1"
if ($LASTEXITCODE -ne 0) { throw "Step 4 (Release Verification) failed." }

Write-Host "================================================================" -ForegroundColor Green
Write-Host "  🎉 TELEVAULT RELEASE ARTIFACTS READY FOR DISTRIBUTION!" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host "Output Directory: $(Join-Path (Split-Path -Parent $ScriptDir) 'release')" -ForegroundColor Yellow
Write-Host ""
