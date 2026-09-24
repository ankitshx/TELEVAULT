from pathlib import Path
import pytest

from televault.application.backup import BackupFileUseCase
from televault.application.versioning import BackupNewVersionUseCase, VersionManager
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.mark.asyncio
async def test_versioning_sequential_increments(tmp_path: Path):
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    backup_uc = BackupFileUseCase(gateway, repo)
    version_uc = BackupNewVersionUseCase(backup_uc, repo)
    manager = VersionManager(repo)

    test_file = tmp_path / "ledger.xlsx"

    # Version 1
    test_file.write_bytes(b"January ledger figures")
    res1 = await version_uc.execute(test_file)
    assert res1.record.version == 1

    # Version 2
    test_file.write_bytes(b"February ledger figures with changes")
    res2 = await version_uc.execute(test_file)
    assert res2.record.version == 2

    # Version 3
    test_file.write_bytes(b"March ledger figures with further changes")
    res3 = await version_uc.execute(test_file)
    assert res3.record.version == 3

    # Check history
    history = manager.get_version_history(str(test_file.resolve()))
    assert len(history) == 3
    assert [r.version for r in history] == [3, 2, 1]
