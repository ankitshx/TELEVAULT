from pathlib import Path
import pytest

from televault.application.backup import BackupFileUseCase
from televault.application.restore import RestoreFileUseCase
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.mark.asyncio
async def test_restore_success_with_hash_match(tmp_path: Path):
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)

    backup_uc = BackupFileUseCase(gateway, repo)
    restore_uc = RestoreFileUseCase(gateway, repo)

    # 1. Back up a file
    source_file = tmp_path / "important_contract.pdf"
    content = b"This is a legally binding contract content preserved in TeleVault."
    source_file.write_bytes(content)

    backup_res = await backup_uc.execute(source_file)
    rec_id = backup_res.record.id

    # 2. Restore file to a new target directory
    restore_dir = tmp_path / "restored_output"
    restore_res = await restore_uc.execute(record_id=rec_id, dest_dir=restore_dir)

    assert restore_res.success is True
    assert restore_res.sha256_matched is True
    assert restore_res.restored_path is not None
    assert restore_res.restored_path.exists()
    assert restore_res.restored_path.read_bytes() == content
    assert restore_res.restored_path.name == "important_contract.pdf"


@pytest.mark.asyncio
async def test_restore_tampered_download_fails_invariant_4(tmp_path: Path):
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)

    backup_uc = BackupFileUseCase(gateway, repo)
    restore_uc = RestoreFileUseCase(gateway, repo)

    source_file = tmp_path / "source.txt"
    source_file.write_bytes(b"Original untampered data")
    backup_res = await backup_uc.execute(source_file)
    rec_id = backup_res.record.id

    # Simulate bitrot/tampering in the fake gateway's stored message
    msg_id = backup_res.record.primary_ref.message_id
    stored = gateway.channels[gateway.primary_id][msg_id]
    stored.data = b"Corrupted bytes injected by network error"

    restore_dir = tmp_path / "tampered_output"
    restore_res = await restore_uc.execute(record_id=rec_id, dest_dir=restore_dir)

    # Assert invariant 4: rejected and corrupted file deleted
    assert restore_res.success is False
    assert restore_res.sha256_matched is False
    assert "Integrity check failed" in restore_res.message
    # Corrupted destination file must be deleted
    assert not (restore_dir / "source.txt").exists()
