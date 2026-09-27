<#
.SYNOPSIS
    Builds the professional Windows installer for TELEVAULT using Inno Setup.
.DESCRIPTION
    Compiles installer/TELEVAULT.iss to produce release/TELEVAULT-Setup.exe.
.PARAMETER IssPath
    Path to the Inno Setup script. Defaults to installer/TELEVAULT.iss.
#>
[CmdletBinding()]
param(
    [string]$IssPath = ""
)

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "  TELEVAULT Windows Installer Build (Inno Setup)" -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

# 1. Verify Prerequisites
$DistExe = Join-Path $ProjectRoot "dist\TELEVAULT.exe"
if (-not (Test-Path $DistExe)) {
    throw "Target executable '$DistExe' does not exist. Run scripts/build_windows.ps1 first."
}

if (-not $IssPath) {
    $IssPath = Join-Path $ProjectRoot "installer\TELEVAULT.iss"
    if (-not (Test-Path $IssPath)) {
        $IssPath = Join-Path $ProjectRoot "packaging\installer.iss"
    }
}

if (-not (Test-Path $IssPath)) {
    throw "Installer script '$IssPath' not found."
}

# 2. Locate ISCC.exe (Inno Setup Compiler)
$IsccCandidates = @(
    (Get-Command iscc.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue),
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 7\ISCC.exe",
    "C:\Program Files\Inno Setup 7\ISCC.exe"
)

$IsccPath = $null
foreach ($cand in $IsccCandidates) {
    if ($cand -and (Test-Path $cand)) {
        $IsccPath = $cand
        break
    }
}

if (-not $IsccPath) {
    throw "Inno Setup Compiler (ISCC.exe) was not found. Please install Inno Setup 6 via: winget install JRSoftware.InnoSetup"
}

Write-Host "Using Inno Setup Compiler: $IsccPath" -ForegroundColor Gray
Write-Host "Compiling installer script: $IssPath" -ForegroundColor Gray

# Ensure release output directory exists
$ReleaseDir = Join-Path $ProjectRoot "release"
if (-not (Test-Path $ReleaseDir)) {
    New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null
}

# 3. Compile Installer
Push-Location (Split-Path -Parent $IssPath)
try {
    & "$IsccPath" "$IssPath"
    if ($LASTEXITCODE -ne 0) {
        throw "Inno Setup compilation failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}

# 4. Verify Output
$InstallerExe = Join-Path $ReleaseDir "TELEVAULT-Setup.exe"
if (-not (Test-Path $InstallerExe)) {
    # Check for versioned filename fallback
    $Found = Get-ChildItem -Path $ReleaseDir -Filter "*TELEVAULT*Setup*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($Found) {
        $InstallerExe = $Found.FullName
    } else {
        throw "Installer output was not created in '$ReleaseDir'."
    }
}

$SizeMB = (Get-Item $InstallerExe).Length / 1MB
Write-Host "✅ Installer created successfully!" -ForegroundColor Green
Write-Host "   Output: $InstallerExe ($([math]::Round($SizeMB, 2)) MB)" -ForegroundColor Green
Write-Host "Done.`n"
