# TeleVault

> Manual-only, append-only resilient desktop backup vault powered by Telegram MTProto user sessions.

TeleVault is a local desktop application designed for Windows 11. It provides honest, redundant, and tamper-evident backup of your files into private Telegram channels with dual-write mirroring, auto-healing, and zero third-party telemetry.

---

## Key Principles

- **One File = One Telegram Document:** Uploaded directly with `force_document=True`, maintaining original filename and bytes.
- **Manual-Only, Append-Only:** TeleVault does nothing without explicit user consent. No auto-sync, no background watched folders. TeleVault never deletes anything from Telegram.
- **Dual-Write Mirroring:** Every file is uploaded once to `TeleVault Primary` and immediately server-side forwarded to `TeleVault Mirror` without extra bandwidth consumption.
- **Integrity Guarantee:** Pre-upload and post-restore SHA-256 verification. A restore is reported successful only if the byte hash matches.
- **Self-Describing Captions:** Every upload carries a `tv1` JSON header, allowing full index rebuild even if local storage is wiped.

---

## Project Structure & Architecture

Built with **Clean Architecture** (Python 3.12+, Telethon MTProto, SQLite, PyQt6):
- `televault/domain/`: Pure business rules, states, entities, and protocol ports.
- `televault/application/`: Backup, Restore, Verify, Heal, Rebuild use cases.
- `televault/infrastructure/`: Telethon gateway, SQLite repository, Windows keyring, encryption.
- `televault/presentation/`: Dark schematic PyQt6 desktop UI and CLI commands.

---

## Running Invariant Tests

```bash
pytest tests/test_invariants.py -v
```
All invariant tests run against in-memory fake backends in milliseconds.
