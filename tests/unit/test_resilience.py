from pathlib import Path
import pytest

from televault.application.backup import BackupFileUseCase
from televault.application.heal import HealVaultUseCase
from televault.application.verify import VerifyVaultUseCase
from televault.domain.states import HealthState, LocalFileStatus
from televault.infrastructure.storage.recovery_cache import RecoveryCache
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.mark.asyncio
async def test_scenario_primary_deleted_degraded_and_healed(tmp_path: Path):
    """Scenario 1 & 2: Primary copy deleted -> DEGRADED -> Auto-heal forwards Mirror -> HEALTHY."""
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    cache = RecoveryCache(tmp_path / "cache")

    backup_uc = BackupFileUseCase(gateway, repo)
    verify_uc = VerifyVaultUseCase(gateway, repo, cache)
    heal_uc = HealVaultUseCase(gateway, repo, cache)

    # 1. Back up a file
    source_file = tmp_path / "sensitive_report.pdf"
    source_file.write_bytes(b"Highly sensitive strategic report data.")
    backup_res = await backup_uc.execute(source_file)
    rec_id = backup_res.record.id

    assert backup_res.record.state == HealthState.HEALTHY
    initial_p_ref = backup_res.record.primary_ref
    initial_m_ref = backup_res.record.mirror_ref

    # 2. Simulate manual deletion of Primary message in Telegram
    gateway.simulate_external_delete(initial_p_ref.channel_id, initial_p_ref.message_id)

    # 3. Run Verify -> Detects degradation
    v_summary = await verify_uc.execute(rec_id)
    assert v_summary.degraded_count == 1
    assert repo.get(rec_id).state == HealthState.DEGRADED

    # 4. Run Heal -> Server-side forwards surviving Mirror copy back to Primary
    initial_uploads = len(gateway.upload_calls)
    initial_forwards = len(gateway.forward_calls)

    h_summary = await heal_uc.execute(rec_id)
    assert h_summary.healed_count == 1

    healed_rec = repo.get(rec_id)
    assert healed_rec.state == HealthState.HEALTHY
    assert healed_rec.primary_ref.message_id != initial_p_ref.message_id
    assert healed_rec.mirror_ref.message_id == initial_m_ref.message_id

    # Crucial: Healing did NOT re-upload bytes; it forwarded server-side!
    assert len(gateway.upload_calls) == initial_uploads
    assert len(gateway.forward_calls) == initial_forwards + 1


@pytest.mark.asyncio
async def test_scenario_both_deleted_with_local_recovery_copy(tmp_path: Path):
    """Scenario 4: Both cloud copies deleted, local recovery cache restores and re-uploads."""
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    cache = RecoveryCache(tmp_path / "cache")

    backup_uc = BackupFileUseCase(gateway, repo)
    heal_uc = HealVaultUseCase(gateway, repo, cache)

    source_file = tmp_path / "cached_doc.txt"
    source_file.write_bytes(b"Valuable content protected by local recovery copy.")
    backup_res = await backup_uc.execute(source_file)
    rec_id = backup_res.record.id

    # Populate local recovery cache
    cache.store(source_file, backup_res.record.sha256)
    assert cache.has(backup_res.record.sha256)

    # Simulate deletion of BOTH Primary and Mirror messages
    p = backup_res.record.primary_ref
    m = backup_res.record.mirror_ref
    gateway.simulate_external_delete(p.channel_id, p.message_id)
    gateway.simulate_external_delete(m.channel_id, m.message_id)

    # Run Heal -> Re-uploads from recovery cache
    heal_res = await heal_uc.execute(rec_id)
    assert heal_res.healed_count == 1

    healed_rec = repo.get(rec_id)
    assert healed_rec.state == HealthState.HEALTHY


@pytest.mark.asyncio
async def test_scenario_both_deleted_without_cache_marks_lost(tmp_path: Path):
    """Scenario 5: Both copies deleted, no recovery copy -> Honest failure -> LOST."""
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)
    cache = RecoveryCache(tmp_path / "empty_cache")

    backup_uc = BackupFileUseCase(gateway, repo)
    verify_uc = VerifyVaultUseCase(gateway, repo, cache)

    source_file = tmp_path / "ephemeral.txt"
    source_file.write_bytes(b"Data lost in transit.")
    backup_res = await backup_uc.execute(source_file)
    rec_id = backup_res.record.id

    # Delete both cloud copies
    p = backup_res.record.primary_ref
    m = backup_res.record.mirror_ref
    gateway.simulate_external_delete(p.channel_id, p.message_id)
    gateway.simulate_external_delete(m.channel_id, m.message_id)

    v_summary = await verify_uc.execute(rec_id)
    assert v_summary.lost_count == 1
    assert repo.get(rec_id).state == HealthState.LOST


@pytest.mark.asyncio
async def test_scenario_manual_only_local_pc_deletion(tmp_path: Path):
    """Rule 2b: Local file deleted on PC -> row shows 'Only in Telegram'

    Verify does NOT delete cloud copies and does NOT auto-download or re-upload.
    """
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)

    backup_uc = BackupFileUseCase(gateway, repo)
    verify_uc = VerifyVaultUseCase(gateway, repo)

    test_file = tmp_path / "user_local_file.pdf"
    test_file.write_bytes(b"User document on desktop.")

    backup_res = await backup_uc.execute(test_file)
    rec_id = backup_res.record.id

    initial_uploads = len(gateway.upload_calls)
    initial_downloads = len(gateway.download_calls)

    # User deletes file from PC!
    test_file.unlink()
    assert not test_file.exists()

    # Run Verify
    v_summary = await verify_uc.execute(rec_id)
    res = v_summary.results[0]

    # Cloud health is still HEALTHY
    assert res.new_state == HealthState.HEALTHY
    # Local status is updated to MISSING ("Only in Telegram")
    assert res.local_status == LocalFileStatus.MISSING
    assert repo.get(rec_id).local_status == LocalFileStatus.MISSING

    # Invariants guaranteed: Zero extra uploads, zero downloads, zero delete calls
    assert len(gateway.upload_calls) == initial_uploads
    assert len(gateway.download_calls) == initial_downloads
    assert len(gateway.delete_message_calls) == 0


@pytest.mark.asyncio
async def test_scenario_manual_only_local_pc_modified(tmp_path: Path):
    """Rule 2b: Local file changed on PC -> row shows 'Changed since last backup'

    Verify does NOT auto-upload.
    """
    gateway = FakeTelegramGateway()
    db_path = tmp_path / "index.db"
    repo = SQLiteVaultRepository(db_path)

    backup_uc = BackupFileUseCase(gateway, repo)
    verify_uc = VerifyVaultUseCase(gateway, repo)

    test_file = tmp_path / "code.py"
    test_file.write_bytes(b"version = 1.0\n")

    backup_res = await backup_uc.execute(test_file)
    rec_id = backup_res.record.id

    # User modifies file locally
    test_file.write_bytes(b"version = 2.0 (modified)\n")

    # Run Verify
    v_summary = await verify_uc.execute(rec_id)
    res = v_summary.results[0]

    # Local status shows CHANGED
    assert res.local_status == LocalFileStatus.CHANGED
    assert repo.get(rec_id).local_status == LocalFileStatus.CHANGED

    # Cloud copies remain 1
    assert len(gateway.upload_calls) == 1
