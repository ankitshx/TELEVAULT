# TELEVAULT - Windows 11 Production Release Guide

This document defines the official, reproducible packaging, code-signing, installer creation, and GitHub Release pipeline for **TELEVAULT**.

---

## 1. Architecture & Release Pipeline

The distribution pipeline follows a strict, sequential workflow:

```
Source Code
    ↓
Application Validation (pytest test suite)
    ↓
PyInstaller Build (TELEVAULT.exe)
    ↓
Windows Installer Generation (Inno Setup -> TELEVAULT-Setup.exe)
    ↓
Authenticode Code Signing (SHA-256 + RFC 3161 Timestamp)
    ↓
Release Verification & Checksums (SHA256SUMS.txt)
    ↓
GitHub Release
```

---

## 2. Prerequisites

### Local Developer Environment
- **Operating System:** Windows 11 or Windows 10 (x64)
- **Python:** 3.12+ (tested through 3.14) with virtual environment or system Python
- **Dependencies:** `pip install -e .[dev]` (includes `pyinstaller`, `PyQt6`, `telethon`, `cryptography`, `argon2-cffi`, `keyring`, `pytest`)
- **Inno Setup:** Inno Setup 6+ (Install via `winget install JRSoftware.InnoSetup`)
- **PowerShell:** PowerShell 5.1+ or PowerShell 7 (pwsh)

---

## 3. Build Commands

All build tasks are automated via PowerShell scripts in `scripts/`:

### A. Development Build (Fast, Unsigned)
For quick local testing and validation without full test runs or code signing:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\build_windows.ps1 -Mode Development -SkipTests
```
Output: `dist\TELEVAULT.exe`

### B. Production Build (Clean, Tested, Versioned)
Executes full pytest constitution invariants suite, clean PyInstaller compilation, and cleans temporary directories:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\build_windows.ps1 -Mode Production
```
Output: `dist\TELEVAULT.exe` with embedded Windows file version information.

### C. Build Installer Only
Compiles the Inno Setup script into a standalone installer:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\build_installer.ps1
```
Output: `release\TELEVAULT-Setup.exe`

### D. Full End-to-End Production Release
Orchestrates test validation, PyInstaller compilation, installer creation, code signing (if configured), and SHA-256 checksum generation:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -Mode Production
```
Outputs in `release/`:
- `release\TELEVAULT-Setup.exe` (Professional Installer)
- `release\TELEVAULT.exe` (Standalone Executable)
- `release\SHA256SUMS.txt` (Cryptographic verification checksums)
- `release\RELEASE_NOTES.txt` (Release manifest and summary)

---

## 4. Code Signing & Authenticode Configuration

### Why Code Signing is Required
Unsigned `.exe` files downloaded via web browsers trigger Windows Defender SmartScreen ("Windows protected your PC") due to the **Mark of the Web** (`Zone.Identifier=3`). Signing binaries with a trusted Authenticode certificate establishes software publisher identity and allows reputation building.

### Certificate Types
1. **EV (Extended Validation) Certificate:** Provides hardware token (HSM/YubiKey) security and **instant SmartScreen reputation**.
2. **Standard OV (Organization Validation) / Individual Certificate:** Authenticates the publisher. Requires an initial period of user downloads before SmartScreen establishes positive reputation.

### Supported Certificate Providers
- DigiCert
- Sectigo
- SSL.com
- Certum

### Environment Variables
Configure the following environment variables before running `sign_release.ps1` or `release.ps1`:

| Variable | Description | Example |
|---|---|---|
| `TELEVAULT_CERTIFICATE` | File path to `.pfx` file OR Certificate Thumbprint | `C:\certs\telecloud.pfx` or `A1B2C3D4...` |
| `TELEVAULT_CERT_PASSWORD` | Password for the `.pfx` certificate file | `YourSecurePassword` |
| `TELEVAULT_TIMESTAMP_URL` | RFC 3161 Timestamp Server (Default: DigiCert) | `http://timestamp.digicert.com` |

### Manual Signing Command
```powershell
# Set credentials
$env:TELEVAULT_CERTIFICATE = "C:\path\to\my_certificate.pfx"
$env:TELEVAULT_CERT_PASSWORD = "SecretPassword123"

# Sign all release artifacts
powershell -ExecutionPolicy Bypass -File scripts\sign_release.ps1
```

If no certificate is configured, `scripts/sign_release.ps1` safely detects its absence and creates an unsigned build for testing without error. To enforce signing in CI, pass `-Strict`:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\sign_release.ps1 -Strict
```

---

## 5. Windows Defender SmartScreen & Reputation Behavior

> **IMPORTANT NOTICE REGARDING SMARTSCREEN:**
> Code signing does **NOT** immediately guarantee that SmartScreen warnings disappear on day one.

### How Microsoft Defender SmartScreen Works:
1. When a user downloads a binary through a browser, Windows tags it with the **Mark of the Web**.
2. When the user executes the file, SmartScreen queries Microsoft's cloud intelligence database with the file's hash and publisher certificate thumbprint.
3. **Reputation Establishment:**
   - **Unsigned Apps:** Always flagged as "Unrecognised app".
   - **Signed with Standard OV Certificate:** Starts with neutral reputation. As legitimate users download and run the file (clicking "More info" ➔ "Run anyway"), Microsoft's telemetry builds positive reputation for your certificate. Once established, subsequent releases signed with the same certificate inherit this reputation.
   - **Signed with EV Certificate:** Inherits instant Microsoft SmartScreen trust without warnings.

### Safe User Guidance:
When distributing initial releases:
1. Provide the direct installer: `TELEVAULT-Setup.exe`.
2. Provide verified SHA-256 checksums in the release notes so users can verify byte-for-byte integrity.
3. Advise users: Click **"More info"** ➔ **"Run anyway"** on initial launch.
4. **Never** instruct users to disable Windows Defender or modify registry security settings.

---

## 6. Installer Details & User Data Safety

The Inno Setup installer (`installer/TELEVAULT.iss`) adheres to strict data preservation standards:
- **Installation Directory:** `{autopf}\TELEVAULT` (`C:\Program Files\TELEVAULT` or user local apps)
- **Shortcuts:** Start Menu and optional Desktop shortcut.
- **Uninstaller:** Cleanly registered in Windows *Installed Apps / Control Panel*.
- **Data Preservation Guarantee:** User database files, encryption vaults, credentials, and logs stored in `%LOCALAPPDATA%\TeleVault\` and `%USERPROFILE%\.televault\` are **NEVER deleted or modified** during application upgrades or uninstallation.

---

## 7. Automated GitHub Actions Workflow

In GitHub Actions, `.github/workflows/release-windows.yml` automates the entire pipeline upon pushing a release tag (`v*`):

1. Sets up Python 3.12 and installs dependencies.
2. Runs full pytest test suite.
3. Compiles `TELEVAULT.exe` via PyInstaller.
4. Compiles `TELEVAULT-Setup.exe` via Inno Setup.
5. If GitHub Secrets `TELEVAULT_CERTIFICATE_BASE64` and `TELEVAULT_CERT_PASSWORD` are present, decodes and signs artifacts with Authenticode.
6. Computes `SHA256SUMS.txt`.
7. Drafts a GitHub Release with all signed assets.
