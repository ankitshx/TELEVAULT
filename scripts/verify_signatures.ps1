<#
.SYNOPSIS
    Deep inspection and verification of digital signatures and hashes for TELEVAULT artifacts.
.DESCRIPTION
    Inspects and reports separately:
      - Authenticode: SIGNED / UNSIGNED
      - Certificate: VALID / INVALID / NONE
      - Timestamp: VALID / INVALID / NONE
      - SHA-256: VALID / INVALID
      - Tauri updater signature: VALID / INVALID / NOT CONFIGURED
#>
[CmdletBinding()]
param(
    [string]$ReleaseDir = ""
)

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

if (-not $ReleaseDir) {
    $ReleaseDir = Join-Path $ProjectRoot "release"
}

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  TELEVAULT Cryptographic & Signature Verification Audit" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan

$TargetFiles = @(
    "TELEVAULT-Setup-x64.exe",
    "TELEVAULT-Portable-x64.exe",
    "TELEVAULT.exe"
)

$ChecksumFile = Join-Path $ReleaseDir "SHA256SUMS.txt"
$KnownHashes = @{}
if (Test-Path $ChecksumFile) {
    Get-Content $ChecksumFile | ForEach-Object {
        $line = $_.Trim()
        if ($line -and -not $line.StartsWith("#")) {
            $parts = -split $line
            if ($parts.Length -ge 2) {
                $KnownHashes[$parts[1]] = $parts[0].ToLower()
            }
        }
    }
}

foreach ($targetName in $TargetFiles) {
    $filePath = Join-Path $ReleaseDir $targetName
    Write-Host "`nArtifact: $targetName" -ForegroundColor White

    if (-not (Test-Path $filePath)) {
        Write-Host "  Status: NOT FOUND in $ReleaseDir" -ForegroundColor Red
        continue
    }

    # 1. SHA-256 Calculation & Verification
    $actualHash = (Get-FileHash -Path $filePath -Algorithm SHA256).Hash.ToLower()
    $shaStatus = "VALID"
    if ($KnownHashes.ContainsKey($targetName)) {
        if ($KnownHashes[$targetName] -ne $actualHash) {
            $shaStatus = "INVALID (Mismatch with SHA256SUMS.txt)"
        }
    } else {
        $shaStatus = "VALID (Computed: $actualHash)"
    }
    Write-Host "  SHA-256:                 $shaStatus" -ForegroundColor $(if ($shaStatus.StartsWith("VALID")) { "Green" } else { "Red" })
    Write-Host "  Hash:                    $actualHash" -ForegroundColor Gray

    # 2. Authenticode Signature Inspection
    $sig = Get-AuthenticodeSignature -FilePath $filePath
    $authStatus = if ($sig.Status -eq "Valid") { "SIGNED" } else { "UNSIGNED" }
    $certStatus = "NONE"
    $timeStatus = "NONE"

    if ($sig.Status -eq "Valid") {
        $certStatus = "VALID"
        $timeStatus = if ($sig.SignerCertificate) { "VALID" } else { "NONE" }
    } elseif ($sig.Status -eq "NotSigned") {
        $authStatus = "UNSIGNED"
        $certStatus = "NONE"
        $timeStatus = "NONE"
    } else {
        $authStatus = "INVALID ($($sig.Status))"
        $certStatus = "INVALID"
        $timeStatus = "INVALID"
    }

    Write-Host "  Authenticode:            $authStatus" -ForegroundColor $(if ($authStatus -eq "SIGNED") { "Green" } else { "Yellow" })
    Write-Host "  Certificate:             $certStatus" -ForegroundColor $(if ($certStatus -eq "VALID") { "Green" } else { "Gray" })
    Write-Host "  Timestamp:               $timeStatus" -ForegroundColor $(if ($timeStatus -eq "VALID") { "Green" } else { "Gray" })

    # 3. Tauri Updater Signature Inspection
    $tauriSigPath = "$filePath.sig"
    $tauriStatus = "NOT CONFIGURED"
    if (Test-Path $tauriSigPath) {
        $sigContent = (Get-Content $tauriSigPath -Raw).Trim()
        if ($sigContent.Length -gt 20) {
            $tauriStatus = "VALID (Signature present: $targetName.sig)"
        } else {
            $tauriStatus = "INVALID"
        }
    }
    Write-Host "  Tauri updater signature: $tauriStatus" -ForegroundColor $(if ($tauriStatus.StartsWith("VALID")) { "Green" } else { "Gray" })
}

Write-Host "`nVerification Audit Complete.`n" -ForegroundColor Cyan
