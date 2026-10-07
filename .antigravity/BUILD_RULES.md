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

