# UI/UX Specification: Dark Terminal-Inspired Schematic

**Owner:** Agent 3 (UI/UX)  
**Status:** Approved for Phase 0 (Specification & Wireframes - No GUI Code in Phase 0)  
**Target Toolkit:** PyQt6 (Desktop, Windows 11, DPI 100% / 125% / 150%)  

---

## 1. Visual Aesthetics & Design System

### 1.1 Palette & Color Tokens
A strict technical, high-contrast, schematic palette:
- **Background Root:** `#0B0C0E` (Deep Carbon)
- **Panel Surface:** `#14171C` (Muted Slate Panel)
- **Border / Grid Lines:** `#242A35` (1px clean technical boundary)
- **Text Primary:** `#ECEFF4` (Crisp Monospace White)
- **Text Secondary:** `#7E889B` (Muted Steel Gray)
- **Active / Accent:** `#F59E0B` (Schematic Industrial Amber)
- **Healthy Dot:** `#10B981` (Terminal Emerald Green)
- **Degraded Alert:** `#F59E0B` (Warning Amber)
- **Lost / Error:** `#EF4444` (Telemetry Crimson)

### 1.2 Typography
- **Headings & Badges:** `JetBrains Mono` (Bold / SemiBold, uppercase badges)
- **Data Tables & Labels:** `IBM Plex Mono` (Regular 13px / Medium 13px)
- **Numbers / Hashes:** `IBM Plex Mono` tabular figures for exact column alignment.

---

## 2. Window Hierarchy & Layout

Minimum Window Dimensions: `960 x 640` px. Fully responsive to resizing.

```text
+-----------------------------------------------------------------------------------------+
| [ TELEVAULT ]  [#] [● ALL FILES SAFE]                 [ VERIFY VAULT ]  [ SETTINGS ]     |
+-----------------------------------------------------------------------------------------+
|                                                                                         |
|   + - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - +   |
|   |                         DROP FILES HERE TO BACK UP                              |   |
|   |                       [ CHOOSE FILES ]   [ CHOOSE FOLDER ]                      |   |
|   + - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - +   |
|                                                                                         |
|   [PIPELINE] report_2026.pdf (48.2 MB)                                                  |
|   [HASH: OK]  -->  [UPLOAD: 74%  3.4 MB/s  ETA 00:04]  -->  [MIRROR]  -->  [VERIFY]     |
|   ===================================================================================   |
|                                                                                         |
|   SEARCH: [ Find by name, ext or #tag...          ]   FILTER: [ ALL STATES ▼ ]           |
|                                                                                         |
|   NAME               SIZE      BACKED UP      HEALTH     LOCAL STATUS     ACTIONS       |
|   -----------------------------------------------------------------------------------   |
|   report_2026.pdf    48.2 MB   Today 14:10    ● HEALTHY  Present          [RESTORE] [:] |
|   database.kdbx       1.4 MB   Yesterday      ● HEALTHY  Changed          [BACKUP]  [:] |
|   archive_raw.tar   840.0 MB   2026-09-12     ● HEALTHY  Only in Telegram [RESTORE] [:] |
|   budget_final.xlsx   8.1 MB   2026-09-01     ▲ DEGRADED Missing          [HEAL]    [:] |
|                                                                                         |
+-----------------------------------------------------------------------------------------+
| TOTAL: 4 FILES | VAULT SIZE: 897.7 MB | MIRROR: TELEVAULT MIRROR (ONLINE)               |
+-----------------------------------------------------------------------------------------+
```

---

## 3. Screen & Dialog Inventory

### 3.1 First-Run Setup Wizard (under 2 minutes)
- Step 1: MTProto Credentials (`api_id`, `api_hash`, direct link to `https://my.telegram.org`).
- Step 2: Phone Number & verification code / 2FA password prompt.
- Step 3: Vault Channel Provisioning (create/verify `TeleVault Primary` and `TeleVault Mirror`).
- Step 4: Vault Mode Selection (Original vs. Private mode toggle).

### 3.2 Over-Limit Dialog
- Triggered when file exceeds 2 GB (Free) or 4 GB (Premium).
- Clear copy: *"File 'dataset.zip' is 2.8 GB, which exceeds your account limit (2.0 GB)."*
- Choices:
  - `[ Cancel ]` (Default, primary button)
  - `[ Split into parts (.part001, .part002) ]`

### 3.3 Restore Dialog
- Displays destination folder picker.
- Progress bar per file with active download speed.
- Post-download SHA-256 verification banner: `[✓ HASH MATCH CONFIRMED]`.

### 3.4 Health Report & Healing Screen
- Detailed table listing any `DEGRADED` or `LOST` items.
- Specific diagnosis per row (*"Missing in Primary channel; surviving copy found in Mirror"*).
- One-click `[ Heal Now ]` button (re-forwards surviving copy; never re-uploads from PC).

### 3.5 Settings Dialog
- Vault mode toggle (Original vs. Private Mode with argon2 passphrase).
- Local recovery copy toggle (enabled by default for files < 500 MB).
- Version retention count (default: 5).
- Concurrency limit (default: 1).
- System tray minimization toggle.

---

## 4. UI States Specification

1. **Empty Vault:** Schematic dashed frame with message: *"No files protected yet. Drag and drop any file or folder to initiate backup."*
2. **Offline / Network Disconnected:** Amber warning strip across top: *"Telegram disconnected. Waiting for connection..."* with automatic reconnection.
3. **FloodWait State:** Clear timer countdown: *"Telegram FloodWait: pausing requests for 42s to protect account standing."*
4. **Duplicate Detected:** Non-blocking notification banner: *"File 'report.pdf' (SHA-256 matches) is already protected in TeleVault. Upload skipped."*
