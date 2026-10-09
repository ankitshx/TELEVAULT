# TELEVAULT — Canonical Windows Desktop Release Build Script
# Usage: powershell -ExecutionPolicy Bypass -File scripts\build_release.ps1

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  TELEVAULT — Production Windows Release Build Pipeline  " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# Step 1: Build latest React frontend production bundle
Write-Host "`n[1/3] Building React Frontend Production Bundle..." -ForegroundColor Yellow
Push-Location "apps\desktop\ui"
try {
    npm.cmd run build
    if ($LASTEXITCODE -ne 0) {
        throw "Frontend build failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

# Step 2: Compile Rust release executable with embedded frontend assets
Write-Host "`n[2/3] Compiling TELEVAULT Windows GUI Desktop Executable..." -ForegroundColor Yellow
cargo build --release -p televault-desktop
if ($LASTEXITCODE -ne 0) {
    throw "Cargo release build failed with exit code $LASTEXITCODE"
}

# Step 3: Validate output executable
Write-Host "`n[3/3] Validating Release Artifact..." -ForegroundColor Yellow
$exePath = "target\release\TELEVAULT.exe"
if (-not (Test-Path $exePath)) {
    throw "Target executable not found at $exePath"
}

$exeItem = Get-Item $exePath
Write-Host "Output: $exePath ($([math]::Round($exeItem.Length / 1MB, 2)) MB)" -ForegroundColor Green
Write-Host "`n[SUCCESS] TELEVAULT release build completed successfully!" -ForegroundColor Green
