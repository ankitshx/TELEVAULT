# Packaging, CI & OS Integration Plan

**Owner:** Agent 4 (Platform & Packaging)  
**Status:** Approved for Phase 0  
**Target Platform:** Windows 11 (x64), Python 3.12+  

---

## 1. Windows OS Integration

### 1.1 Secure Credential Storage (Keyring)
- Sensitive credentials (`api_id`, `api_hash`, Telethon user session string) are stored in the **Windows Credential Manager** via Python's `keyring` library (`service_name="TeleVault"`).
- Neither credentials nor MTProto session files are stored in plain text or in the application repository.

### 1.2 Explorer "Send To" Integration
- TeleVault installs a shell shortcut into `%APPDATA%\Microsoft\Windows\SendTo\TeleVault.lnk`.
- When the user right-clicks any file or folder in Windows File Explorer and selects **Send to -> TeleVault**, Windows executes:
  ```powershell
  televault.exe backup "<path>"
  ```
- If the GUI application is already running, the path is handed over via the local IPC socket.

### 1.3 Single-Instance Enforcement
- Windows named mutex (`Global\TeleVault_Instance_Mutex`) or local loopback socket.
- Second instances forward arguments (e.g. from "Send To") to the running instance and exit cleanly.

---

## 2. Packaging Specification (PyInstaller Single-Exe)

### 2.1 PyInstaller Spec (`packaging/televault.spec`)
- Target: Single executable (`--onefile --windowed`).
- Bundled assets: Fonts (`assets/fonts/*`), Icons (`assets/icons/*`), SQLite migration scripts.
- Excluded packages: Unused test runners (`pytest`), developer tools (`mypy`, `ruff`).
- Output: `dist/televault.exe`.

### 2.2 Inno Setup Installer (`packaging/installer.iss`)
- Optional installer for clean install / uninstall, desktop icon, and Windows Send-To registration.

---

## 3. Continuous Integration (GitHub Actions)

Workflow file: `.github/workflows/ci.yml`
1. **Lint & Style:** `ruff check .` and `ruff format --check .`
2. **Type Safety:** `mypy televault/`
3. **Constitution / Invariant Tests:** `pytest tests/test_invariants.py -v`
4. **Unit & Resilience Test Suite:** `pytest tests/unit/ -v`
5. **Executable Build Smoke Test:** PyInstaller build verification on `windows-latest`.
