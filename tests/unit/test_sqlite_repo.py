from pathlib import Path
import pytest

from televault.domain.entities import FileRecord, MessageRef
from televault.domain.states import HealthState, LocalFileStatus
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository


def test_sqlite_repo_crud(tmp_path: Path):
    db_file = tmp_path / "test_vault.db"
    repo = SQLiteVaultRepository(db_file)

    # Initial empty list
    assert repo.list_all() == []

    # Add a record
    record = FileRecord(
        name="document.pdf",
        original_path="C:/docs/document.pdf",
        sha256="11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff",
        size=1024,
        mtime=1700000000.0,
        state=HealthState.PENDING,
        local_status=LocalFileStatus.PRESENT,
        tags=["work", "finance"],
    )
    repo.add(record)

    # Fetch back
    fetched = repo.get(record.id)
    assert fetched.id == record.id
    assert fetched.name == "document.pdf"
    assert fetched.sha256 == record.sha256
    assert fetched.tags == ["work", "finance"]
    assert fetched.state == HealthState.PENDING

    # Find by hash
    by_hash = repo.find_by_hash(record.sha256)
    assert by_hash is not None
    assert by_hash.id == record.id

    # Update state and refs
    p_ref = MessageRef(channel_id=-1001, message_id=42)
    m_ref = MessageRef(channel_id=-1002, message_id=84)
    repo.update_state(record.id, HealthState.HEALTHY, primary_ref=p_ref, mirror_ref=m_ref)

    updated = repo.get(record.id)
    assert updated.state == HealthState.HEALTHY
    assert updated.primary_ref == p_ref
    assert updated.mirror_ref == m_ref

    # Search
    search_results = repo.search("finance")
    assert len(search_results) == 1
    assert search_results[0].id == record.id

    # Snapshot export
    snapshot_file = tmp_path / "snapshot.db"
    repo.export_snapshot(snapshot_file)
    assert snapshot_file.exists()
    assert snapshot_file.stat().st_size > 0
