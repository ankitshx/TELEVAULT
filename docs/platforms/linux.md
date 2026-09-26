# TeleCloud — Linux Platform Specification (Secondary Target)

**Platform:** Linux (x86_64, aarch64)  
**Interface:** Headless Command-Line Interface (Rust CLI `telecloud`)  
**Core Runtime:** Rust (`telecloud-core`)  

---

## 1. Overview

Linux support is **CLI-ONLY**. Linux users get a high-performance, scriptable, server-friendly CLI binary with zero graphical dependencies.

Linux CLI shares 100% of the underlying Rust core:
- Same SQLite database models
- Same streaming chunking engine
- Same manifest schema
- Same AES-256-GCM / Argon2id encryption format
- Same Telegram MTProto storage provider

---

## 2. Linux CLI Command Interface

```bash
# Authentication
telecloud login                      # Interactive login (phone + SMS/app code + 2FA password)
telecloud logout                     # Terminate session and clear credentials
telecloud whoami                     # Display current account status and storage limits

# File & Folder Management
telecloud ls [folder]                # List files and folders
telecloud tree [folder]              # Display directory tree
telecloud mkdir <path>               # Create folder in drive hierarchy
telecloud rename <old> <new>         # Rename file or folder
telecloud move <src> <dest>          # Move file or folder
telecloud copy <src> <dest>          # Copy file or folder
telecloud rm <path>                  # Delete file or folder

# Transfers
telecloud upload <local_file> [remote_folder] [--encrypt] [--chunk-size <mb>]
telecloud download <remote_file> [local_dest]
telecloud transfer list              # List active and queued transfers
telecloud transfer pause <id>        # Pause a transfer
telecloud transfer resume <id>       # Resume an interrupted or paused transfer
telecloud transfer retry <id>        # Retry a failed transfer
telecloud transfer cancel <id>       # Cancel transfer and cleanup partial state

# Search & Integrity
telecloud search <query>             # Instant FTS5 search across names and extensions
telecloud verify [file_id]           # Verify cryptographic integrity against manifest
telecloud doctor                     # Run comprehensive diagnostic scan
telecloud repair <file_id>           # Re-upload missing/degraded chunks from local copy
```

---

## 3. Linux Secret Management & Persistence
- Credentials stored via Freedesktop Secret Service API (`libsecret` / `secret-tool`) with fallback to local AES-encrypted key file protected by master passphrase.
- Configuration and local database stored adhering to XDG Base Directory specification:
  - Metadata Database: `$XDG_DATA_HOME/telecloud/telecloud.db` (defaults to `~/.local/share/telecloud/telecloud.db`)
  - Config: `$XDG_CONFIG_HOME/telecloud/config.toml` (defaults to `~/.config/telecloud/config.toml`)
  - Logs: `$XDG_STATE_HOME/telecloud/telecloud.log`
