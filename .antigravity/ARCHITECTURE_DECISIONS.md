# ARCHITECTURE DECISIONS

## AD-001: Rust-Only Architecture
- Decision: TELEVAULT uses Rust-only architecture with Tauri 2
- Reason: Eliminates localhost, port, sidecar, and Python dependency issues
- Date: 2026-10-07

## AD-002: Single Executable
- Decision: One .exe file with React frontend inside WebView2 and Rust core in same process
- Reason: No version mismatch, no backend startup issues, no DLL missing
- Date: 2026-10-07

## AD-003: Cargo Workspace Member Structure
- Decision: Centralized root Cargo workspace with 2 applications (`apps/cli`, `apps/desktop`) and 10 library crates under `crates/televault-*`
- Reason: Enforces strict modularity, clean domain boundaries, decoupled testing, and zero cyclic dependencies
- Date: 2026-10-07

## AD-004: Windows Static CRT Target Strategy
- Decision: Target Windows release binaries with static C runtime (`RUSTFLAGS="-C target-feature=+crt-static"`) on MSVC toolchains
- Reason: Ensures self-contained distribution without external Visual C++ Redistributable requirements
- Date: 2026-10-07

## AD-005: Centralized Path Resolution Strategy
- Decision: All filesystem paths must resolve exclusively through `PathManager`, utilizing platform-standard directories (`%LOCALAPPDATA%\TELEVAULT` on Windows) and validating subpaths against path traversal attacks
- Reason: Prevents deployment bugs, portability failures, developer-specific hardcoded paths, and directory traversal vulnerabilities
- Date: 2026-10-07

## AD-006: Strong Typed Domain Identifiers
- Decision: All entity identifiers (`ProfileId`, `SnapshotId`, `FileId`, `ChunkId`, `JobId`, `ScheduleId`, `VersionId`) are distinct validated newtypes
- Reason: Prevents parameter mix-up, ensures validity before processing, and enforces domain boundary type safety
- Date: 2026-10-07

## AD-007: Secret Exclusion from Core Configuration
- Decision: `AppConfig` strictly models non-sensitive operational parameters; secrets, session credentials, and cryptographic keys are forbidden in `AppConfig` and will reside in secure OS keyrings in later phases
- Reason: Prevents credential leakage via config files, debug logs, or IPC payloads
- Date: 2026-10-07

## AD-008: Opt-In Application-Level Encryption Contract
- Decision: Application-level encryption in TELEVAULT is strictly opt-in. TELEVAULT provides encryption as a secure capability, but individual upload and backup operations explicitly choose `EncryptionPolicy::Enabled` or `EncryptionPolicy::Disabled`. The system must never silently force encryption
- Reason: Respects user preference, avoids overhead on non-sensitive public uploads, and keeps compression and integrity verification modular
- Date: 2026-10-07

## AD-009: First-Class Single-File Uploads and Chunking Transparency
- Decision: Single-file uploads are first-class operations. Files below the 2 GB threshold are transferred as a single unit; files at or above 2 GB are internally represented as 1.8 GB chunks with authenticated Associated Data (`ChunkAad`) binding file identity, chunk index, and total chunk count. The user interface presents a single logical file, and restore always reconstructs the original logical file
- Reason: Accommodates Telegram's file size boundaries while providing a unified, clean user experience and preventing chunk replay/tampering attacks
- Date: 2026-10-07

## AD-010: Cryptographic Key Memory Protection and Redaction
- Decision: All symmetric keys are encapsulated in `SecretKey` using `zeroize::ZeroizeOnDrop`, with custom `Debug` implementations that redact key bytes (`SecretKey([REDACTED])`), preventing memory retention after scope exit and accidental leakage in diagnostic logs
- Reason: Defense-in-depth security protecting secret keys from memory scraping and log exposure
- Date: 2026-10-07

## AD-011: Manifest Authoritative Logical File Representation
- Decision: `ManifestV1` is the authoritative source of truth defining a logical file, its integrity digest, its encryption/compression state, and its ordered physical storage chunks. Chunks are internal transfer units and are never presented as independent files to the user
- Reason: Decouples physical Telegram storage chunking limits from the user-visible file model and guarantees atomic file representation
- Date: 2026-10-07

## AD-012: Explicit Manifest Specification Versioning
- Decision: The manifest format is strictly versioned via `ManifestVersion::V1`. Deserialization and validation reject unknown or unvalidated versions with `ManifestError::UnsupportedVersion`
- Reason: Prevents silent corruption or misinterpretation when future schema versions (v2, v3) are introduced
- Date: 2026-10-07

## AD-013: Deterministic Chunk Ordering and Restore Invariants
- Decision: Manifest chunk indices must be contiguous starting at zero (0, 1, ..., N-1), with strict validation ensuring sum of chunk plaintext sizes equals original logical file size. Duplicate chunk IDs or missing indices are rejected deterministically
- Reason: Guarantees that restore pipelines can reassemble the original file without relying on external directory or network order
- Date: 2026-10-07

## AD-014: Explicit Optional Encryption in Manifest Metadata
- Decision: Manifest metadata represents encryption as `Option<EncryptionMetadata>`. When encryption is disabled, the field is `None` with zero synthetic or dummy encryption parameters
- Reason: Enforces the optional encryption contract, ensuring unencrypted files do not carry misleading or redundant cryptographic headers
- Date: 2026-10-07
