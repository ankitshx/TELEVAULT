# KNOWN FAILURES & RISK MITIGATIONS

## KF-001: Interrupted Multi-Chunk Upload Leading to Incomplete Manifest
- Risk: A multi-chunk upload (e.g. 5.2 GB file across 3 chunks) fails or is cancelled after chunk 0 and 1 upload, leaving chunk 2 unuploaded.
- Failure Mode: If a restore engine attempts to reconstruct the file from the incomplete manifest, it could write a partial/truncated file.
- Protection in Phase 9: Manifest pre-validation (`validate_manifest_for_restore`) strictly checks that NO chunk has `StorageReference::Pending`, verifies expected chunk counts against logical size, and asserts all chunk references are present and verified before any download commences. Incomplete manifests are rejected upfront with `RestoreError::PendingChunk` or `RestoreError::IncompleteBackup`.

## KF-002: Destination Collision Silently Overwriting User Files
- Risk: Restoring to an existing file path could overwrite newer local modifications if not explicitly confirmed.
- Failure Mode: Data loss of current local files.
- Protection in Phase 9: Explicit `CollisionPolicy` (`Overwrite`, `Skip`, `KeepBoth`). By default, collision resolution provides deterministic alternate file names (`file (1).ext`, `file (2).ext`) or skips restoration without modifying the target. `Overwrite` is only permitted when explicitly selected by the user.

## KF-003: BigInt Precision Truncation in TypeScript IPC
- Risk: Large 64-bit integers (`u64`, `usize`, `i64`) deserialized by `JSON.parse` in JavaScript environments can lose precision if exceeding `Number.MAX_SAFE_INTEGER` (2^53 - 1, ~9 Petabytes).
- Failure Mode: Corrupted file byte counts, chunk offsets, or timestamp values in React UI state.
- Protection in Phase 10: `specta-typescript` explicitly forbids unannotated BigInt exports. Byte counts and timestamps below 9 PB are annotated with `#[specta(type = specta_typescript::Number)]`, and limit parameters use `u32`, guaranteeing safe representation in JavaScript `number`.

## KF-004: Frontend Backend Architectural Drift (Localhost / Sidecar Regression)
- Risk: Architectural regression introducing a separate backend executable, sidecar daemon, Python runtime, or localhost HTTP server.
- Failure Mode: Startup failures, network port collisions, antivirus firewall blocks, packaging nightmare.
- Protection in Phase 10: Enforced via `apps/desktop/tests/architecture_invariants.rs` (4 tests) permanently verifying: zero localhost / 127.0.0.1 references in frontend and IPC bindings, zero sidecars in `tauri.conf.json`, zero Python files or scripts, and single-process desktop architecture.

## KF-005: Backup Storms After Long System Downtime
- Risk: When the desktop application starts after being offline for days or weeks, multiple scheduled intervals have elapsed. Replaying every missed execution would trigger a storm of back-to-back backups, exhausting bandwidth and locking SQLite.
- Mitigation in Phase 11: Bounded `MissedSchedulePolicy::RunOnce` policy. The scheduler executes at most ONE catch-up backup for an overdue schedule upon waking, and immediately advances `next_run_at` to the next scheduled interval in the future.

## KF-006: Concurrency Collisions Between Manual and Scheduled Backups
- Risk: A user manually triggers `start_backup` while a background scheduled backup for the same profile is already in flight, or the scheduler wakes up while a manual backup is executing.
- Mitigation in Phase 11: Managed `ExecutionGuard` per-profile lock registry. Concurrent manual triggers immediately return structured `IpcError::conflict("SCHEDULE_ALREADY_RUNNING")`, while overlapping scheduled ticks record a `Skipped` execution history entry and advance to the next run date without racing.

## KF-007: Accidental Remote Payload Deletion During Retention Pruning
- Risk: A retention policy engine attempting to free space calls storage deletion against Telegram messages, documents, or chunks.
- Failure Mode: Irrevocable destruction of the user's remote cloud backup data.
- Mitigation in Phase 12: Absolute Cloud-First Retention Rule. Pruning operations are strictly limited to local SQLite `snapshots` and `versions` metadata. The `RetentionEngine` contains ZERO calls to `StorageProvider`, `TelegramClient`, or remote delete methods. Remote Telegram storage remains completely immutable.

## KF-008: Premature Pruning of In-Progress Backup or In-Flight Restore Snapshots
- Risk: A retention pruning pass runs while a backup or restore is currently writing to or reading from a snapshot.
- Failure Mode: Dangling manifest pointers, partial snapshot corruption, or failed restores.
- Mitigation in Phase 12: Active snapshot protection and `ExecutionGuard` synchronization. The retention engine takes an `active_snapshots: &HashSet<SnapshotId>` set and checks `execution_guard().is_running(&pid)`. Any snapshot currently active or in-progress is unconditionally protected (`ProtectedActive`).

## KF-009: Pruning the Sole Recovery Point via Incomplete or Failed Snapshots
- Risk: If an old successful snapshot is expired by age, and the latest backup failed, pruning the old snapshot would leave the user with zero restorable snapshots.
- Failure Mode: Complete loss of restorable recovery points.
- Mitigation in Phase 12: Invariant protection for `keep_latest_successful` (preserving the most recent completed snapshot regardless of age) and `ProtectedSoleSnapshot` (preventing deletion of the only remaining snapshot for a profile).

## KF-010: Cross-Profile Repair Privilege Escalation
- Risk: A client requests repair for Profile B specifying an affected file or chunk belonging to Profile A, or uses an ambiguous/tampered manifest.
- Failure Mode: Corrupted cross-profile references, unauthorized file exposure, catalog inconsistency.
- Mitigation in Phase 14: Strict unbroken ownership validation (`RepairEligibilityChecker::validate_ownership_chain`). Validates `Profile -> Snapshot -> File -> Manifest -> Chunk -> Remote Reference`. Cross-profile references trigger immediate `RepairError::OwnershipViolation`.

## KF-011: Unbounded RAM Allocation during Multi-Gigabyte Chunk Repair
- Risk: Repairing a damaged chunk of a 5.2 GB file loads the whole 5.2 GB file or whole 1.8 GB chunk into memory for hashing/encryption.
- Failure Mode: Memory exhaustion, OOM crash, desktop UI freeze.
- Mitigation in Phase 14: Streaming single-chunk pipeline using bounded 64 KiB buffers (`REPAIR_STREAM_BUFFER_SIZE = 64 * 1024`). Seeks directly to the damaged chunk's offset, streams to a temporary staging file, hashes incrementally, and dispatches to `TransferEngine`. RAM overhead is capped under 50 MB regardless of file size.

## KF-012: Inadvertent Remote Telegram Object Deletion during Recovery
- Risk: A repair process attempting to clean up a damaged chunk calls Telegram deletion methods, causing data loss if the chunk was only temporarily unreachable or if recovery aborts midway.
- Failure Mode: Irrevocable loss of remote objects.
- Mitigation in Phase 14: Strict Remote Non-Deletion Invariant. Replacement chunks are uploaded to fresh Telegram references; old references remain intact in remote cloud storage.

## KF-013: Memory Bloat from Buffering Large File Payloads in React
- Risk: A future UI feature attempting to preview or display a 5.2 GB file by reading chunk contents over IPC into JavaScript state.
- Failure Mode: JavaScript heap exhaustion, browser tab crash, desktop UI freeze.
- Protection in Phase 15: Pure metadata-driven UI architecture. Restore, backup, verification, and repair commands only pass file paths and identifiers across IPC; the Rust engine streams data in 64 KiB chunks directly to disk or network without exposing raw payload bytes to React.

## KF-014: Frontend Stale State during Concurrent Long-Running Tasks
- Risk: Triggering backup, restore, verification, or repair while another operation is in flight on the same profile causing unhandled conflict errors.
- Failure Mode: Silent button stalls or unexpected error popups.
- Protection in Phase 15: Global status indicators, button disabling during in-flight operations, and robust handling of `SCHEDULE_ALREADY_RUNNING` and conflict errors with clear, actionable user messages.

## KF-015: Historical Manifest Overwrite Preventing Point-in-Time Restore
- Risk: In incremental backups where a tracked file is modified, generating a manifest with a fixed `manifest_id` (e.g., `man-{file_id}`) causes SQLite to overwrite the previous manifest record.
- Failure Mode: Attempting to restore a historical snapshot recovers the newest version of the file rather than the historical point-in-time version.
- Mitigation in Phase 16: Manifest IDs incorporate content hash prefixes (`man-{file_id}-{hash_prefix}`). Each unique version retains an immutable manifest row in SQLite, guaranteeing accurate historical recovery.

## KF-016: Orphaned Snapshots Trapped in BackingUp State on Crash or Cancellation
- Risk: If the application process crashes, is forcefully killed, or encounters an uncaught abort during backup, the snapshot record remains in `BackingUp` status indefinitely.
- Failure Mode: Snapshot permanently appears in-progress, blocking future backups and failing validation.
- Mitigation in Phase 16: Automatic failure wrapping in `execute_backup_inner` immediately transitions snapshot status to `Failed` upon any error or cancellation. Cold restart recovery detects incomplete in-flight snapshots and recovers gracefully.

## KF-017: Telegram Bot API Default File Size Limit Constraints
- Risk: Standard Telegram Bot API servers enforce a 50 MB limit for `sendDocument` and a 20 MB limit for `getFile` downloads, whereas TELEVAULT's multi-chunk architecture uses ~1.8 GB chunks.
- Failure Mode: Attempting multi-gigabyte chunk uploads through standard public Bot API endpoints returns HTTP 400 with `file is too large`.
- Mitigation in Phase 17: Architectural isolation via `TelegramStorageConfig` and progressive test sizes. Supporting standard 1.8 GB chunks in production requires configuring a local Telegram Bot API server (`api_endpoint`), which supports up to 2 GB documents, or connecting through Telegram MTProto transports in subsequent phases.
- Status: Documented and architected; live cloud testing accurately reported as NOT EXECUTED pending production environment setup.

## KF-018: Secret Credential Leakage in Diagnostics, Logs, or Frontend Inspection
- Risk: User's bot token or session string inadvertently printed in error messages, formatted via `println!`/`format!`, sent in IPC responses, or written to SQLite plaintext.
- Failure Mode: Bot token compromise, unauthorized channel access, credential leakage in bug reports or Git.
- Mitigation in Phase 17: `TelegramCredentials` redacts secrets in `Debug` and `Display` (`[REDACTED]`), persists solely to a git-ignored file (`telegram_credentials.json`), excludes secrets from Specta DTOs (`TelegramStatusDto` returns only `is_configured`, `target_chat_id`, and `bot_username`), and rejects plaintext storage in database tables.

## KF-019: Stale In-Flight Operations After Abnormal Process Termination
- Risk: Process kill, OS reboot, or power outage occurs during active chunk uploading or scanning, leaving snapshots in `BackingUp`/`Scanning` and transfer jobs in `Transferring`.
- Failure Mode: Subsequent launches perceive active jobs in flight, blocking scheduled backups or reporting false activity.
- Mitigation in Phase 18: Deterministic Startup Recovery (`reconcile_interrupted_operations`) automatically runs inside a transaction on `DesktopAppState::new`, identifying non-terminal active operations and marking them `Failed` with diagnostic reason annotations, while purging orphaned temporary staging files.

## KF-020: Truncated Configuration Files from Interrupted Writes
- Risk: System crash or power interruption while writing `televault.json` leaves zero-byte or truncated JSON on disk.
- Failure Mode: Application fails to start or defaults unexpectedly on subsequent launches.
- Mitigation in Phase 18: Configuration updates execute atomically by writing to `.tmp`, flushing to disk, and renaming to replace the destination file.

## KF-021: Stale Frontend Artifact Inlining during Desktop Compilation
- Risk: Developer or build script modifies React code in `apps/desktop/ui/src` but runs `cargo build --release` without running `npm run build`.
- Failure Mode: Tauri embeds stale assets from `ui/dist`, causing the application to display obsolete UI components.
- Mitigation in Phase 20 P0: Configured `beforeBuildCommand: "npm --prefix ui run build"` in `tauri.conf.json` and established `scripts/build_release.ps1` to enforce compilation of frontend assets prior to executable assembly.

## KF-022: Windows Console Host Window Spawning on GUI Application Launch
- Risk: Omitting `windows_subsystem = "windows"` causes Rust to link Windows binaries with `/SUBSYSTEM:CONSOLE`.
- Failure Mode: A command prompt / CMD window appears whenever the desktop application is opened.
- Mitigation in Phase 20 P0: Declared `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` in `main.rs`, ensuring the release binary compiles as PE Subsystem 2 (`IMAGE_SUBSYSTEM_WINDOWS_GUI`).

## KF-023: Startup Authentication Status Race Condition & Dashboard Flash
- Risk: React `authStatus` state defaults to `null`, allowing an unguarded conditional check to fall through and mount the protected dashboard before asynchronous IPC returns.
- Failure Mode: Unauthenticated user sees a momentary flash of the protected dashboard or remains on the dashboard if IPC query fails.
- Mitigation in Phase 20 P1: Implemented explicit `isAuthInitializing` state rendering a branded `.app-splash` screen, blocking protected routes until authentication status is confirmed. If initialization fails, renders an error screen with retry rather than falling through.

## KF-024: Production Mock Driver & Empty Datacenter Session Preventing Code Delivery
- Risk: Production `DesktopAppState` initialized with `MtprotoAuthManager::new_mock`, and `GrammersMtprotoDriver` initialized with an empty session lacking datacenter definitions.
- Failure Mode: Clicking "Send Code" returned success locally without contacting Telegram or, if attempted live, failed immediately with `InvalidDc`.
- Mitigation: Production initialization defaults to `new_live`, populating standard Telegram datacenters (DC1-5) in `grammers-session`, dynamically delegating to mock only when `TELEVAULT_MOCK_TELEGRAM=1` or test credentials are provided, providing user-configurable API credentials with hash redaction, and accurately guiding users regarding Telegram's in-app vs SMS code delivery.

