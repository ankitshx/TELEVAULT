from pathlib import Path
import pytest

from televault.application.backup import BackupFileUseCase
from televault.application.rebuild import RebuildIndexUseCase
from televault.domain.states import HealthState
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.mark.asyncio
async def test_rebuild_from_telegram_after_db_wipe(tmp_path: Path):
    """Simulate catastrophic loss of local SQLite database.

    RebuildIndexUseCase scans Telegram channels, parses tv1 captions,
    and reconstructs the index from scratch.
    """
    gateway = FakeTelegramGateway()

    # Original DB where files are backed up
    orig_db_path = tmp_path / "original_index.db"
    orig_repo = SQLiteVaultRepository(orig_db_path)
    backup_uc = BackupFileUseCase(gateway, orig_repo)

    file_1 = tmp_path / "annual_report.pdf"
    file_1.write_bytes(b"Annual financial figures and projections.")
    res1 = await backup_uc.execute(file_1)

    file_2 = tmp_path / "client_list.csv"
    file_2.write_bytes(b"name,email\nAlice,alice@example.com\nBob,bob@example.com\n")
    res2 = await backup_uc.execute(file_2)

    assert len(orig_repo.list_all()) == 2

    # SIMULATE DISK CATASTROPHE: Delete the original SQLite database!
    orig_db_path.unlink()
    assert not orig_db_path.exists()

    # Create fresh empty SQLite repository
    fresh_db_path = tmp_path / "reconstructed_index.db"
    fresh_repo = SQLiteVaultRepository(fresh_db_path)
    assert len(fresh_repo.list_all()) == 0

    # Execute RebuildFromTelegramUseCase
    rebuild_uc = RebuildIndexUseCase(gateway, fresh_repo)
    rebuild_summary = await rebuild_uc.execute()

    assert rebuild_summary.records_reconstructed == 2
    assert rebuild_summary.records_healthy == 2
    assert rebuild_summary.records_degraded == 0

    reconstructed_records = fresh_repo.list_all()
    assert len(reconstructed_records) == 2

    # Verify field-level fidelity
    rec_map = {r.id: r for r in reconstructed_records}

    r1 = rec_map[res1.record.id]
    assert r1.name == "annual_report.pdf"
    assert r1.sha256 == res1.record.sha256
    assert r1.size == res1.record.size
    assert r1.state == HealthState.HEALTHY
    assert r1.primary_ref == res1.record.primary_ref
    assert r1.mirror_ref == res1.record.mirror_ref

    r2 = rec_map[res2.record.id]
    assert r2.name == "client_list.csv"
    assert r2.sha256 == res2.record.sha256
    assert r2.size == res2.record.size
    assert r2.state == HealthState.HEALTHY
