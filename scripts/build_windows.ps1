<#
.SYNOPSIS
    Builds the TELEVAULT standalone Windows executable using PyInstaller.
.DESCRIPTION
    Compiles TELEVAULT into dist/TELEVAULT.exe with all PyQt6, Telethon,
    cryptographic, and asset resources bundled.
.PARAMETER Mode
    'Development' for fast local builds; 'Production' for clean, optimized builds.
.PARAMETER SkipTests
    Skips the pre-build pytest test suite.
#>
[CmdletBinding()]
param(
    [ValidateSet("Development", "Production")]
    [string]$Mode = "Production",

    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "  TELEVAULT Windows Build Pipeline - Mode: $Mode" -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

# 1. Environment & Dependency Validation
Write-Host "[1/5] Validating build environment..." -ForegroundColor Yellow
$PythonCmd = "python"
if (-not (Get-Command $PythonCmd -ErrorAction SilentlyContinue)) {
    throw "Python was not found in PATH. Please install Python 3.12+."
}

$PyVersion = & $PythonCmd -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}')"
Write-Host "      Detected Python: $PyVersion" -ForegroundColor Gray

# Check required build modules
& $PythonCmd -c "import PyInstaller, PyQt6, telethon, cryptography, argon2, keyring" 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Host "      Installing/verifying required packaging dependencies..." -ForegroundColor Gray
    & $PythonCmd -m pip install -q -e "$ProjectRoot[dev]"
}

# 2. Application Validation (Run Test Suite if requested)
if (-not $SkipTests -and $Mode -eq "Production") {
    Write-Host "[2/5] Running test suite validation..." -ForegroundColor Yellow
    & $PythonCmd -m pytest -q "$ProjectRoot\tests"
    if ($LASTEXITCODE -ne 0) {
        throw "Pre-build test validation failed! Fix failing tests before production packaging."
    }
    Write-Host "      All tests passed successfully." -ForegroundColor Green
} else {
    Write-Host "[2/5] Skipping tests (Mode: $Mode, SkipTests: $SkipTests)..." -ForegroundColor DarkGray
}

# 3. Clean previous build artifacts
Write-Host "[3/5] Cleaning previous build caches..." -ForegroundColor Yellow
$DistDir = Join-Path $ProjectRoot "dist"
$BuildDir = Join-Path $ProjectRoot "build"
$SpecFile = Join-Path $ProjectRoot "packaging\televault.spec"

if (Test-Path (Join-Path $DistDir "TELEVAULT-backend.exe")) {
    Remove-Item (Join-Path $DistDir "TELEVAULT-backend.exe") -Force -ErrorAction SilentlyContinue
}
if ($Mode -eq "Production" -and (Test-Path $BuildDir)) {
    Remove-Item $BuildDir -Recurse -Force -ErrorAction SilentlyContinue
}

# 4. PyInstaller Build
Write-Host "[4/5] Executing PyInstaller build..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
    $PyInstallerArgs = @(
        "-m", "PyInstaller",
        "$SpecFile",
        "--noconfirm",
        "--distpath", "$DistDir",
        "--workpath", "$BuildDir"
    )
    if ($Mode -eq "Production") {
        $PyInstallerArgs += "--clean"
    }

    & $PythonCmd @PyInstallerArgs
    if ($LASTEXITCODE -ne 0) {
        throw "PyInstaller compilation failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}

# 5. Output Verification
$TargetExe = Join-Path $DistDir "TELEVAULT-backend.exe"
if (-not (Test-Path $TargetExe)) {
    throw "Build failed: Output executable '$TargetExe' was not created."
}

$ExeSize = (Get-Item $TargetExe).Length / 1MB
Write-Host "[5/5] Build Successful!" -ForegroundColor Green
Write-Host "      Artifact: $TargetExe ($([math]::Round($ExeSize, 2)) MB)" -ForegroundColor Green

# Clean intermediate build folder in Production mode
if ($Mode -eq "Production" -and (Test-Path $BuildDir)) {
    Remove-Item $BuildDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "Done.`n"
