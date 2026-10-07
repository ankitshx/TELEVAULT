# TELEVAULT

> Your Telegram Vault. Powered by Telegram.

> **Disclaimer**: TELEVAULT is an independent open-source project and is NOT an official Telegram product.

---

## Overview

TELEVAULT is a production-grade Windows desktop application that transforms Telegram into a secure personal cloud and encrypted backup platform. Built with a unified, high-performance Rust core and Tauri 2, TELEVAULT runs entirely in-process without external backends, sidecars, or localhost servers.

## Architecture

```
┌──────────────────────────────────────────────┐
│           TELEVAULT.exe (Tauri 2)            │
│                                              │
│  React + TS UI  ──Tauri IPC──►  Rust Core    │
│                                  │           │
│         ┌────────────────────────┼───────┐   │
│   televault-core         televault-    │   │
│   televault-crypto       telegram      │   │
│   televault-manifest     televault-db  │   │
│   televault-storage      televault-    │   │
│   televault-integrity    transfer      │   │
│         └────────────────────────┴───────┘   │
│                        │                     │
│                   SQLite (embedded)          │
└──────────────────────────────────────────────┘
```

- **Runtime**: Single Windows executable (`TELEVAULT.exe`)
- **Frontend**: React + TypeScript within WebView2
- **Core Engine**: Pure Rust workspace crates
- **Database**: Embedded SQLite (local cache; Telegram is the source of truth)
- **IPC**: Strongly typed IPC via `tauri-specta`

## Features

- [ ] End-to-end encrypted file storage and backup
- [ ] Direct Telegram integration
- [ ] Chunked, verifiable uploads and downloads
- [ ] Cryptographic file integrity verification
- [ ] Embedded local SQLite catalog and sync engine
- [ ] Automated scheduled backups

## Repository Structure

```
├── apps/
│   ├── desktop/              # Tauri 2 + React Windows Desktop Application
│   └── cli/                  # TELEVAULT CLI utility
├── crates/
│   ├── televault-core/       # Core types, configuration, and path management
│   ├── televault-crypto/     # Cryptographic primitives and encryption
│   ├── televault-manifest/   # Manifest serialization and schema
│   ├── televault-storage/    # Local storage and cache management
│   ├── televault-telegram/   # Telegram communication client
│   ├── televault-db/         # Embedded SQLite database and queries
│   ├── televault-transfer/   # Upload/download pipeline & chunking
│   ├── televault-backup/     # Backup engine and job orchestration
│   ├── televault-scheduler/  # Background scheduling engine
│   └── televault-integrity/  # Hash validation and integrity checking
├── protocol/
│   ├── schemas/              # Protocol data schemas
│   └── specification/        # Protocol specifications
├── scripts/                  # Build and development scripts
├── tests/                    # Integration and regression tests
└── docs/                     # Architectural documentation
```

## Getting Started

*(Placeholder: Instructions for building and running TELEVAULT will be provided in upcoming phases.)*

### Prerequisites

- Rust (latest stable toolchain)
- Node.js (v20+ recommended)
- Windows 10/11 with WebView2 Runtime

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
