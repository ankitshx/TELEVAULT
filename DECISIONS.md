# TeleVault Architecture & Design Decisions Log

This document records architectural decisions made throughout the project lifecycle. Small ambiguities choose the stated default and are logged here.

---

## Decision 001: Telethon MTProto User Session (Bot API Rejected)
- **Date:** Phase 0
- **Context:** Telegram Bot API enforces a 50 MB file upload ceiling and cannot create channels or perform server-side message forwarding.
- **Decision:** Use Telethon MTProto user sessions requiring user `api_id` and `api_hash`.
- **Status:** Approved.

## Decision 002: Dual-Channel Server-Side Mirroring
- **Date:** Phase 0
- **Context:** Need restore-if-deleted guarantee without doubling client upload bandwidth and time.
- **Decision:** Upload once to `TeleVault Primary` private channel, then issue a Telegram server-side `forward_messages` to `TeleVault Mirror`. Telegram duplicates the file pointer instantly on its servers.
- **Status:** Approved.

## Decision 003: Strict Append-Only in Telegram
- **Date:** Phase 0
- **Context:** Prevent software bugs or user misclicks from deleting cloud backup archives.
- **Decision:** Zero delete-from-Telegram functionality in the application. Invariant 2 test scans the AST of all source files to verify no code calls `delete_messages`. "Remove from list" only deletes the local SQLite index row.
- **Status:** Approved.

## Decision 004: Clean Architecture Port Protocol Isolation
- **Date:** Phase 0
- **Context:** GUI and CLI must be decoupled from Telethon, SQLite, and OS dependencies to enable fast in-memory testing and fake backends.
- **Decision:** Define `TelegramGateway`, `VaultRepository`, and `EventBus` as `@runtime_checkable` Python Protocols in `televault.domain.ports`. All use cases consume only these protocol interfaces.
- **Status:** Approved.

## Decision 005: Concurrency Default of 1
- **Date:** Phase 0
- **Context:** Parallel uploads on personal Telegram accounts dramatically increase the risk of triggering FloodWait bans.
- **Decision:** Set default upload concurrency to 1 file at a time, queueing additional files in the pipeline.
- **Status:** Approved.

## Decision 006: Self-Describing `tv1` JSON Captions
- **Date:** Phase 0
- **Context:** If the local SQLite database is deleted or corrupted, the user must be able to restore and reconstruct their vault from Telegram alone.
- **Decision:** Every document message carries a `tv1 {"id":"...","name":"...","sha256":"...","size":...}` header in its caption.
- **Status:** Approved.
