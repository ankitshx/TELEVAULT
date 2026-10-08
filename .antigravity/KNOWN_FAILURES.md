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
