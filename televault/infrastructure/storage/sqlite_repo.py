from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import sqlite3
from typing import Any, Iterator

from televault.domain.entities import (
    AuditEvent,
    BackupReceipt,
    FilePart,
    FileRecord,
    ManifestRecord,
    MessageRef,
    TransactionRecord,
)
from televault.domain.ports import (
    AuditLedger,
    ManifestRepository,
    TransactionJournal,
    VaultRepository,
)
from televault.domain.states import (
    HealthState,
    LocalFileStatus,
    TransactionState,
    VaultMode,
)

GENESIS_HASH = "0" * 64


class SQLiteVaultRepository(
    VaultRepository,
    TransactionJournal,
    ManifestRepository,
    AuditLedger,
):
    """Integrated SQLite-backed persistence store implementing:
    - VaultRepository (file records & metadata index)
    - TransactionJournal (crash-safe persistent backup transactions)
    - ManifestRepository (tamper-evident hash-chained manifests)
    - AuditLedger (append-only hash-chained audit events)
    """

    def __init__(self, db_path: Path):
        self.db_path = db_path
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._init_db()

    @contextmanager
    def _connection(self) -> Iterator[sqlite3.Connection]:
        conn = sqlite3.connect(str(self.db_path))
        conn.row_factory = sqlite3.Row
        try:
            yield conn
            conn.commit()
        finally:
            conn.close()

    def _init_db(self) -> None:
        """Run schema migrations in sorted order and ensure backwards compatibility."""
        migrations_dir = Path(__file__).parent / "migrations"
        migration_files = sorted(migrations_dir.glob("*.sql"))
        with self._connection() as conn:
            for mf in migration_files:
                conn.executescript(mf.read_text(encoding="utf-8"))
            # Ensure parts column exists for existing databases
            cols = [row[1] for row in conn.execute("PRAGMA table_info(file_records)").fetchall()]
            if "parts" not in cols:
                conn.execute("ALTER TABLE file_records ADD COLUMN parts TEXT NOT NULL DEFAULT '[]'")

    # ---------------------------------------------------------
    # VaultRepository Implementation
    # ---------------------------------------------------------
    def add(self, rec: FileRecord) -> None:
        sql = """
        INSERT INTO file_records (
            id, name, original_path, sha256, size, mtime, state,
            primary_channel_id, primary_message_id,
            mirror_channel_id, mirror_message_id,
            local_status, version, mode, tags, recovery_path, parts,
            created_at, updated_at
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?,
            ?, ?,
            ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?
        )
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            original_path = excluded.original_path,
            sha256 = excluded.sha256,
            size = excluded.size,
            mtime = excluded.mtime,
            state = excluded.state,
            primary_channel_id = excluded.primary_channel_id,
            primary_message_id = excluded.primary_message_id,
            mirror_channel_id = excluded.mirror_channel_id,
            mirror_message_id = excluded.mirror_message_id,
            local_status = excluded.local_status,
            version = excluded.version,
            mode = excluded.mode,
            tags = excluded.tags,
            recovery_path = excluded.recovery_path,
            parts = excluded.parts,
            updated_at = excluded.updated_at
        """
        now = datetime.now(timezone.utc).isoformat()
        serialized_parts = json.dumps([
            {
                "part_number": p.part_number,
                "total_parts": p.total_parts,
                "size": p.size,
                "sha256": p.sha256,
                "primary_channel_id": p.primary_ref.channel_id if p.primary_ref else None,
                "primary_message_id": p.primary_ref.message_id if p.primary_ref else None,
                "mirror_channel_id": p.mirror_ref.channel_id if p.mirror_ref else None,
                "mirror_message_id": p.mirror_ref.message_id if p.mirror_ref else None,
            }
            for p in rec.parts
        ])
        with self._connection() as conn:
            conn.execute(
                sql,
                (
                    rec.id,
                    rec.name,
                    rec.original_path,
                    rec.sha256,
                    rec.size,
                    rec.mtime,
                    rec.state.value,
                    rec.primary_ref.channel_id if rec.primary_ref else None,
                    rec.primary_ref.message_id if rec.primary_ref else None,
                    rec.mirror_ref.channel_id if rec.mirror_ref else None,
                    rec.mirror_ref.message_id if rec.mirror_ref else None,
                    rec.local_status.value,
                    rec.version,
                    rec.mode.value,
                    json.dumps(rec.tags),
                    rec.recovery_path,
                    serialized_parts,
                    rec.created_at.isoformat() if rec.created_at else now,
                    now,
                ),
            )

    def get(self, record_id: str) -> FileRecord:
        sql = "SELECT * FROM file_records WHERE id = ?"
        with self._connection() as conn:
            row = conn.execute(sql, (record_id,)).fetchone()
            if not row:
                raise KeyError(f"FileRecord not found: {record_id}")
            return self._row_to_record(row)

    def find_by_hash(self, sha256: str) -> FileRecord | None:
        sql = "SELECT * FROM file_records WHERE sha256 = ?"
        with self._connection() as conn:
            row = conn.execute(sql, (sha256,)).fetchone()
            return self._row_to_record(row) if row else None

    def list_all(self) -> list[FileRecord]:
        sql = "SELECT * FROM file_records ORDER BY created_at DESC"
        with self._connection() as conn:
            rows = conn.execute(sql).fetchall()
            return [self._row_to_record(r) for r in rows]

    def update_state(self, entity_id: str, state: HealthState | TransactionState, **refs: Any) -> None:
        if isinstance(state, TransactionState):
            self.update_transaction_state(entity_id, state, **refs)
            return

        p_ref = refs.get("primary_ref")
        m_ref = refs.get("mirror_ref")
        parts = refs.get("parts")
        now = datetime.now(timezone.utc).isoformat()

        serialized_parts = None
        if parts is not None:
            serialized_parts = json.dumps([
                {
                    "part_number": p.part_number,
                    "total_parts": p.total_parts,
                    "size": p.size,
                    "sha256": p.sha256,
                    "primary_channel_id": p.primary_ref.channel_id if p.primary_ref else None,
                    "primary_message_id": p.primary_ref.message_id if p.primary_ref else None,
                    "mirror_channel_id": p.mirror_ref.channel_id if p.mirror_ref else None,
                    "mirror_message_id": p.mirror_ref.message_id if p.mirror_ref else None,
                }
                for p in parts
            ])

        sql = """
        UPDATE file_records
        SET state = ?,
            primary_channel_id = COALESCE(?, primary_channel_id),
            primary_message_id = COALESCE(?, primary_message_id),
            mirror_channel_id = COALESCE(?, mirror_channel_id),
            mirror_message_id = COALESCE(?, mirror_message_id),
            parts = COALESCE(?, parts),
            updated_at = ?
        WHERE id = ?
        """
        with self._connection() as conn:
            conn.execute(
                sql,
                (
                    state.value,
                    p_ref.channel_id if p_ref else None,
                    p_ref.message_id if p_ref else None,
                    m_ref.channel_id if m_ref else None,
                    m_ref.message_id if m_ref else None,
                    serialized_parts,
                    now,
                    entity_id,
                ),
            )

    def search(self, query: str) -> list[FileRecord]:
        sql = """
        SELECT * FROM file_records
        WHERE name LIKE ? OR original_path LIKE ? OR sha256 LIKE ? OR tags LIKE ?
        ORDER BY created_at DESC
        """
        param = f"%{query}%"
        with self._connection() as conn:
            rows = conn.execute(sql, (param, param, param, param)).fetchall()
            return [self._row_to_record(r) for r in rows]

    def export_snapshot(self, dest: Path) -> None:
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(self.db_path, dest)

    def _row_to_record(self, r: sqlite3.Row) -> FileRecord:
        p_ref = None
        if r["primary_channel_id"] and r["primary_message_id"]:
            p_ref = MessageRef(channel_id=r["primary_channel_id"], message_id=r["primary_message_id"])

        m_ref = None
        if r["mirror_channel_id"] and r["mirror_message_id"]:
            m_ref = MessageRef(channel_id=r["mirror_channel_id"], message_id=r["mirror_message_id"])

        parts_list: list[FilePart] = []
        if "parts" in r.keys() and r["parts"]:
            try:
                raw_parts = json.loads(r["parts"])
                for item in raw_parts:
                    part_p_ref = (
                        MessageRef(channel_id=item["primary_channel_id"], message_id=item["primary_message_id"])
                        if item.get("primary_message_id")
                        else None
                    )
                    part_m_ref = (
                        MessageRef(channel_id=item["mirror_channel_id"], message_id=item["mirror_message_id"])
                        if item.get("mirror_message_id")
                        else None
                    )
                    parts_list.append(
                        FilePart(
                            part_number=item["part_number"],
                            total_parts=item["total_parts"],
                            size=item["size"],
                            sha256=item["sha256"],
                            primary_ref=part_p_ref,
                            mirror_ref=part_m_ref,
                        )
                    )
            except Exception:
                pass

        return FileRecord(
            id=r["id"],
            name=r["name"],
            original_path=r["original_path"],
            sha256=r["sha256"],
            size=r["size"],
            mtime=r["mtime"],
            state=HealthState(r["state"]),
            primary_ref=p_ref,
            mirror_ref=m_ref,
            local_status=LocalFileStatus(r["local_status"]),
            version=r["version"],
            mode=VaultMode(r["mode"]),
            tags=json.loads(r["tags"]) if r["tags"] else [],
            recovery_path=r["recovery_path"],
            parts=parts_list,
            created_at=datetime.fromisoformat(r["created_at"]),
            updated_at=datetime.fromisoformat(r["updated_at"]),
        )

    # ---------------------------------------------------------
    # TransactionJournal Implementation (Crash-Safe Transactions)
    # ---------------------------------------------------------
    def create(self, tx: TransactionRecord) -> None:
        sql = """
        INSERT INTO transactions (
            id, file_path, file_name, sha256, size, mode, state,
            primary_channel_id, primary_message_id,
            mirror_channel_id, mirror_message_id,
            record_id, manifest_generation, error_message,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """
        now = datetime.now(timezone.utc).isoformat()
        with self._connection() as conn:
            conn.execute(
                sql,
                (
                    tx.id,
                    tx.file_path,
                    tx.file_name,
                    tx.sha256,
                    tx.size,
                    tx.mode.value if hasattr(tx.mode, "value") else str(tx.mode),
                    tx.state.value if hasattr(tx.state, "value") else str(tx.state),
                    tx.primary_ref.channel_id if tx.primary_ref else None,
                    tx.primary_ref.message_id if tx.primary_ref else None,
                    tx.mirror_ref.channel_id if tx.mirror_ref else None,
                    tx.mirror_ref.message_id if tx.mirror_ref else None,
                    tx.record_id,
                    tx.manifest_generation,
                    tx.error_message,
                    tx.created_at.isoformat() if tx.created_at else now,
                    now,
                ),
            )

    def update_transaction_state(self, tx_id: str, state: TransactionState, **kwargs: Any) -> None:
        p_ref = kwargs.get("primary_ref")
        m_ref = kwargs.get("mirror_ref")
        rec_id = kwargs.get("record_id")
        sha = kwargs.get("sha256")
        err = kwargs.get("error_message")
        now = datetime.now(timezone.utc).isoformat()

        sql = """
        UPDATE transactions
        SET state = ?,
            primary_channel_id = COALESCE(?, primary_channel_id),
            primary_message_id = COALESCE(?, primary_message_id),
            mirror_channel_id = COALESCE(?, mirror_channel_id),
            mirror_message_id = COALESCE(?, mirror_message_id),
            record_id = COALESCE(?, record_id),
            sha256 = COALESCE(?, sha256),
            error_message = COALESCE(?, error_message),
            updated_at = ?
        WHERE id = ?
        """
        with self._connection() as conn:
            conn.execute(
                sql,
                (
                    state.value,
                    p_ref.channel_id if p_ref else None,
                    p_ref.message_id if p_ref else None,
                    m_ref.channel_id if m_ref else None,
                    m_ref.message_id if m_ref else None,
                    rec_id,
                    sha,
                    err,
                    now,
                    tx_id,
                ),
            )

    def get_transaction(self, tx_id: str) -> TransactionRecord | None:
        sql = "SELECT * FROM transactions WHERE id = ?"
        with self._connection() as conn:
            row = conn.execute(sql, (tx_id,)).fetchone()
            return self._row_to_transaction(row) if row else None

    def list_pending(self) -> list[TransactionRecord]:
        sql = """
        SELECT * FROM transactions
        WHERE state NOT IN ('COMPLETED', 'FAILED')
        ORDER BY created_at ASC
        """
        with self._connection() as conn:
            rows = conn.execute(sql).fetchall()
            return [self._row_to_transaction(r) for r in rows]

    def complete(self, tx_id: str, record_id: str) -> None:
        self.update_transaction_state(tx_id, TransactionState.COMPLETED, record_id=record_id)

    def fail(self, tx_id: str, error: str) -> None:
        self.update_transaction_state(tx_id, TransactionState.FAILED, error_message=error)

    def _row_to_transaction(self, r: sqlite3.Row) -> TransactionRecord:
        p_ref = None
        if r["primary_channel_id"] and r["primary_message_id"]:
            p_ref = MessageRef(channel_id=r["primary_channel_id"], message_id=r["primary_message_id"])

        m_ref = None
        if r["mirror_channel_id"] and r["mirror_message_id"]:
            m_ref = MessageRef(channel_id=r["mirror_channel_id"], message_id=r["mirror_message_id"])

        return TransactionRecord(
            id=r["id"],
            file_path=r["file_path"],
            file_name=r["file_name"],
            sha256=r["sha256"] or "",
            size=r["size"],
            mode=VaultMode(r["mode"]),
            state=TransactionState(r["state"]),
            primary_ref=p_ref,
            mirror_ref=m_ref,
            record_id=r["record_id"],
            manifest_generation=r["manifest_generation"],
            created_at=datetime.fromisoformat(r["created_at"]),
            updated_at=datetime.fromisoformat(r["updated_at"]),
            error_message=r["error_message"],
        )

    # ---------------------------------------------------------
    # ManifestRepository Implementation (Tamper-Evident Manifests)
    # ---------------------------------------------------------
    def save_manifest(self, manifest: ManifestRecord) -> None:
        sql = """
        INSERT INTO manifests (
            generation, vault_id, previous_hash, manifest_hash,
            record_ids, total_files, total_bytes, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """
        now = manifest.created_at.isoformat() if manifest.created_at else datetime.now(timezone.utc).isoformat()
        with self._connection() as conn:
            conn.execute(
                sql,
                (
                    manifest.generation,
                    manifest.vault_id,
                    manifest.previous_hash,
                    manifest.manifest_hash,
                    json.dumps(manifest.record_ids),
                    manifest.total_files,
                    manifest.total_bytes,
                    now,
                ),
            )

    def get_latest_manifest(self) -> ManifestRecord | None:
        sql = "SELECT * FROM manifests ORDER BY generation DESC LIMIT 1"
        with self._connection() as conn:
            row = conn.execute(sql).fetchone()
            return self._row_to_manifest(row) if row else None

    def get_manifest_by_generation(self, generation: int) -> ManifestRecord | None:
        sql = "SELECT * FROM manifests WHERE generation = ?"
        with self._connection() as conn:
            row = conn.execute(sql, (generation,)).fetchone()
            return self._row_to_manifest(row) if row else None

    def list_manifests(self) -> list[ManifestRecord]:
        sql = "SELECT * FROM manifests ORDER BY generation ASC"
        with self._connection() as conn:
            rows = conn.execute(sql).fetchall()
            return [self._row_to_manifest(r) for r in rows]

    def _row_to_manifest(self, r: sqlite3.Row) -> ManifestRecord:
        return ManifestRecord(
            generation=r["generation"],
            vault_id=r["vault_id"],
            previous_hash=r["previous_hash"],
            manifest_hash=r["manifest_hash"],
            record_ids=json.loads(r["record_ids"]),
            total_files=r["total_files"],
            total_bytes=r["total_bytes"],
            created_at=datetime.fromisoformat(r["created_at"]),
        )

    # ---------------------------------------------------------
    # AuditLedger Implementation (Append-Only Hash-Chained Events)
    # ---------------------------------------------------------
    def append_event(
        self,
        operation: str,
        record_id: str | None,
        result: str,
        details: str,
    ) -> AuditEvent:
        latest = self._get_latest_audit_event()
        prev_hash = latest.current_hash if latest else GENESIS_HASH

        now = datetime.now(timezone.utc)
        now_str = now.isoformat()
        event_id = hashlib.sha256(f"{now_str}_{operation}_{record_id}_{details}".encode()).hexdigest()[:16]

        payload = f"{prev_hash}|{now_str}|{operation}|{record_id or ''}|{result}|{details}"
        current_hash = hashlib.sha256(payload.encode("utf-8")).hexdigest()

        sql = """
        INSERT INTO audit_events (
            event_id, timestamp, operation, record_id,
            result, details, previous_hash, current_hash
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """
        with self._connection() as conn:
            conn.execute(sql, (event_id, now_str, operation, record_id, result, details, prev_hash, current_hash))

        return AuditEvent(
            event_id=event_id,
            timestamp=now,
            operation=operation,
            record_id=record_id,
            result=result,
            details=details,
            previous_hash=prev_hash,
            current_hash=current_hash,
        )

    def list_events(self, limit: int = 100) -> list[AuditEvent]:
        sql = "SELECT * FROM audit_events ORDER BY timestamp DESC LIMIT ?"
        with self._connection() as conn:
            rows = conn.execute(sql, (limit,)).fetchall()
            return [self._row_to_audit_event(r) for r in rows]

    def verify_integrity(self) -> tuple[bool, str]:
        sql = "SELECT * FROM audit_events ORDER BY timestamp ASC"
        with self._connection() as conn:
            rows = conn.execute(sql).fetchall()

        if not rows:
            return True, "Audit ledger is empty."

        expected_prev = GENESIS_HASH
        for idx, r in enumerate(rows):
            if r["previous_hash"] != expected_prev:
                return False, f"Broken chain at event {r['event_id']} (idx {idx}): previous_hash mismatch."

            now_str = r["timestamp"]
            payload = f"{r['previous_hash']}|{now_str}|{r['operation']}|{r['record_id'] or ''}|{r['result']}|{r['details'] or ''}"
            computed = hashlib.sha256(payload.encode("utf-8")).hexdigest()
            if computed != r["current_hash"]:
                return False, f"Tampered audit event {r['event_id']} (idx {idx}): hash signature corrupted."

            expected_prev = r["current_hash"]

        return True, f"Audit ledger verified: {len(rows)} chained events intact."

    def _get_latest_audit_event(self) -> AuditEvent | None:
        sql = "SELECT * FROM audit_events ORDER BY timestamp DESC LIMIT 1"
        with self._connection() as conn:
            row = conn.execute(sql).fetchone()
            return self._row_to_audit_event(row) if row else None

    def _row_to_audit_event(self, r: sqlite3.Row) -> AuditEvent:
        return AuditEvent(
            event_id=r["event_id"],
            timestamp=datetime.fromisoformat(r["timestamp"]),
            operation=r["operation"],
            record_id=r["record_id"],
            result=r["result"],
            details=r["details"] or "",
            previous_hash=r["previous_hash"],
            current_hash=r["current_hash"],
        )

    # ---------------------------------------------------------
    # Backup Receipts
    # ---------------------------------------------------------
    def save_receipt(self, receipt: BackupReceipt) -> None:
        sql = """
        INSERT INTO backup_receipts (
            backup_id, timestamp, files, total_size,
            primary_message_id, mirror_message_id,
            sha256, manifest_generation, sha256_verified
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """
        now = receipt.timestamp.isoformat() if receipt.timestamp else datetime.now(timezone.utc).isoformat()
        with self._connection() as conn:
            conn.execute(
                sql,
                (
                    receipt.backup_id,
                    now,
                    json.dumps(receipt.files),
                    receipt.total_size,
                    receipt.primary_message_id,
                    receipt.mirror_message_id,
                    receipt.sha256,
                    receipt.manifest_generation,
                    1 if receipt.sha256_verified else 0,
                ),
            )

    def get_receipt(self, backup_id: str) -> BackupReceipt | None:
        sql = "SELECT * FROM backup_receipts WHERE backup_id = ?"
        with self._connection() as conn:
            row = conn.execute(sql, (backup_id,)).fetchone()
            if not row:
                return None
            return BackupReceipt(
                backup_id=row["backup_id"],
                timestamp=datetime.fromisoformat(row["timestamp"]),
                files=json.loads(row["files"]),
                total_size=row["total_size"],
                primary_message_id=row["primary_message_id"],
                mirror_message_id=row["mirror_message_id"],
                sha256=row["sha256"],
                manifest_generation=row["manifest_generation"],
                sha256_verified=bool(row["sha256_verified"]),
            )
