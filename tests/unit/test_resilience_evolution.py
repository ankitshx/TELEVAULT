from pathlib import Path
import pytest

from televault.application.doctor import VaultDoctorUseCase
from televault.application.manifest import ManifestService
from televault.application.recovery_drill import RecoveryDrillUseCase
from televault.application.transaction import TransactionManager
from televault.domain.entities import FileRecord, MessageRef
from televault.domain.states import DoctorHealthState, HealthState, LocalFileStatus, TransactionState, VaultMode
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.fixture
def repo(tmp_path: Path) -> SQLiteVaultRepository:
    return SQLiteVaultRepository(tmp_path / "test_db.db")


@pytest.fixture
def gateway() -> FakeTelegramGateway:
    return FakeTelegramGateway()


def test_transaction_manager_lifecycle(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway, tmp_path: Path):
    tm = TransactionManager(repo, repo, gateway, audit_ledger=repo)
    dummy_file = tmp_path / "sample.txt"
    dummy_file.write_text("Crash test content")

    # 1. Begin
    tx = tm.begin(dummy_file, VaultMode.ORIGINAL)
    assert tx.state == TransactionState.CREATED
    assert len(repo.list_pending()) == 1

    # 2. Steps
    tm.update_hashed(tx.id, "hash123", 18)
    tx_rec = repo.get_transaction(tx.id)
    assert tx_rec.state == TransactionState.HASHED
    assert tx_rec.sha256 == "hash123"

    p_ref = MessageRef(channel_id=-10010001, message_id=101)
    tm.update_primary_complete(tx.id, p_ref)
    assert repo.get_transaction(tx.id).state == TransactionState.PRIMARY_COMPLETE

    # 3. Complete
    tm.complete(tx.id, record_id="rec_abc")
    assert repo.get_transaction(tx.id).state == TransactionState.COMPLETED
    assert len(repo.list_pending()) == 0


@pytest.mark.asyncio
async def test_transaction_crash_recovery(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway, tmp_path: Path):
    tm = TransactionManager(repo, repo, gateway, audit_ledger=repo)
    dummy_file = tmp_path / "important.docx"
    dummy_file.write_bytes(b"Valuable documents")

    # Simulate transaction interrupted after primary upload
    tx = tm.begin(dummy_file, VaultMode.ORIGINAL)
    tm.update_hashed(tx.id, "docxhash", 18)

    # Store a dummy message in fake gateway primary channel
    from tests.fakes.fake_gateway import StoredMessage
    gateway.channels[gateway.primary_id][105] = StoredMessage(
        channel_id=gateway.primary_id,
        message_id=105,
        data=b"Valuable documents",
        caption="tv1 ...",
        document_name="important.docx",
        document_size=18,
        sha256="docxhash",
    )
    p_ref = MessageRef(channel_id=gateway.primary_id, message_id=105)
    tm.update_primary_complete(tx.id, p_ref)

    assert len(repo.list_pending()) == 1

    # Execute recovery on startup
    actions = await tm.recover_interrupted_transactions()
    assert len(actions) == 1
    assert "Recovered important.docx" in actions[0]

    # Transaction is now completed, file is indexed in repo
    assert len(repo.list_pending()) == 0
    rec = repo.find_by_hash("docxhash")
    assert rec is not None
    assert rec.name == "important.docx"
    assert rec.state == HealthState.HEALTHY


def test_manifest_chain_and_tamper_detection(repo: SQLiteVaultRepository):
    service = ManifestService(repo, repo, vault_id="v_prod")

    # Empty manifests
    is_valid, msg = service.verify_chain()
    assert is_valid

    # Add a file and create manifest 1
    rec = FileRecord(name="file1.txt", sha256="h1", size=100, state=HealthState.HEALTHY)
    repo.add(rec)
    m1 = service.generate_manifest()
    assert m1.generation == 1
    assert m1.previous_hash == "0" * 64

    # Add another file and create manifest 2
    rec2 = FileRecord(name="file2.txt", sha256="h2", size=200, state=HealthState.HEALTHY)
    repo.add(rec2)
    m2 = service.generate_manifest()
    assert m2.generation == 2
    assert m2.previous_hash == m1.manifest_hash

    # Verification passes
    is_valid, msg = service.verify_chain()
    assert is_valid
    assert "2 generations intact" in msg

    # Simulate tampering in database directly
    with repo._connection() as conn:
        conn.execute("UPDATE manifests SET total_bytes = 99999 WHERE generation = 2")

    is_valid_tampered, tamper_msg = service.verify_chain()
    assert not is_valid_tampered
    assert "VAULT MANIFEST INTEGRITY FAILURE" in tamper_msg


def test_audit_ledger_integrity_and_tamper_detection(repo: SQLiteVaultRepository):
    # Log 3 chained events
    e1 = repo.append_event("BACKUP", "rec1", "SUCCESS", "File backed up")
    e2 = repo.append_event("VERIFY", "rec1", "SUCCESS", "Dual copies intact")
    e3 = repo.append_event("RESTORE", "rec1", "SUCCESS", "Restored cleanly")

    is_valid, msg = repo.verify_integrity()
    assert is_valid
    assert "3 chained events intact" in msg

    # Simulate malicious modification of past audit log
    with repo._connection() as conn:
        conn.execute("UPDATE audit_events SET result = 'FAILED' WHERE event_id = ?", (e2.event_id,))

    is_valid_tampered, err_msg = repo.verify_integrity()
    assert not is_valid_tampered
    assert "Tampered audit event" in err_msg


@pytest.mark.asyncio
async def test_vault_doctor_diagnosis_and_repair(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway):
    doctor = VaultDoctorUseCase(
        gateway=gateway,
        repo=repo,
        journal=repo,
        manifest_repo=repo,
        audit_ledger=repo,
    )

    findings = await doctor.diagnose()
    assert any(f.category == "AUTHENTICATION" and f.state == DoctorHealthState.HEALTHY for f in findings)
    assert any(f.category == "CHANNELS" and f.state == DoctorHealthState.HEALTHY for f in findings)
    assert any(f.category == "REDUNDANCY" and f.state == DoctorHealthState.HEALTHY for f in findings)


@pytest.mark.asyncio
async def test_recovery_drill_execution(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway, tmp_path: Path):
    dummy_file = tmp_path / "drill_source.bin"
    dummy_bytes = b"Sample byte content for non-destructive drill"
    dummy_file.write_bytes(dummy_bytes)

    # Backup the file using FakeTelegramGateway
    from televault.application.backup import BackupFileUseCase
    from televault.application.event_bus import SimpleEventBus
    uc = BackupFileUseCase(gateway, repo, SimpleEventBus())
    res = await uc.execute(dummy_file, VaultMode.ORIGINAL)

    # Run Recovery Drill
    drill = RecoveryDrillUseCase(gateway, repo, scratch_dir=tmp_path / "scratch", audit_ledger=repo)
    report = await drill.execute(record_id=res.record.id)

    assert report.passed
    assert report.sha256_matched
    assert report.size_matched
    assert report.file_size == len(dummy_bytes)
    # Temporary scratch folder must be deleted after drill
    assert not (tmp_path / "scratch" / f"drill_{res.record.id}").exists()
