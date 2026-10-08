# BUILD RULES

1. No hardcoded developer paths
2. All filesystem access via PathManager
3. Static CRT mandatory (RUSTFLAGS="-C target-feature=+crt-static")
4. WebView2 bundling mandatory (embedBootstrapper)
5. Typed IPC only (tauri-specta)
6. No secrets in logs
7. Every bug = root cause + fix + regression test + prevention rule
8. Local DB is cache; Telegram is source of truth
9. Backup complete only when hash verified
10. Feature done only when test + error handling + logging + docs
11. Cloud-First Rule: Actual backup payloads reside permanently in Telegram Cloud; local computer stores metadata/cache/temp staging only
12. Permanent Resource Rule: Minimize idle RAM, CPU, disk, network, processes, and dependencies
13. Streaming Rule: Zero whole-file memory buffering; all payload transfers use bounded 64 KiB streams
14. Temporary Staging Rule: Temporary payload files must have explicit RAII lifecycle cleanup (`Drop`), bounded directories, and stale purge
15. Bounded Concurrency Rule: Never spawn unbounded threads or tasks per transfer; always throttle via bounded queue (`max_concurrent_transfers`)
16. Throttled Progress Rule: Never emit progress events on every stream buffer read; throttle progress notifications to avoid IPC/CPU flooding
17. Single Catalog Rule: Transfer jobs must persist only to the existing embedded `transfer_jobs` database table
18. Metadata-First Scan Rule: Never hash entire files during routine scans; use size and modification timestamp to detect unchanged files and reuse prior manifests/versions
19. Remote Retention Invariant Rule: Never delete remote backups when a file is deleted locally; record deletion logically in snapshot metadata and delegate retention to the retention policy engine
20. No Premature Component Rule: Do not implement background recurring backup scheduling (Phase 11) or React UI before their designated phases
21. Verify Before Final Destination Rule: Never write unverified, partially decrypted, or partially downloaded data to the user destination path; stage in temporary storage, verify whole-file hash and size, and atomically move
22. Remote Data Read-Only Restore Rule: Restore operations are strictly read-only against Telegram Cloud storage; remote chunks, messages, and historical manifests must never be mutated or deleted by restore
23. Deterministic Collision Resolution Rule: `KeepBoth` must deterministically inspect destination presence and generate `(1)`, `(2)` suffixes without relying on directory iteration order
24. IPC Payload Size Rule: Never transfer whole-file byte arrays or multi-gigabyte data across Tauri IPC; IPC payloads carry metadata, commands, and status only
25. Single Desktop Process Invariant: The desktop application consists of exactly one process; zero localhost communication, zero HTTP servers, zero Python runtimes, zero backend sidecars
26. IPC Untrusted Boundary Rule: Treat every frontend IPC request as untrusted; validate paths, IDs, enums, and destination directories using existing `PathManager` and domain validators before execution
27. Secret Redaction Invariant: Never return API secrets, Telegram credentials, session secrets, or encryption keys in IPC DTOs or logs
28. Zero Busy Loops for Background Daemons: Never poll in tight or frequent loops (`while true { check every ms }`); always sleep until the earliest event (`tokio::select!`) with wakeups via `Notify` or cancellation tokens.
29. Per-Profile Backup Mutual Exclusion: Never run concurrent backups on the same profile; always synchronize manual and scheduled executions using `ExecutionGuard`.
30. Single Backup Engine Rule: The scheduler must never implement its own backup logic; it must orchestrate the domain `BackupEngine`.
31. Cloud-First Retention Rule: Local retention controls local SQLite snapshot and version metadata only; never delete remote Telegram messages, documents, or chunks during retention pruning. Remote cloud backups remain immutable.
32. Snapshot Retention Safety Rule: Retention pruning must never delete active in-progress snapshots, snapshots from unrelated profiles, or the sole recovery point for a profile.
33. Dry-Run Zero-Mutation Rule: Retention preview/dry-run must be strictly non-destructive; it must never perform database mutations or remote network calls.
34. Transaction-Safe Metadata Pruning Rule: Snapshot and version metadata pruning must execute inside an atomic SQLite transaction and maintain referential integrity.
35. Continuous GitHub Checkpoint Rule: Every completed TELEVAULT phase MUST be committed locally, validated, and pushed to the official GitHub repository before the phase is considered complete. Every future checkpoint MUST contain the complete cumulative project state from the earliest phase through the current phase.
    Workflow for EVERY future phase:
    1. Start from latest approved GitHub checkpoint (`origin/main`).
    2. Implement only the current phase (never start later phases prematurely).
    3. Run complete test suite (`cargo test --workspace`).
    4. Run formatting and lint checks (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`).
    5. Update README.md with current cumulative architecture and completed phases.
    6. Update .antigravity/ records (PROGRESS_LOG, MISTAKES, ARCHITECTURE_DECISIONS, BUILD_RULES, KNOWN_FAILURES).
    7. Review security and architecture invariants (zero secrets, zero Telegram credentials, zero unverified payloads, zero localhost, zero sidecars).
    8. Commit the completed phase.
    9. Push to origin/main (https://github.com/ankitshx/TELEVAULT.git).
    10. Verify local == origin/main.
    11. Stop and report.
36. Remote Repair Safety & Non-Deletion Rule: Remote repair must be opt-in, verification-driven, and strictly ownership-isolated (Profile -> Snapshot -> File -> Manifest -> Chunk -> Remote Reference). Damaged chunks are reconstructed and uploaded to fresh remote references without mutating or deleting old remote references. Remote deletion is deferred to an explicitly authorized future phase.
37. Large-File Single-Chunk Repair Rule: For multi-chunk logical files (e.g. 5.2 GB across 3 chunks), repair must reconstruct and upload ONLY the affected chunk(s) using bounded 64 KiB streaming buffers. RAM allocations exceeding streaming boundaries are strictly prohibited.
38. Windows Node.js & npm.cmd Execution Rule: On Windows environments, ensure the system Node.js directory is explicitly prepended to `$env:PATH` when running scripts and invoke `npm.cmd` directly to guarantee deterministic execution across PowerShell sessions.
39. Bounded React State Invariant: The desktop React frontend must never buffer file contents, backup payload streams, or raw chunk binary data. All state binding is strictly restricted to typed DTO metadata, progress notifications, and bounded list models.
40. End-to-End Reliability & Invariant Preservation Rule: Any change to backup, verification, repair, restore, or retention must validate end-to-end reliability across cold restarts on disk-backed SQLite databases (`PRAGMA integrity_check` & `PRAGMA foreign_key_check`), multi-generation incremental point-in-time recovery, bounded 64 KiB streaming, cooperative cancellation state preservation, and zero remote Telegram deletions.
41. Production Telegram Cloud Storage Integration Rule: Real Telegram uploads and downloads must strictly use native Rust `reqwest` transport inside the single desktop process, stream through bounded 64 KiB buffers with automatic RAII temporary file cleanup, authenticate via secure git-ignored `telegram_credentials.json`, redact secret tokens in all logs/diagnostics/UI/DTOs, and gracefully route through `DynamicStorageProvider` without restarting the application.
42. Deterministic Startup Recovery & State Machine Reconciliation Rule: Application boot must unconditionally execute SQLite PRAGMA integrity/foreign-key checks, transition crashed active operations (snapshots in BackingUp/Scanning, transfers in Transferring) to Failed with descriptive diagnostic reasons inside an atomic transaction, sweep orphaned staging files, and expose diagnostic telemetry via `get_startup_recovery_report`.
43. Configuration Atomicity Rule: Updates to `AppConfig` and `TelegramCredentials` must write to a `.tmp` file, flush/sync, and atomically rename over the target configuration file to prevent partial or corrupt configuration state under power interruptions.


