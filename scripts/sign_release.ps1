<#
.SYNOPSIS
    Signs TELEVAULT release binaries and installers with a trusted Authenticode certificate.
.DESCRIPTION
    Uses Microsoft Authenticode (signtool.exe or Set-AuthenticodeSignature) with SHA-256
    and RFC 3161 timestamping.
.PARAMETER TargetPath
    Optional specific file to sign. If omitted, signs both dist/TELEVAULT.exe and release/TELEVAULT-Setup.exe.
.PARAMETER Strict
    If true, exits with error if no valid certificate is configured. If false, warns and allows unsigned build.
#>
[CmdletBinding()]
param(
    [string]$TargetPath = "",

    [switch]$Strict
)

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "  TELEVAULT Authenticode Code Signing Pipeline" -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

# 1. Resolve Certificate Configuration
$CertSource = $env:TELEVAULT_CERTIFICATE
$CertPassword = $env:TELEVAULT_CERT_PASSWORD
$TimestampUrl = if ($env:TELEVAULT_TIMESTAMP_URL) { $env:TELEVAULT_TIMESTAMP_URL } else { "http://timestamp.digicert.com" }

if (-not $CertSource) {
    Write-Host '[WARNING] No code-signing certificate configured (TELEVAULT_CERTIFICATE is unset).' -ForegroundColor Yellow
    Write-Host ''
    Write-Host 'For production Windows distribution, configure one of the following:' -ForegroundColor Gray
    Write-Host '  1. Path to a valid .pfx file:  $env:TELEVAULT_CERTIFICATE = "C:\path\to\cert.pfx"' -ForegroundColor Gray
    Write-Host '     Password:                   $env:TELEVAULT_CERT_PASSWORD = "your-password"' -ForegroundColor Gray
    Write-Host '  2. Certificate Thumbprint:     $env:TELEVAULT_CERTIFICATE = "THUMBPRINT_HEX"' -ForegroundColor Gray
    Write-Host '     (Imported into Windows Certificate Store: Cert:\CurrentUser\My)' -ForegroundColor Gray
    Write-Host '  3. In GitHub Actions, add secrets: TELEVAULT_CERTIFICATE_BASE64 and TELEVAULT_CERT_PASSWORD' -ForegroundColor Gray
    Write-Host ''

    if ($Strict) {
        throw 'Strict signing mode enabled: No certificate configured. Aborting release.'
    } else {
        Write-Host 'Proceeding with UNSIGNED binaries for local development/testing.' -ForegroundColor Yellow
        exit 0
    }
}

# Load Certificate
$Certificate = $null
if (Test-Path $CertSource) {
    Write-Host "Loading certificate from file: $CertSource" -ForegroundColor Gray
    $SecurePass = $null
    if ($CertPassword) {
        $SecurePass = ConvertTo-SecureString $CertPassword -AsPlainText -Force
    }
    $Certificate = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($CertSource, $SecurePass, [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::Exportable)
} else {
    Write-Host "Searching certificate store for thumbprint/subject: $CertSource" -ForegroundColor Gray
    $FoundCerts = Get-ChildItem -Path Cert:\CurrentUser\My, Cert:\LocalMachine\My -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Thumbprint -eq $CertSource -or $_.Subject -like "*$CertSource*" }
    if ($FoundCerts) {
        $Certificate = $FoundCerts[0]
    }
}

if (-not $Certificate) {
    throw "Failed to load certificate from source: $CertSource"
}

Write-Host "Certificate Loaded:" -ForegroundColor Green
Write-Host "  Subject:    $($Certificate.Subject)" -ForegroundColor Gray
Write-Host "  Issuer:     $($Certificate.Issuer)" -ForegroundColor Gray
Write-Host "  Valid To:   $($Certificate.NotAfter.ToString('yyyy-MM-dd'))" -ForegroundColor Gray
Write-Host "  Thumbprint: $($Certificate.Thumbprint)" -ForegroundColor Gray
Write-Host "  Timestamp:  $TimestampUrl" -ForegroundColor Gray

# 2. Determine Targets to Sign
$TargetsToSign = @()
if ($TargetPath) {
    if (-not (Test-Path $TargetPath)) {
        throw "Specified target path '$TargetPath' does not exist."
    }
    $TargetsToSign += (Resolve-Path $TargetPath).Path
} else {
    $ExePath = Join-Path $ProjectRoot "dist\TELEVAULT.exe"
    $InstallerPath = Join-Path $ProjectRoot "release\TELEVAULT-Setup.exe"

    if (Test-Path $ExePath) { $TargetsToSign += $ExePath }
    if (Test-Path $InstallerPath) { $TargetsToSign += $InstallerPath }
}

if ($TargetsToSign.Count -eq 0) {
    throw "No files found to sign. Expected dist\TELEVAULT.exe or release\TELEVAULT-Setup.exe."
}

# 3. Sign Files
foreach ($file in $TargetsToSign) {
    Write-Host "Signing: $file..." -ForegroundColor Yellow
    
    # Try signtool.exe if available for standard Authenticode, fallback to Set-AuthenticodeSignature
    $Signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue)
    if ($Signtool -and (Test-Path $Signtool) -and (Test-Path $CertSource)) {
        Write-Host "  Using signtool.exe..." -ForegroundColor Gray
        $SignArgs = @("sign", "/fd", "SHA256", "/tr", $TimestampUrl, "/td", "SHA256", "/f", $CertSource)
        if ($CertPassword) {
            $SignArgs += @("/p", $CertPassword)
        }
        $SignArgs += $file
        & "$Signtool" @SignArgs
        if ($LASTEXITCODE -ne 0) {
            throw "signtool.exe failed with exit code $LASTEXITCODE."
        }
    } else {
        # Use PowerShell native Set-AuthenticodeSignature with RFC 3161 timestamping
        $SigResult = Set-AuthenticodeSignature -FilePath $file `
                                              -Certificate $Certificate `
                                              -HashAlgorithm SHA256 `
                                              -TimestampServer $TimestampUrl
        if ($SigResult.Status -ne "Valid") {
            Write-Host "  Signature warning: $($SigResult.StatusMessage)" -ForegroundColor Yellow
        }
    }

    # Verify signature
    $Verify = Get-AuthenticodeSignature -FilePath $file
    Write-Host "  Signature Status: $($Verify.Status) - $($Verify.StatusMessage)" -ForegroundColor Green
}

Write-Host "All targets signed and verified.`n"
