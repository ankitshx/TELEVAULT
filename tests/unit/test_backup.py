from pathlib import Path
import pytest

from televault.application.backup import BackupFileUseCase
from televault.application.event_bus import SimpleEventBus
from televault.domain.states import HealthState
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.mark.asyncio
async def test_backup_dual_write_success(tmp_path: Path):
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    event_bus = SimpleEventBus()
    use_case = BackupFileUseCase(gateway, repo, event_bus)

    # Create dummy file to back up
    sample_file = tmp_path / "project_report.docx"
    sample_file.write_bytes(b"Executive summary and quarterly metrics data.")

    result = await use_case.execute(sample_file)

    assert result.is_duplicate is False
    assert result.dry_run is False
    assert result.record.state == HealthState.HEALTHY
    assert result.record.primary_ref is not None
    assert result.record.mirror_ref is not None

    # Check fake gateway calls
    assert len(gateway.upload_calls) == 1
    assert len(gateway.forward_calls) == 1
    assert gateway.forward_calls[0]["to_channel"] == gateway.mirror_id

    # Verify persisted in SQLite
    persisted = repo.get(result.record.id)
    assert persisted.state == HealthState.HEALTHY
    assert persisted.primary_ref == result.record.primary_ref
    assert persisted.mirror_ref == result.record.mirror_ref


@pytest.mark.asyncio
async def test_backup_deduplication(tmp_path: Path):
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    use_case = BackupFileUseCase(gateway, repo)

    sample_file = tmp_path / "data.csv"
    sample_file.write_bytes(b"id,name,value\n1,alpha,100\n2,beta,200\n")

    # First backup -> uploads
    res1 = await use_case.execute(sample_file)
    assert res1.is_duplicate is False
    assert len(gateway.upload_calls) == 1

    # Second backup of identical file -> dedupe skips upload
    res2 = await use_case.execute(sample_file)
    assert res2.is_duplicate is True
    assert res2.record.id == res1.record.id
    # Upload count remains 1!
    assert len(gateway.upload_calls) == 1


@pytest.mark.asyncio
async def test_backup_dry_run_does_not_upload(tmp_path: Path):
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    use_case = BackupFileUseCase(gateway, repo)

    sample_file = tmp_path / "confidential.pdf"
    sample_file.write_bytes(b"Top secret dry run content.")

    res = await use_case.execute(sample_file, dry_run=True)
    assert res.dry_run is True
    assert "[DRY-RUN]" in res.message
    # Gateway and repo untouched
    assert len(gateway.upload_calls) == 0
    assert repo.list_all() == []
