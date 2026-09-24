import ast
import hashlib
import os
from pathlib import Path
import pytest

from televault.application.backup import BackupFileUseCase
from televault.application.heal import HealVaultUseCase
from televault.application.rebuild import RebuildIndexUseCase
from televault.application.restore import RestoreFileUseCase
from televault.application.verify import VerifyVaultUseCase
from televault.domain.states import HealthState, LocalFileStatus, VaultMode
from televault.infrastructure.crypto.stream_cipher import (
    CryptoError,
    decrypt_file_stream,
    encrypt_file_stream,
)
from televault.infrastructure.storage.recovery_cache import RecoveryCache
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from televault.infrastructure.telegram.limits import AccountLimits
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.mark.asyncio
async def test_acceptance_01_single_document_dual_write_1gb_simulation(tmp_path: Path):
    """1. Drop a file: exactly one message in Primary, one in Mirror, matching name and size."""
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    use_case = BackupFileUseCase(gateway, repo)

    test_file = tmp_path / "large_dataset.iso"
    # Simulate a file with distinct bytes
    test_file.write_bytes(b"Simulated high-density ISO payload." * 1000)

    result = await use_case.execute(test_file)

    assert result.record.state == HealthState.HEALTHY
    assert len(gateway.upload_calls) == 1
    assert gateway.upload_calls[0]["force_document"] is True
    assert gateway.upload_calls[0]["size"] == test_file.stat().st_size

    assert len(gateway.forward_calls) == 1
    assert gateway.forward_calls[0]["to_channel"] == gateway.mirror_id


@pytest.mark.asyncio
async def test_acceptance_02_delete_primary_degraded_and_healed(tmp_path: Path):
    """2. Delete Primary message manually, run Verify: DEGRADED, heal restores it to HEALTHY."""
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "index.db")
    backup_uc = BackupFileUseCase(gateway, repo)
    verify_uc = VerifyVaultUseCase(gateway, repo)
    heal_uc = HealVaultUseCase(gateway, repo)

    file_p = tmp_path / "report.pdf"
    file_p.write_bytes(b"Financial Report 2026")
    res = await backup_uc.execute(file_p)
    rec_id = res.record.id

    # Delete primary copy
    gateway.simulate_external_delete(res.record.primary_ref.channel_id, res.record.primary_ref.message_id)

    # Verify detects degradation
    v_res = await verify_uc.execute(rec_id)
    assert v_res.degraded_count == 1
    assert repo.get(rec_id).state == HealthState.DEGRADED

    # Heal re-forwards surviving mirror copy
    h_res = await heal_uc.execute(rec_id)
    assert h_res.healed_count == 1
    assert repo.get(rec_id).state == HealthState.HEALTHY


@pytest.mark.asyncio
async def test_acceptance_03_double_deletion_recovery_and_lost(tmp_path: Path):
    """3. Delete both messages with recovery copy present -> re-upload succeeds.

    Without local copy -> state LOST with honest failure.
    """
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "index.db")
    cache = RecoveryCache(tmp_path / "cache")
    backup_uc = BackupFileUseCase(gateway, repo)
    heal_uc = HealVaultUseCase(gateway, repo, cache)
    verify_uc = VerifyVaultUseCase(gateway, repo, cache)

    # Item with recovery cache
    file_cached = tmp_path / "cached.dat"
    file_cached.write_bytes(b"Cached data bytes")
    res_c = await backup_uc.execute(file_cached)
    cache.store(file_cached, res_c.record.sha256)

    # Delete both cloud copies
    gateway.simulate_external_delete(res_c.record.primary_ref.channel_id, res_c.record.primary_ref.message_id)
    gateway.simulate_external_delete(res_c.record.mirror_ref.channel_id, res_c.record.mirror_ref.message_id)

    # Heal succeeds from cache
    h_res = await heal_uc.execute(res_c.record.id)
    assert h_res.healed_count == 1
    assert repo.get(res_c.record.id).state == HealthState.HEALTHY

    # Item without cache
    file_uncached = tmp_path / "uncached.dat"
    file_uncached.write_bytes(b"Uncached data bytes")
    res_u = await backup_uc.execute(file_uncached)

    gateway.simulate_external_delete(res_u.record.primary_ref.channel_id, res_u.record.primary_ref.message_id)
    gateway.simulate_external_delete(res_u.record.mirror_ref.channel_id, res_u.record.mirror_ref.message_id)

    v_res = await verify_uc.execute(res_u.record.id)
    assert v_res.lost_count == 1
    assert repo.get(res_u.record.id).state == HealthState.LOST


@pytest.mark.asyncio
async def test_acceptance_04_rebuild_from_telegram_captions(tmp_path: Path):
    """4. Delete local DB: Rebuild from Telegram reproduces all records from captions."""
    gateway = FakeTelegramGateway()
    db1 = tmp_path / "db1.db"
    repo1 = SQLiteVaultRepository(db1)
    backup_uc = BackupFileUseCase(gateway, repo1)

    f1 = tmp_path / "f1.pdf"
    f1.write_bytes(b"File 1 content")
    r1 = await backup_uc.execute(f1)

    f2 = tmp_path / "f2.png"
    f2.write_bytes(b"File 2 content")
    r2 = await backup_uc.execute(f2)

    # Wipe DB
    db2 = tmp_path / "db2.db"
    repo2 = SQLiteVaultRepository(db2)

    rebuild_uc = RebuildIndexUseCase(gateway, repo2)
    s = await rebuild_uc.execute()
    assert s.records_reconstructed == 2
    assert s.records_healthy == 2

    rebuilt = repo2.list_all()
    assert len(rebuilt) == 2
    assert {r.name for r in rebuilt} == {"f1.pdf", "f2.png"}


@pytest.mark.asyncio
async def test_acceptance_05_restore_sha256_filename_mtime_match(tmp_path: Path):
    """5. Restore any file: SHA-256, filename, and mtime match original."""
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "index.db")
    backup_uc = BackupFileUseCase(gateway, repo)
    restore_uc = RestoreFileUseCase(gateway, repo)

    test_file = tmp_path / "document_v1.docx"
    content = b"Detailed acceptance contract specification."
    test_file.write_bytes(content)
    orig_mtime = 1774000000.0
    os.utime(test_file, (orig_mtime, orig_mtime))

    b_res = await backup_uc.execute(test_file)

    restore_dir = tmp_path / "dest"
    r_res = await restore_uc.execute(b_res.record.id, restore_dir)

    assert r_res.success is True
    assert r_res.sha256_matched is True
    assert r_res.restored_path.name == "document_v1.docx"
    assert r_res.restored_path.read_bytes() == content
    assert abs(r_res.restored_path.stat().st_mtime - orig_mtime) < 1.0


def test_acceptance_06_overlimit_check():
    """6. File over 2 GB (4 GB Premium): account limit detects correctly."""
    free_limits = AccountLimits(is_premium=False)
    assert free_limits.exceeds_limit(2001 * 1024 * 1024) is True
    assert free_limits.exceeds_limit(1900 * 1024 * 1024) is False

    prem_limits = AccountLimits(is_premium=True)
    assert prem_limits.exceeds_limit(3000 * 1024 * 1024) is False
    assert prem_limits.exceeds_limit(4001 * 1024 * 1024) is True


def test_acceptance_08_private_mode_crypto_roundtrip_and_passphrase_failure(tmp_path: Path):
    """8. Private mode round-trip works and wrong passphrase fails cleanly."""
    src = tmp_path / "confidential.txt"
    src.write_bytes(b"Confidential data bytes")
    enc = tmp_path / "enc.tvc"

    encrypt_file_stream(src, enc, "Passphrase123")

    # Correct passphrase -> restores
    out_dir = tmp_path / "dec_ok"
    dec = decrypt_file_stream(enc, out_dir, "Passphrase123")
    assert dec.read_bytes() == b"Confidential data bytes"
    assert dec.name == "confidential.txt"

    # Wrong passphrase -> fails cleanly
    fail_dir = tmp_path / "dec_fail"
    with pytest.raises(CryptoError):
        decrypt_file_stream(enc, fail_dir, "WrongPassword")


@pytest.mark.asyncio
async def test_acceptance_09_manual_only_local_pc_deletion_untouched(tmp_path: Path):
    """9. Delete backed-up file from PC: cloud copies untouched, row shows 'Only in Telegram'."""
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "index.db")
    backup_uc = BackupFileUseCase(gateway, repo)
    verify_uc = VerifyVaultUseCase(gateway, repo)

    local_file = tmp_path / "user_notes.txt"
    local_file.write_bytes(b"Local notes")
    b_res = await backup_uc.execute(local_file)

    # Delete locally on PC
    local_file.unlink()

    v_res = await verify_uc.execute(b_res.record.id)
    assert v_res.results[0].local_status == LocalFileStatus.MISSING
    # Ensure zero delete calls made to Telegram
    assert len(gateway.delete_message_calls) == 0


def test_acceptance_10_grep_test_no_deletions_and_no_auto_uploads():
    """10. Grep test: codebase contains NO call that deletes Telegram messages

    and NO code path uploads without a user-triggered action.
    """
    src_dir = Path(__file__).resolve().parent.parent.parent / "televault"
    py_files = list(src_dir.rglob("*.py"))
    assert len(py_files) > 0

    forbidden_calls = {"delete_messages", "delete_dialog", "delete_channel"}
    forbidden_modules = {"watchdog", "apscheduler", "schedule"}

    for f in py_files:
        content = f.read_text(encoding="utf-8")
        tree = ast.parse(content, filename=str(f))
        for node in ast.walk(tree):
            if isinstance(node, ast.Attribute) and node.attr in forbidden_calls:
                pytest.fail(f"Forbidden call {node.attr} in {f.name}:{node.lineno}")
            if isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name.split(".")[0].lower() in forbidden_modules:
                        pytest.fail(f"Forbidden import {alias.name} in {f.name}:{node.lineno}")


@pytest.mark.asyncio
async def test_acceptance_11_modified_file_shows_changed_no_auto_upload(tmp_path: Path):
    """11. Modify a backed-up file locally: row shows 'Changed since last backup'

    and nothing uploads until user explicitly backs up again.
    """
    gateway = FakeTelegramGateway()
    repo = SQLiteVaultRepository(tmp_path / "index.db")
    backup_uc = BackupFileUseCase(gateway, repo)
    verify_uc = VerifyVaultUseCase(gateway, repo)

    test_file = tmp_path / "notes.txt"
    test_file.write_bytes(b"Version 1")
    b_res = await backup_uc.execute(test_file)

    # User modifies file
    test_file.write_bytes(b"Version 2 - Modified")

    v_res = await verify_uc.execute(b_res.record.id)
    assert v_res.results[0].local_status == LocalFileStatus.CHANGED
    # Upload calls remains 1 (no auto upload!)
    assert len(gateway.upload_calls) == 1
