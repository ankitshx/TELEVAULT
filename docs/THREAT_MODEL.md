# TeleVault Threat Model & Security Architecture

**Owner:** Agent 5 (QA & Security)  
**Status:** Approved for Phase 0  

---

## 1. Assets & Trust Boundaries

| Asset | Sensitivity | Storage Location | Protection Mechanism |
| :--- | :--- | :--- | :--- |
| **Telegram Credentials** (`api_id`, `api_hash`, phone) | Critical | Windows Credential Manager | Never committed, never logged. |
| **Session String** (`.session`) | Critical | Windows Credential Manager | Auth token isolating full account control. |
| **User Data Files (Original Mode)** | High | Private Telegram Channels | Access restricted to account owner. |
| **User Data Files (Private Mode)** | Critical | Private Telegram Channels | Client-side AES-256-GCM streaming encryption (argon2id). |
| **Encryption Passphrase** | Critical | Volatile memory only | Never stored anywhere on disk. |
| **Local SQLite Metadata Index** | Medium | `%LOCALAPPDATA%/TeleVault/` | Local user isolation, snapshot backups to Primary channel. |

---

## 2. Threat Scenarios & Mitigations

### T-01: Telegram Account Suspension / Anti-Abuse Flagging
- **Threat:** Automated abuse filters ban user account due to excessive upload traffic or rapid API calls.
- **Mitigation:**
  - Strict concurrency ceiling (`max_concurrent = 1` default).
  - Polite backoff with automated sleep on `FloodWaitError`.
  - No background polling or automated schedulers. Uploads only happen on direct user command.

### T-02: Exposure of Backed Up Data to Telegram Infrastructure
- **Threat:** Telegram staff or third-party subpoenas read cloud chat contents.
- **Mitigation:**
  - **Private Mode:** Streaming AES-256-GCM encryption with key derived via `argon2id`.
  - Filename and metadata are encrypted; Telegram sees only random ciphertext bytes and an opaque UUID.

### T-03: Silent Bitrot or Incomplete Upload / Download
- **Threat:** Network glitch or byte truncation corrupts files silently.
- **Mitigation:**
  - SHA-256 computed on disk prior to upload and recorded in `tv1` metadata caption.
  - Post-restore SHA-256 calculation. Any mismatch aborts restore and flags corruption.

### T-04: Credential Leakage in Debug Logs or Terminal Output
- **Threat:** User phone number, login code, 2FA password, or session token printed in stdout or log files.
- **Mitigation:**
  - Zero logging of auth tokens, codes, or passphrases.
  - Test suites enforce log redaction and credential absence.

### T-05: Accidental Data Loss from Erroneous Delete Calls
- **Threat:** Software bug issues delete commands to Telegram channels.
- **Mitigation:**
  - **Invariant 2:** Architectural constitution enforced by static AST tests. Zero code paths exist in `televault/` to delete Telegram messages.
