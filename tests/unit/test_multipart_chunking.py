import os
from pathlib import Path
import pytest

from televault.application.backup import BackupFileUseCase
from televault.application.chunking import (
    extract_chunk_to_file,
    merge_chunks,
    split_file_slices,
)
from televault.application.hashing import calculate_sha256
from televault.application.restore import RestoreFileUseCase
from televault.domain.states import HealthState
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


def test_split_file_slices_logic():
    # 5 MB file with 2 MB chunks -> 3 chunks: [2MB, 2MB, 1MB]
    slices = split_file_slices(5 * 1024 * 1024, chunk_size=2 * 1024 * 1024)
    assert len(slices) == 3
    assert slices[0] == (0, 2 * 1024 * 1024)
    assert slices[1] == (2 * 1024 * 1024, 2 * 1024 * 1024)
    assert slices[2] == (4 * 1024 * 1024, 1 * 1024 * 1024)


def test_extract_and_merge_chunks_roundtrip(tmp_path: Path):
    source_file = tmp_path / "large_sample.bin"
    # Create 2.5 MB random test payload
    original_data = os.urandom(2500000)
    source_file.write_bytes(original_data)
    original_sha, _ = calculate_sha256(source_file)

    slices = split_file_slices(len(original_data), chunk_size=1000000)
    assert len(slices) == 3

    chunk_files: list[Path] = []
    for idx, (offset, size) in enumerate(slices, start=1):
        chunk_file = tmp_path / f"part_{idx}.tmp"
        chunk_sha = extract_chunk_to_file(source_file, offset, size, chunk_file)
        assert chunk_file.stat().st_size == size
        chunk_files.append(chunk_file)

    reconstructed = tmp_path / "reconstructed.bin"
    merged_sha = merge_chunks(chunk_files, reconstructed)

    assert merged_sha == original_sha
    assert reconstructed.read_bytes() == original_data


@pytest.mark.asyncio
async def test_backup_single_document_when_under_limit(tmp_path: Path):
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "test.db")
    # Set limit to 2 MB (2097152 bytes)
    backup_uc = BackupFileUseCase(gateway, repo, max_single_file_size=2 * 1024 * 1024)

    small_file = tmp_path / "small.dat"
    small_file.write_bytes(b"Small document under 2 GB limit")

    res = await backup_uc.execute(small_file)
    assert res.record.state == HealthState.HEALTHY
    assert res.record.is_multipart is False
    assert len(res.record.parts) == 0
    # In fake gateway, exactly 1 upload call (single document)
    assert len(gateway.upload_calls) == 1
    assert gateway.upload_calls[0]["reply_to"] is None


@pytest.mark.asyncio
async def test_backup_and_restore_multipart_when_over_limit(tmp_path: Path):
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "test.db")
    # Simulate a 100 KB limit with 40 KB chunks (to test 3-part split quickly)
    chunk_limit = 100 * 1024
    chunk_size = 40 * 1024
    backup_uc = BackupFileUseCase(
        gateway,
        repo,
        max_single_file_size=chunk_limit,
        chunk_size=chunk_size,
    )
    restore_uc = RestoreFileUseCase(gateway, repo)

    large_file = tmp_path / "big_video.mp4"
    data = os.urandom(110 * 1024)  # 110 KB -> 3 parts (40 KB, 40 KB, 30 KB)
    large_file.write_bytes(data)
    orig_mtime = 1750000000.0
    os.utime(large_file, (orig_mtime, orig_mtime))

    # Step 1: Execute Backup
    res = await backup_uc.execute(large_file)
    assert res.record.state == HealthState.HEALTHY
    assert res.record.is_multipart is True
    assert len(res.record.parts) == 3

    # Verify Telegram layout:
    # 1. Master post created (primary_ref is the master message)
    # 2. Uploaded chunk documents replied to primary master message
    for call in gateway.upload_calls:
        assert call["reply_to"] == res.record.primary_ref.message_id

    # Verify parts stored in DB correctly
    persisted = repo.get(res.record.id)
    assert persisted.is_multipart is True
    assert len(persisted.parts) == 3
    assert persisted.parts[0].part_number == 1
    assert persisted.parts[0].size == 40 * 1024
    assert persisted.parts[1].part_number == 2
    assert persisted.parts[1].size == 40 * 1024
    assert persisted.parts[2].part_number == 3
    assert persisted.parts[2].size == 30 * 1024

    # Step 2: Execute Restore
    restore_dest = tmp_path / "restored_output"
    r_res = await restore_uc.execute(res.record.id, restore_dest)

    assert r_res.success is True
    assert r_res.sha256_matched is True
    assert r_res.restored_path.name == "big_video.mp4"
    assert r_res.restored_path.read_bytes() == data
    assert abs(r_res.restored_path.stat().st_mtime - orig_mtime) < 1.0


@pytest.mark.asyncio
async def test_multipart_tampered_chunk_detected(tmp_path: Path):
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "test.db")
    backup_uc = BackupFileUseCase(
        gateway,
        repo,
        max_single_file_size=50 * 1024,
        chunk_size=30 * 1024,
    )
    restore_uc = RestoreFileUseCase(gateway, repo)

    test_file = tmp_path / "game_installer.iso"
    test_file.write_bytes(os.urandom(80 * 1024))

    res = await backup_uc.execute(test_file)
    assert res.record.is_multipart is True

    # Tamper with chunk part 2 in the gateway primary channel
    part2_msg_id = res.record.parts[1].primary_ref.message_id
    stored = gateway.channels[gateway.primary_id][part2_msg_id]
    stored.data = b"CORRUPTED_BYTES" + stored.data[15:]

    restore_dest = tmp_path / "restored_tampered"
    r_res = await restore_uc.execute(res.record.id, restore_dest)

    # Invariant 4 must reject corrupted chunk
    assert r_res.success is False
    assert r_res.sha256_matched is False
    assert "Integrity check failed" in r_res.message
    # Final destination file should not exist (cleaned up)
    assert not (restore_dest / "game_installer.iso").exists()
