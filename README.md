# TELEVAULT

> **Your Telegram Vault. Powered by Telegram.**  
> *Production-grade, end-to-end encrypted cloud backup and recovery platform running on Windows.*

> [!NOTE]  
> **Disclaimer**: TELEVAULT is an independent open-source project and is NOT an official Telegram product.

---

## 1. Overview & Purpose

TELEVAULT transforms Telegram into a secure, unlimited personal cloud storage and automated backup system. It is engineered specifically for Windows as a native desktop application, combining an ultra-fast, unified **Rust** core with a modern **Tauri 2** frontend.

TELEVAULT eliminates the fragile multi-process architectures common in desktop apps: there are **zero external daemon processes**, **zero Python runtimes**, **zero localhost HTTP servers**, and **zero backend sidecars**. Everything runs in a single secure process.

---

## 2. Core Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                        TELEVAULT.exe (Tauri 2)                         │
│                                                                        │
│   React 19 + TypeScript UI ──[tauri-specta IPC]──► Rust Core Engine    │
│   (WebView2 Windows Runtime)                      │                    │
│                                                   ▼                    │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │                      Domain Crates Workspace                   │   │
│   │  • televault-core       • televault-crypto    • televault-db   │   │
│   │  • televault-manifest   • televault-storage   • televault-pipe │   │
│   │  • televault-telegram   • televault-transfer  • televault-sched│   │
│   │  • televault-backup     • televault-integrity                  │   │
│   └────────────────────────────────────────────────────────────────┘   │
│                                                   │                    │
│               ┌───────────────────────────────────┴────────────┐       │
│               ▼                                                ▼       │
│      Embedded SQLite Database                        Telegram Cloud API│
│      (WAL mode, Cache & Catalog)                     (Authoritative)   │
└────────────────────────────────────────────────────────────────────────┘
```

- **Runtime**: Single Windows executable (`TELEVAULT.exe`).
- **Frontend**: React + TypeScript inside Microsoft WebView2.
- **Core Engine**: Pure Rust Cargo workspace (12 crates).
- **Embedded Persistence**: SQLite with Write-Ahead Logging (`WAL`), strict foreign keys, FTS5 full-text search, and Refinery migrations.
- **Inter-Process Communication (IPC)**: Strongly typed IPC generated through `tauri-specta` (no raw JSON strings, zero localhost network sockets).

---

## 3. Storage & Integrity Architecture

### 3.1 Cloud-First Storage Model
TELEVAULT is strictly **cloud-first**:
- **Authoritative Source of Truth**: Telegram Cloud stores all actual backup payloads, chunks, and manifests permanently.
- **Local Disk Policy**: The local computer stores only embedded SQLite catalog records, small cache files, temporary staging files during in-flight transfers, and operational logs.
- **Zero Permanent Local Payloads**: Actual backup archive payloads (.zip, .bin, .tar, .stage) never accumulate permanently on the local machine.

### 3.2 Logical Files & Physical Chunk Model
- **Unified Logical Files**: A logical file in TELEVAULT (e.g. a 5.2 GB archive) is presented to the user as a single coherent file entity.
- **Physical Chunking**: Large files exceeding 2 GB are partitioned into standard 1.8 GB chunks (`TARGET_CHUNK_SIZE_BYTES = 1,887,436,800 bytes`).
- **Bounded Streaming Transfer**: Both uploads and downloads execute via bounded **64 KiB streaming buffers** (`STREAM_BUFFER_SIZE = 64 * 1024`). The engine never buffers 5.2 GB or 1.8 GB chunks in system RAM.

### 3.3 Cryptographic Model
- **Authenticated Encryption**: Optional application-level AES-256-GCM symmetric encryption.
- **Key Derivation (KDF)**: Argon2id (64 MiB RAM, 3 iterations, 2 threads, 32-byte salt).
- **Chunk-Aware AAD (`ChunkAad`)**: Every chunk binds `FileId`, `chunk_index`, and `total_chunks` in Authenticated Associated Data, mathematically preventing chunk tampering, index reordering, or cross-file splicing.
- **Secret Zeroization**: All cryptographic secrets implement `zeroize::ZeroizeOnDrop` and are redacted in debug and logging output.

### 3.4 Compression Model
- **High-Throughput Compression**: Optional stream-oriented compression using Zstandard (`zstd`).
- **Policy Driven**: Profiles and manifests explicitly record `CompressionMetadata` (`CompressionAlgorithm::None` or `CompressionAlgorithm::Zstd` with compression level).
- **Zero Heap Buffering**: Compression and decompression operate through streaming readers and writers in bounded 64 KiB chunks, keeping memory consumption strictly constant regardless of file size.

### 3.5 Verification Model (Phase 13)
Verification audits backup integrity across 4 distinct levels:
1. **Level 1 (MetadataOnly)**: Invariant check of catalog manifests, chunk indexing, sizes, and hashes.
2. **Level 2 (RemoteAvailability)**: Queries remote Telegram storage to verify message and file existence.
3. **Level 3 (RemoteIntegrity)**: Validates remote size, hash digests, and server metadata.
4. **Level 4 (RestoreReadiness)**: Complete cryptographic payload validation and end-to-end restore dry-run.

### 3.6 Remote Repair & Recovery Model (Phase 14)
When verification detects damaged or missing remote chunks, the **Remote Repair Engine** recovers them safely:
- **Strict Ownership Isolation**: Validates the unbroken hierarchy:  
  `Profile ➔ Snapshot ➔ File ➔ Manifest ➔ Chunk ➔ Remote Reference`.  
  Cross-profile access or ambiguous manifests trigger immediate repair refusal.
- **Isolated Single-Chunk Repair**: For multi-chunk files (e.g. 5.2 GB across 3 chunks), if Chunk 1 is corrupted, **only Chunk 1** is reconstructed, re-encrypted, and uploaded. Unaffected chunks are untouched.
- **Remote Non-Deletion Guarantee**: Repair uploads replacement objects to fresh Telegram references; old remote objects are **never deleted** during repair (deletion is deferred to an explicitly authorized future phase).
- **Atomic Database Transactions**: Local catalog updates execute inside an atomic SQLite transaction; any failure immediately triggers rollback.
- **Idempotence & Cancellation**: Already-healthy chunks are skipped without duplicate uploads. Cooperative cancellation via `CancellationToken` ensures immediate abort with complete cleanup of temporary staging files.

### 3.7 Desktop UI & Application Integration Model (Phase 15)
TELEVAULT features a unified, dark-theme desktop user experience integrated directly into Tauri 2:
- **Zero Network Drift**: Pure in-process architecture connecting React 19 and TypeScript to the Rust core strictly via `tauri-specta` typed IPC bindings. Zero localhost HTTP, zero WebSocket servers, zero Python runtimes, and zero sidecar executables.
- **10 Core Views Integrated**:
  1. **Dashboard**: Live system status, profile inventory, recent backups, active transfers, verification, and repair summary.
  2. **Backups**: Manual and profile-driven backup trigger, live execution status, cancellation, and error reporting.
  3. **Restore**: Historical snapshot browser, point-in-time file catalog inspection, destination path selection, and deterministic collision policies (`Overwrite`, `Skip`, `KeepBoth`).
  4. **Schedules**: Automation manager supporting Interval, Daily, Weekly, and Cron triggers, with run history and manual trigger overrides.
  5. **Retention**: Multi-rule snapshot retention policy designer (keep latest N, age window, keep latest successful) with dry-run preview and safe atomic execution (zero remote deletion).
  6. **Verification**: 4-level integrity auditor (`MetadataOnly`, `RemoteAvailability`, `RemoteIntegrity`, `RestoreReadiness`) displaying health indicators, finding codes, and repair eligibility.
  7. **Repair**: Guided remote repair wizard with pre-execution eligibility validation, dry-run preview, single-chunk isolated streaming repair, and persistent audit history.
  8. **Activity**: Transfer and operational job monitor showing queued, active, completed, failed, and cancelled transfers with bounded byte counters and throughput tracking.
  9. **Profiles**: Complete profile CRUD management (target paths, exclusion patterns, Zstd compression, and AES-256-GCM encryption configuration).
  10. **Settings**: Centralized application configuration (`AppConfig`) covering general preferences, storage limits, transfer concurrency, and backup defaults.
- **Zero-Payload Streaming UI Contract**: React state never loads, buffers, or retains actual backup payloads or multi-gigabyte files (e.g. 5.2 GB archives). The UI consumes and displays bounded metadata, percentages, and status codes while the Rust backend streams payloads in 64 KiB bounded chunks.

---

## 4. Development Workflow & GitHub Integration

### Mandatory Development Rule
> **"Every completed TELEVAULT phase MUST be committed locally, validated, and pushed to the official GitHub repository before the phase is considered complete."**

### Permanent Phase Workflow for All Future Phases:
1. **Start from Checkpoint**: Always start from the latest approved, synchronized GitHub checkpoint (`origin/main`).
2. **Phase Boundary Discipline**: Implement exactly one phase at a time; never begin later phases prematurely.
3. **Complete Test Suite**: Run `cargo test --workspace` ensuring 100% tests pass (0 failed, 0 ignored).
4. **Code Quality**: Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
5. **Documentation**: Update `README.md` reflecting current cumulative architecture and completed phases.
6. **Architecture Records**: Update `.antigravity/` records (`PROGRESS_LOG.md`, `MISTAKES.md`, `ARCHITECTURE_DECISIONS.md`, `BUILD_RULES.md`, `KNOWN_FAILURES.md`).
7. **Security & Invariant Audit**: Verify zero secrets, zero Telegram credentials, zero unverified payloads, and zero architecture drift (no Python, no localhost, no sidecars).
8. **Phase Commit**: Create a dedicated phase commit with complete cumulative project state.
9. **GitHub Push**: Push commit to `origin/main` ([https://github.com/ankitshx/TELEVAULT.git](https://github.com/ankitshx/TELEVAULT.git)).
10. **Verify Synchronization**: Confirm `HEAD == origin/main` with `git status` and `git branch -vv`.
11. **Stop & Report**: Conclude the phase and await explicit review before proceeding.

### Repository Information
- **Official GitHub Repository**: [https://github.com/ankitshx/TELEVAULT](https://github.com/ankitshx/TELEVAULT)
- **Primary Branch**: `main`
- **Continuous Cumulative Remote Checkpoints**: Every completed phase is pushed immediately to origin, creating an immutable chronological backup of code, tests, and documentation.


---

## 5. Completed Phases & Status

| Phase | Description | Tests | Status |
|:---|:---|:---:|:---:|
| **Phase 0** | Workspace Directory Structure & Git Setup | 0 | Approved |
| **Phase 1** | Cargo Workspace Foundation & 12 Member Crates | 0 | Approved |
| **Phase 2** | `televault-core` Domain Layer & PathManager | 19 | Approved |
| **Phase 3** | `televault-crypto` AES-256-GCM & Argon2id KDF | 23 | Approved |
| **Phase 4** | `televault-manifest` ManifestV1 & Chunk Specification | 20 | Approved |
| **Phase 5** | `televault-db` SQLite Embedded Catalog & FTS5 | 15 | Approved |
| **Phase 6** | `televault-storage` Cloud-First & Temp Staging | 13 | Approved |
| **Phase 7** | `televault-transfer` Bounded Streaming Queue Engine | 27 | Approved |
| **Phase 8** | `televault-backup` Snapshot & Incremental Backup Engine | 9 | Approved |
| **Phase 9** | `televault-backup` Single & Multi-Chunk Restore Engine | 22 | Approved |
| **Phase 10** | `televault-desktop` Tauri 2 Desktop Shell & Specta IPC | 17 | Approved |
| **Phase 11** | `televault-scheduler` Background Scheduling Engine | 15 | Approved |
| **Phase 12** | `televault-backup` Snapshot Retention & Pruning Engine | 34 | Approved |
| **Phase 13** | `televault-integrity` Remote Verification & Audit Engine | 50 | Approved |
| **Phase 14** | Remote Repair & Recovery, Disk Audit & GitHub Sync | 17 | Approved |
| **Phase 15** | **Production Desktop App Integration & UI Completion** | **24** | **Completed** |
| **Total** | **Authoritative Test Baseline (282 Rust + 23 UI)** | **305** | **100% Passed** |

---

## 6. Build & Test Commands

### Prerequisites
- **Rust**: 1.85+ (stable toolchain)
- **Node.js**: v20+ with npm
- **Windows**: Windows 10/11 with WebView2 Runtime

### Running Verification & Tests
```powershell
# Run the Rust test suite (282 tests)
cargo test --workspace

# Run strict Clippy lint checks
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Verify code formatting
cargo fmt --all -- --check

# Run Frontend test suite (23 Vitest tests across 6 suites)
cd apps/desktop/ui
npm test -- --run

# Build Frontend production assets
npm run build
```

### Running the Desktop Application Locally
```powershell
# Navigate to desktop app and start development server
cd apps/desktop
npm install
npm run tauri dev
```

---

## 7. Architecture Restrictions & Invariants

1. **Pure Single-Process**: One executable only. Zero child processes, zero sidecar daemons, zero Python runtimes.
2. **No Localhost**: Zero HTTP/WebSocket servers running on `127.0.0.1`. All UI-to-backend communication uses direct in-process Tauri IPC.
3. **No Unbounded Memory**: Maximum streaming buffer size is 64 KiB. Transferring 5.2 GB logical files allocates less than 50 MB RAM total.
4. **No Secrets in Logs or Catalog**: Telegram tokens, encryption keys, and session credentials are never logged or stored in plain SQLite.
5. **No Remote Deletion in Repair**: Remote Telegram chunks are never deleted during Phase 14 repair operations.
6. **Zero Payload in UI**: React state never retains multi-gigabyte files or backup payload bytes.

---

## 8. Known Limitations & Future Work

- **Future Phase 16+**: Remote garbage collection and explicit user-authorized remote Telegram chunk deletion.
- **Future Phase 17+**: Release packaging, code signing, and installer automation.

---

## 9. License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
