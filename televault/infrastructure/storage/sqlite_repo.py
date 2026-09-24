from contextlib import contextmanager
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import sqlite3
from typing import Any, Iterator

from televault.domain.entities import FileRecord, MessageRef
from televault.domain.ports import VaultRepository
from televault.domain.states import HealthState, LocalFileStatus, VaultMode


class SQLiteVaultRepository(VaultRepository):
    """SQLite-backed metadata index satisfying the VaultRepository protocol."""

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
        """Run initial schema migration if tables don't exist."""
        migration_file = Path(__file__).parent / "migrations" / "001_initial.sql"
        schema_sql = migration_file.read_text(encoding="utf-8")
        with self._connection() as conn:
            conn.executescript(schema_sql)

    def add(self, rec: FileRecord) -> None:
        sql = """
        INSERT INTO file_records (
            id, name, original_path, sha256, size, mtime, state,
            primary_channel_id, primary_message_id,
            mirror_channel_id, mirror_message_id,
            local_status, version, mode, tags, recovery_path,
            created_at, updated_at
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?,
            ?, ?,
            ?, ?,
            ?, ?, ?, ?, ?,
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
            updated_at = excluded.updated_at
        """
        now = datetime.now(timezone.utc).isoformat()
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
                    rec.created_at.isoformat() if rec.created_at else now,
                    now,
                ),
            )

    def get(self, record_id: str) -> FileRecord:
        sql = "SELECT * FROM file_records WHERE id = ?"
        with self._connection() as conn:
            row = conn.execute(sql, (record_id,)).fetchone()
            if not row:
                raise KeyError(f"FileRecord not found with id: {record_id}")
            return self._row_to_record(row)

    def find_by_hash(self, sha256: str) -> FileRecord | None:
        sql = "SELECT * FROM file_records WHERE sha256 = ? ORDER BY version DESC LIMIT 1"
        with self._connection() as conn:
            row = conn.execute(sql, (sha256,)).fetchone()
            if not row:
                return None
            return self._row_to_record(row)

    def list_all(self) -> list[FileRecord]:
        sql = "SELECT * FROM file_records ORDER BY created_at DESC"
        with self._connection() as conn:
            rows = conn.execute(sql).fetchall()
            return [self._row_to_record(r) for r in rows]

    def update_state(self, id: str, state: HealthState, **refs: Any) -> None:
        now = datetime.now(timezone.utc).isoformat()
        primary_ref = refs.get("primary_ref")
        mirror_ref = refs.get("mirror_ref")

        updates = ["state = ?", "updated_at = ?"]
        params: list[Any] = [state.value, now]

        if primary_ref is not None:
            updates.extend(["primary_channel_id = ?", "primary_message_id = ?"])
            params.extend([primary_ref.channel_id, primary_ref.message_id])

        if mirror_ref is not None:
            updates.extend(["mirror_channel_id = ?", "mirror_message_id = ?"])
            params.extend([mirror_ref.channel_id, mirror_ref.message_id])

        params.append(id)
        sql = f"UPDATE file_records SET {', '.join(updates)} WHERE id = ?"

        with self._connection() as conn:
            cursor = conn.execute(sql, params)
            if cursor.rowcount == 0:
                raise KeyError(f"FileRecord not found with id: {id}")

    def search(self, query: str) -> list[FileRecord]:
        like_query = f"%{query}%"
        sql = """
        SELECT * FROM file_records
        WHERE name LIKE ? OR tags LIKE ? OR original_path LIKE ?
        ORDER BY created_at DESC
        """
        with self._connection() as conn:
            rows = conn.execute(sql, (like_query, like_query, like_query)).fetchall()
            return [self._row_to_record(r) for r in rows]

    def export_snapshot(self, dest: Path) -> None:
        """Create a consistent SQLite file snapshot on disk."""
        dest.parent.mkdir(parents=True, exist_ok=True)
        # Using SQLite VACUUM INTO for consistent transaction-safe copy
        with self._connection() as conn:
            # If destination already exists, remove it first
            if dest.exists():
                dest.unlink()
            conn.execute(f"VACUUM INTO '{dest.resolve()}'")

    def _row_to_record(self, row: sqlite3.Row) -> FileRecord:
        primary_ref = None
        if row["primary_channel_id"] is not None and row["primary_message_id"] is not None:
            primary_ref = MessageRef(
                channel_id=row["primary_channel_id"],
                message_id=row["primary_message_id"],
            )

        mirror_ref = None
        if row["mirror_channel_id"] is not None and row["mirror_message_id"] is not None:
            mirror_ref = MessageRef(
                channel_id=row["mirror_channel_id"],
                message_id=row["mirror_message_id"],
            )

        return FileRecord(
            id=row["id"],
            name=row["name"],
            original_path=row["original_path"],
            sha256=row["sha256"],
            size=row["size"],
            mtime=row["mtime"],
            state=HealthState(row["state"]),
            primary_ref=primary_ref,
            mirror_ref=mirror_ref,
            local_status=LocalFileStatus(row["local_status"]),
            version=row["version"],
            mode=VaultMode(row["mode"]),
            tags=json.loads(row["tags"]),
            recovery_path=row["recovery_path"],
            created_at=datetime.fromisoformat(row["created_at"]),
            updated_at=datetime.fromisoformat(row["updated_at"]),
        )
