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

## AD-015: Embedded SQLite Catalog and Refinery Versioned Migrations
- Decision: Local persistence is handled by an embedded SQLite database managed via `rusqlite` and `refinery`. Schema migrations are stored as versioned SQL files (`V1__initial_schema.sql`) and run deterministically upon database initialization
- Reason: Eliminates external database services, network listeners, and schema drift while enabling robust transactional upgrades
- Date: 2026-10-07

## AD-016: Strict Foreign Key Referential Integrity and Protected Historical Retention
- Decision: `PRAGMA foreign_keys = ON;` is enforced unconditionally. Historical snapshots and version associations use `ON DELETE RESTRICT` rather than cascading deletes, preventing accidental destruction of backup history when editing profiles or files
- Reason: Guarantees catalog consistency and protects point-in-time restore history from inadvertent deletion
- Date: 2026-10-07

## AD-017: Logical File Primacy and Physical Chunk Storage Transparency in SQLite
- Decision: The `files` table stores single logical file entities (e.g., 5.2 GB), while the `chunks` table stores physical storage units (e.g., three 1.8 GB chunks). Chunks are never exposed as files, and queries always maintain `(file_id, chunk_index) UNIQUE` with strict index ordering
- Reason: Enforces product rule that physical chunking is an internal transport implementation detail and keeps user file tracking unified
- Date: 2026-10-07

## AD-018: Trigger-Synchronized FTS5 Full-Text Search Engine
- Decision: Full-text search is implemented using SQLite's FTS5 extension (`files_fts`), synchronized automatically via `AFTER INSERT`, `AFTER UPDATE`, and `AFTER DELETE` database triggers on the `files` table. Search queries are sanitized to prevent FTS5 syntax errors
- Reason: Delivers instant, typo-tolerant search across logical filenames and relative paths without manual application-layer synchronization logic
- Date: 2026-10-07

## AD-019: SQLite Pragmas for High Performance and Concurrency
- Decision: Database connections configure `PRAGMA journal_mode = WAL;`, `PRAGMA synchronous = NORMAL;`, `PRAGMA foreign_keys = ON;`, and `PRAGMA busy_timeout = 5000;`. The database handle is wrapped in `Arc<Mutex<Connection>>` for safe sharing across concurrent desktop workers
- Reason: Enables high write concurrency, fast commits, resilient crash recovery, and thread-safe desktop application access
- Date: 2026-10-07

## AD-020: Cloud-First Architecture and Telegram Cloud Authoritative Store
- Decision: TELEVAULT is strictly a cloud-first backup application. Telegram Cloud is the authoritative and permanent repository for all backup payloads. The local system retains only SQLite metadata, small caches, bounded staging files, and diagnostics. No permanent backup copies are maintained on the local computer
- Reason: Enforces the product identity as an encrypted cloud backup solution and prevents disk space exhaustion on user devices
- Date: 2026-10-07

## AD-021: Local Temporary Storage Policy and Staging RAII Lifecycle Guarantee
- Decision: Temporary staging files reside strictly within TELEVAULT's managed temp area (`PathManager::temp_dir()`). Staging files are wrapped in `TempPayloadFile`, which implements RAII cleanup via `Drop` to ensure any aborted or failed operation immediately deletes unfinalized payload data from disk. Successful uploads clean up staging files immediately upon remote verification
- Reason: Guarantees bounded local disk usage and prevents accumulation of orphaned payload data across process crashes or worker restarts
- Date: 2026-10-07

## AD-022: Streaming-First I/O and Bounded 64 KiB Buffers
- Decision: All storage interfaces operate over `&mut dyn Read` and `&mut dyn Write` streaming contracts utilizing bounded stack buffers (`STREAM_CHUNK_BUFFER_SIZE = 65,536` bytes). Allocating whole-file `Vec<u8>` buffers for multi-gigabyte files or chunks is strictly prohibited across all storage components and test harnesses
- Reason: Binds application memory footprint to constant, predictable usage regardless of user backup file sizes (e.g., 5 GB, 50 GB, 100 GB)
- Date: 2026-10-07

## AD-023: 1.8 GB Logical Chunk Size Decoupled From Memory Allocation
- Decision: 1.8 GB (1,887,436,800 bytes) represents a logical chunk partitioning threshold for Telegram compatibility, NOT a memory allocation unit. Chunks are generated, hashed, encrypted, and transferred as streaming flows without ever materializing 1.8 GB in RAM
- Reason: Enforces TELEVAULT's permanent resource-efficiency architecture on desktop environments
- Date: 2026-10-07

## AD-024: Decoupled StorageProvider Abstraction Boundary
- Decision: Core domains interact exclusively with the generic `StorageProvider` trait (`crates/televault-storage`). Telegram-specific types (`TelegramReference`, `TelegramTransport`, Telegram API structures) are encapsulated inside `crates/televault-telegram`, preventing Telegram API leakage into `televault-core` or `televault-backup`
- Reason: Maintains modular architectural separation of concerns, simplifies unit testing via mock providers, and allows potential alternate cloud storage adapters in the future
- Date: 2026-10-07

## AD-025: Structured Telegram Chunk Header Tagging in Message Captions
- Decision: Telegram documents are tagged with structured, verifiable text captions formatted as `TELEVAULT:v=1:fid=<file_id>:cid=<chunk_id>:idx=<idx>:tot=<total>:sz=<bytes>`. Remote references are stored as `(chat_id, message_id, file_id)` tuples, allowing stateless out-of-band catalog recovery and verification directly from Telegram chat history
- Reason: Enables catastrophic disaster recovery and independent validation of remote backup objects without relying on raw URLs or secret leakage
- Date: 2026-10-07

## AD-026: Transfer Engine Decoupled from Remote Storage Protocols
- Decision: `TransferEngine`, `UploadWorker`, and `DownloadWorker` interact strictly through the generic `StorageProvider` trait. Zero Telegram APIs or provider-specific details are imported or referenced in `televault-transfer`
- Reason: Enforces modular design, eliminates hardcoded dependencies on Telegram, and allows simple mock-driven testing and future multi-cloud extensibility
- Date: 2026-10-07

## AD-027: Bounded Transfer Concurrency and Demand-Driven Workers
- Decision: Transfer execution concurrency is strictly constrained by `max_concurrent_transfers` (default 2), managed by `TransferQueue` using atomic capacity tracking. Workers and queues are completely idle with zero CPU or network polling when no jobs are active
- Reason: Complies with the Permanent Resource Rule to minimize RAM and CPU, preventing thread explosions, memory spikes, and Telegram rate-limit penalties
- Date: 2026-10-07

## AD-028: Throttled Transfer Progress Notification Model
- Decision: Streaming progress updates are coalesced and emitted only when the accumulated bytes cross a 256 KiB threshold or on explicit lifecycle state transitions (e.g., Transferring, Verifying, Completed, Failed)
- Reason: Prevents event spamming, high CPU utilization, and IPC flooding when transferring large multi-gigabyte files
- Date: 2026-10-07

## AD-029: Idempotent Retries with Exponential Backoff
- Decision: Transient network or storage errors trigger retries governed by `RetryPolicy` with bounded exponential backoff. Retries strictly preserve deterministic `FileId`, `ChunkId`, `chunk_index`, and `total_chunks`. Terminal failures (exhausted retries, cancellation, invalid requests) do not enter retry loops
- Reason: Prevents retry storms while ensuring disaster-recovery manifests and chunks remain idempotent and consistent across retries
- Date: 2026-10-07

## AD-030: Single Embedded Database Synchronization
- Decision: Transfer job execution state maps directly to the existing `transfer_jobs` table in `televault-db` using `upsert_transfer_job`. No secondary database or duplicate job tracking system is created
- Reason: Prevents catalog fragmentation and maintains SQLite as the single local source of truth for job history
- Date: 2026-10-07

## AD-031: Metadata-First Change Detection Strategy
- Decision: Routine incremental backup scans inspect file metadata (size and modification timestamp) against previously recorded catalog versions to identify changes. Files with matching size and timestamp are marked `Unchanged` without reading disk contents or hashing
- Reason: Enforces high scan speed, minimizes idle CPU and disk I/O, and complies with the permanent resource-efficiency architecture
- Date: 2026-10-07

## AD-032: Deterministic Logical File Identity Across Snapshots
- Decision: Logical file identity (`FileId`) is computed deterministically from the normalized relative path (`sha256(relative_path)[..16]`) upon initial discovery and preserved across subsequent snapshots and modifications
- Reason: Ensures stable versioning history, predictable catalog queries, and clean snapshot differential comparisons without relying on filesystem enumeration order or random IDs
- Date: 2026-10-07

## AD-033: Cloud-First Retention Invariant on Deleted Local Files
- Decision: When a tracked file is deleted from local disk, the backup engine marks the file status as `deleted` in snapshot metadata, but NEVER automatically deletes or prunes the remote backup payload in Telegram Cloud. Retention policies decide payload cleanup during dedicated retention maintenance
- Reason: Prevents accidental data loss, maintains point-in-time snapshot recovery, and protects against malicious local file destruction
- Date: 2026-10-07

## AD-034: Two-Stage Manifest Storage Reference Lifecycle
- Decision: During initial payload staging, chunk manifests use `StorageReference::Pending` so that `ManifestV1` satisfies schema validation. Upon completion of each staged upload via `execute_staged_upload`, the chunk is updated with the verified remote reference (`StorageReference::Telegram` or `LocalStaging`) and saved to SQLite
- Reason: Satisfies strict database foreign key relationships and schema validation rules at all stages without dummy or zeroed identifiers
- Date: 2026-10-07

## AD-035: Staged Upload Handoff to Phase 7 TransferEngine
- Decision: The backup engine plans and coordinates, but delegates all payload transfers to the Phase 7 `TransferEngine` through `execute_staged_upload`. Temporary staged chunk files are unlinked immediately after upload verification via explicit cleanup, with RAII `Drop` fallback on cancellation or error
- Reason: Eliminates duplicate upload logic, enforces bounded concurrency, and guarantees zero orphaned staging files
- Date: 2026-10-07

## AD-036: Temporary Staging and Verification Prior to Destination Finalization
- Decision: Restored file streams are decrypted, decompressed, and written into a managed temporary staging file (`TempPayloadFile`) where whole-file SHA-256 and byte counts are strictly validated. Only after full verification succeeds is the temporary file atomically committed (via `fs::rename`) to the resolved destination.
- Reason: Prevents corrupting, truncating, or leaving partial restored files at the user destination if network transfer, decryption, or checksum checks fail mid-operation.
- Date: 2026-10-07

## AD-037: Deterministic Collision Resolution Policies
- Decision: Explicit collision policies (`Overwrite`, `Skip`, `KeepBoth`) govern existing target files. `KeepBoth` deterministically resolves alternate names by incrementing safe suffixes (`filename (1).ext`, `filename (2).ext`) without relying on filesystem directory enumeration order.
- Reason: Eliminates nondeterminism, avoids race conditions, and guarantees user files are never overwritten unintentionally.
- Date: 2026-10-07

## AD-038: Incomplete Backup Pre-Validation Rejection
- Decision: Manifests are thoroughly validated prior to initiating payload downloads. Manifests containing `StorageReference::Pending`, missing chunks, invalid chunk counts, non-contiguous chunk indices, or unsupported versions are rejected upfront with typed `RestoreError` before downloading any chunk bytes.
- Reason: Saves network bandwidth, eliminates partial restore states, and prevents reconstructing incomplete or interrupted multi-chunk backups.
- Date: 2026-10-07

## AD-039: Strict Chunk Ordering by Manifest Index
- Decision: Logical file reconstruction always sorts and streams chunks strictly in ascending `ChunkManifest.index` order (0, 1, ..., N-1), completely decoupling reconstruction from Telegram message ID sequence, network response ordering, or database record order.
- Reason: Guarantees deterministic, bit-for-bit file reassembly regardless of out-of-order remote arrivals or asynchronous transfer completions.
- Date: 2026-10-07

## AD-040: Bounded 64 KiB Reverse Streaming Pipeline
- Decision: The restore pipeline is memory-bounded throughout: remote chunks stream in 64 KiB buffers through `TransferEngine::download_stream`, decrypt through authenticated `ChunkAad`, decompress, and write to destination staging. The system never loads a 1.8 GB chunk or whole 5.2 GB logical file into RAM.
- Reason: Guarantees fixed, lightweight memory footprint on user systems even when recovering multi-gigabyte media files.
- Date: 2026-10-07

## AD-041: Remote Backup Data Immutability During Restore
- Decision: Restore operations are strictly read-only against remote storage. Chunks, messages, versions, and manifests in Telegram Cloud and SQLite are never deleted, pruned, or modified by restore.
- Reason: Preserves backup integrity and retention isolation; disaster recovery must never jeopardize remote cloud copies.
- Date: 2026-10-07

## AD-042: Tauri 2 Typed IPC Command Surface Architecture
- Decision: Expose backend domain operations exclusively through Tauri 2 IPC commands registered via `tauri-specta` v2. The frontend communicates with Rust using generated TypeScript bindings (`bindings.ts`) with zero localhost, HTTP, REST, or web sockets.
- Reason: Eliminates cross-origin issues, network port collisions, background process synchronization, and guarantees type safety between React and Rust.
- Date: 2026-10-07

## AD-043: DesktopAppState Managed Lifecycle and Shared Single-Process Services
- Decision: All core Rust domain engines (`Database`, `TransferEngine`, `BackupEngine`, `RestoreEngine`, `BackupChecker`, `PathManager`) reside within a single `DesktopAppState` managed by Tauri's state container (`tauri::State<'_, DesktopAppState>`).
- Reason: Avoids global mutable static variables, eliminates redundant database connections or engine instantiation per command, and guarantees safe concurrent execution across Tauri IPC threads.
- Date: 2026-10-07

## AD-044: Offloaded Async Execution for Heavy Commands
- Decision: CPU-intensive or IO-heavy operations (backup execution, file/manifest/snapshot restore, trial verification) are wrapped in `tokio::task::spawn_blocking` within Tauri async commands rather than running synchronously on the IPC handler thread.
- Reason: Keeps the Tauri main event loop and IPC dispatcher responsive to user interactions and progress/status queries without blocking the UI thread.
- Date: 2026-10-07

## AD-045: Structured IpcError with Machine-Readable Error Codes
- Decision: IPC commands return a structured `Result<T, IpcError>` where `IpcError` contains machine-readable `code`, human-readable sanitized `message`, and optional `details`. Sensitive secrets, internal database paths, and Rust backtraces are strictly redacted.
- Reason: Allows React frontend to branch deterministically on structured error codes (`VALIDATION_ERROR`, `NOT_FOUND`, `CONFLICT`, etc.) rather than parsing localized English error strings.
- Date: 2026-10-07

## AD-046: Cooperative Cancellation Registry via CancellationTokens
- Decision: Operations supporting cancellation register their `CancellationToken` in `DesktopAppState::cancellation_tokens` keyed by operation identifier (`JobId` or `ProfileId`). The frontend issues `cancel_operation`, which triggers token cancellation across the underlying transfer and backup pipelines.
- Reason: Connects frontend abort actions to the existing Phase 7/8/9 cancellation mechanism without creating an unrelated abort system.
- Date: 2026-10-07

## AD-047: Database Migration V2 for Durable Scheduler State and Execution History
- Decision: Implemented `V2__schedules.sql` in `crates/televault-db/migrations/` defining `schedules` and `schedule_history` tables. The `schedules` table includes `schedule_id`, `profile_id`, `schedule_type`, `expression`, `timezone_strategy`, `enabled`, `missed_policy`, `next_run_at`, `last_run_at`, `last_status`, `last_error_code`, `created_at`, `updated_at`, with `FOREIGN KEY (profile_id) REFERENCES profiles(profile_id) ON DELETE CASCADE`. `schedule_history` stores `history_id`, `schedule_id`, `profile_id`, `started_at`, `completed_at`, `status`, `snapshot_id`, `files_processed`, `bytes_uploaded`, `error_code`, `error_message`.
- Reason: Guarantees schedule configurations and execution logs survive desktop application restarts and prevent orphan schedule records when profiles are deleted.
- Date: 2026-10-07

## AD-048: Mutual Exclusion via Shared ExecutionGuard for Scheduled and Manual Backups
- Decision: Created `ExecutionGuard` in `crates/televault-scheduler/src/guard.rs` maintaining a per-profile lock registry (`Arc<Mutex<HashSet<ProfileId>>>`). Both scheduled backup executions in `SchedulerService` and manual backup invocations via `start_backup` in `apps/desktop/src/commands/backup.rs` must acquire a RAII `ProfileGuard`. If an execution is in progress for Profile A, manual triggers fail immediately with `IpcError::conflict("SCHEDULE_ALREADY_RUNNING")`, and scheduled ticks are safely coalesced/skipped with status `Skipped` and rescheduled.
- Reason: Guarantees no two backup processes ever run concurrently on the same profile, preventing SQLite lock contention, duplicate Telegram chunk uploads, or manifest race conditions.
- Date: 2026-10-07

## AD-049: Event-Driven, Non-Busy Scheduler Loop with Zero Idle CPU
- Decision: Implemented `SchedulerService::run_loop` using `tokio::select!` over an asynchronous sleep duration (`calculate_sleep_duration` targeting the earliest `next_run_at`), a wakeup notification (`Arc<tokio::sync::Notify>`), and a cancellation token (`CancellationToken`). The loop sleeps completely until the next scheduled event or until schedule creation, update, deletion, or shutdown wakes it immediately.
- Reason: Eliminates polling and busy loops, ensuring idle CPU utilization remains strictly 0% on Windows desktop environments.
- Date: 2026-10-07

## AD-050: Bounded Single Catch-Up Policy for Missed Runs
- Decision: When TELEVAULT starts up or wakes after dormancy, schedules whose `next_run_at` elapsed in the past are evaluated with `MissedSchedulePolicy::RunOnce` by default. At most one catch-up execution is triggered, immediately after which `next_run_at` advances to the next future recurrence.
- Reason: Prevents backup storms and cascading executions if the user's machine was powered off for weeks.
- Date: 2026-10-07

## AD-051: Testable Clock Abstraction for Deterministic Scheduling Testing
- Decision: Introduced `Clock` trait in `crates/televault-scheduler/src/clock.rs` implemented by `SystemClock` for production and `MockClock` (`Arc<RwLock<DateTime<Utc>>>`) for testing. Tests can advance simulated time by arbitrary intervals (hours, days, DST shifts) and verify next-run calculations and recovery instantly without wall-clock sleeps.
- Reason: Guarantees fast, robust, and deterministic tests while preserving real system time in production.
- Date: 2026-10-07





