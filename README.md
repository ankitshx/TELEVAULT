# TELEVAULT

> **Your Personal Cloud. Powered by Telegram.**
>
> Production-grade, high-performance personal cloud-drive desktop and CLI application that uses Telegram as a resilient, infinite object storage backend.
> 
> 👨‍💻 **Created & Developed by [Ankit Sharma (@ankitshx)](https://github.com/ankitshx)**
> 
> *Disclaimer: TELEVAULT is an independent open-source project and is NOT an official Telegram product.*

[![Author](https://img.shields.io/badge/Author-Ankit%20Sharma%20(@ankitshx)-06B6D4.svg?style=for-the-badge&logo=github)](https://github.com/ankitshx)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011%20x64-brightgreen.svg?style=for-the-badge&logo=windows)]()
[![License](https://img.shields.io/badge/license-MIT-green.svg?style=for-the-badge)](LICENSE)
[![Download Installer](https://img.shields.io/badge/Download-TELEVAULT--Setup--x64.exe-0078D4?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/ankitshx/TELEVAULT/releases/latest/download/TELEVAULT-Setup-x64.exe)

---

## ⬇️ Download TELEVAULT for Windows

**Latest Stable Release: Windows 10/11 x64**

*Zero external dependencies — no Python, Node.js, Rust, or Git required.*

| Edition | Direct Download | Notes |
| :--- | :--- | :--- |
| **Windows Installer (Recommended)** | **[⬇️ TELEVAULT-Setup-x64.exe](https://github.com/ankitshx/TELEVAULT/releases/latest/download/TELEVAULT-Setup-x64.exe)** | Standard installer with Start Menu & Desktop shortcuts, SendTo integration, and persistent data protection |
| **Portable Version** | **[⬇️ TELEVAULT-Portable-x64.exe](https://github.com/ankitshx/TELEVAULT/releases/latest/download/TELEVAULT-Portable-x64.exe)** | Single standalone executable — direct run without installation |

*All releases, source code, and release notes:* **[GitHub Releases](https://github.com/ankitshx/TELEVAULT/releases)**

#### 🛡️ Windows SmartScreen Note ("Windows protected your PC")
> As TELEVAULT is a free community open-source project, it does not use an expensive enterprise EV certificate.
> When Windows SmartScreen displays the notification:
> 1. Click **"More info"**
> 2. Click **"Run anyway"**
> 
> *Tip: Or unblock the downloaded file with PowerShell: `Unblock-File .\TELEVAULT-Setup-x64.exe`.*

---

## 🚀 Installation & First Run

### 1. Install & Launch
1. Download **`TELEVAULT-Setup-x64.exe`** and run the setup wizard.
2. Launch TELEVAULT from your Desktop shortcut or Start Menu.

### 2. Login Once
1. Enter your Telegram phone number, **API ID**, and **API Hash** (obtained free in 1 minute from [my.telegram.org](https://my.telegram.org)).
2. Enter your OTP verification code.
3. Your session is encrypted and securely stored in **Windows Credential Manager**.

### 3. Persistent Across Launches
- Close TELEVAULT and launch it anytime — your session is automatically detected and the dashboard opens immediately.
- Local SQLite database and transfer state are stored in `%LOCALAPPDATA%\TeleVault` and `%LOCALAPPDATA%\TeleCloud`.
- Installing newer versions or updates preserves all existing data and logins.

---

## 🛠️ Build from Source (Developers Only)
```bash
# Clone the repository
git clone https://github.com/ankitshx/TELEVAULT.git
cd TELEVAULT

# Build Desktop App
cd apps/desktop
npm install
npm run build
npx tauri build --no-bundle
```


---

## 🌟 Key Features

- **Logical File & Folder Abstraction:** You interact with your files and folders like any modern cloud drive (Google Drive, OneDrive, Dropbox). Telegram channels, messages, and chunk partitions are transparent implementation details.
- **Streaming Large-File Chunking Engine:** Seamlessly slice and transfer multi-gigabyte files (e.g. 5 GB to 50+ GB disk images and videos) with bounded $O(1)$ RAM usage.
- **Cryptographic Versioned Manifests:** Every multi-part file is governed by a tamper-evident manifest with independent chunk SHA-256 signatures and whole-file cryptographic proofs.
- **Crash-Resilient Transfer Queue:** Transfer state is persisted in local SQLite. If the app closes or the OS crashes during a 20 GB upload, it resumes on next boot without re-uploading completed chunks.
- **Zero-Knowledge Streaming Encryption:** AES-256-GCM authenticated encryption per chunk with Argon2id key derivation, CSPRNG nonces, and chunk index authenticated in associated data.
- **Decoupled Storage Provider:** Business logic depends on an asynchronous Rust trait abstraction (`StorageProvider`), isolating Telegram MTProto internals.
- **Dual-Platform Architecture:**
  - **Windows Desktop:** Tauri 2 + React 18 + TypeScript + Vanilla CSS with dark glassmorphism design, native system tray, notifications, and Windows Credential Manager integration.
  - **Linux CLI:** High-performance headless binary sharing 100% of the Rust core.
  - **Android:** Deferred to future scope; strictly documented in [`docs/platforms/android.md`](docs/platforms/android.md).

---

## 🏛️ System Architecture

```
                         TELECLOUD
                            │
              ┌─────────────┴─────────────┐
              │                           │
        Windows Desktop               Linux CLI
              │                           │
         Tauri 2 GUI                  Rust CLI
              │                           │
              └─────────────┬─────────────┘
                            │
                        Rust Core
                            │
          ┌─────────────────┼──────────────────┐
          │                 │                  │
     File Engine     Transfer Engine     Telegram Layer
          │                 │                  │
          └─────────────────┼──────────────────┘
                            │
                         SQLite
                            │
                     Telegram Storage
```

---

## 📂 Repository Structure

```
TELECLOUD/
├── apps/
│   ├── desktop/                         # Windows Desktop (Tauri 2 + React/TS)
│   └── cli/                             # Linux / Cross-platform Rust CLI
├── crates/
│   ├── telecloud-core/                  # Domain models, errors, configuration
│   ├── telecloud-manifest/              # Versioned file manifests (v1 JSON), builder, parser
│   ├── telecloud-crypto/                # AES-256-GCM streaming cipher, Argon2id KDF
│   ├── telecloud-storage/               # StorageProvider trait & mock provider
│   ├── telecloud-telegram/              # Telegram MTProto adapter & OS keyring
│   ├── telecloud-db/                    # SQLite migrations, relational repository
│   ├── telecloud-transfer/              # Tokio async transfer engine (queue, pause, resume)
│   └── telecloud-integrity/             # Chunk & whole-file SHA-256 verifiers
├── protocol/
│   ├── schemas/manifest_v1.json         # JSON Schema for TeleCloud Manifest V1
│   └── specification/storage_protocol.md# Storage protocol formal specification
├── docs/
│   └── platforms/
│       ├── windows.md                   # Windows Desktop platform specification
│       ├── linux.md                     # Linux CLI platform specification
│       └── android.md                   # Deferred Android platform specification
├── Cargo.toml                           # Cargo Workspace configuration
└── README.md
```

---

## 🚀 Building & Running

### Prerequisites
- **Rust Toolchain:** 1.80+ (MSVC or MinGW)
- **Node.js:** 20+ and npm

### 1. Build Desktop Application
```bash
cd apps/desktop
npm install
npm run build
npm run tauri dev
```

### 2. Build Linux / Headless CLI
```bash
cargo build --release --bin telecloud
./target/release/telecloud --help
```

---

## 🔐 Security & Cryptography

1. **At-Rest Zero-Knowledge Encryption:** Files are optionally encrypted client-side using `AES-256-GCM` before transmission. Telegram servers never receive plaintext content or filenames.
2. **Key Derivation:** Argon2id with 64 MB memory cost, 3 iterations, 2 parallelism lanes, and a 16-byte random salt.
3. **Chunk Binding:** The 8-byte big-endian chunk index is authenticated within the AES-GCM Associated Data (AAD), preventing chunk swap or truncation attacks.
4. **Zeroize Protection:** Sensitive keys and credentials implement Rust's `zeroize::Zeroize` trait to wipe plaintext bytes from memory on drop.

---

## 👨‍💻 Author & Maintainer

Created with ❤️ by **Ankit Sharma** ([@ankitshx](https://github.com/ankitshx)).

---

## 📜 License

MIT License. Copyright (c) 2026 Ankit Sharma (@ankitshx).
