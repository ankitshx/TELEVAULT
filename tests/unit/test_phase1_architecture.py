from pathlib import Path
import pytest

from televault.application.caption_v2 import format_tv2_caption, parse_caption
from televault.domain.entities import (
    ActionProposal,
    AuditEvent,
    BackupReceipt,
    DoctorFinding,
    ManifestRecord,
    TransactionRecord,
)
from televault.domain.states import (
    AIPermission,
    DoctorHealthState,
    MetadataProtocol,
    TransactionState,
    VaultMode,
)
from televault.infrastructure.os.config import TeleVaultConfig


def test_phase1_domain_states_and_entities():
    # Transaction state
    tx = TransactionRecord(
        file_path="C:/docs/file.pdf",
        file_name="file.pdf",
        sha256="abc123hash",
        size=1024,
        state=TransactionState.CREATED,
    )
    assert tx.state == TransactionState.CREATED
    assert tx.manifest_generation == 0

    # Manifest record
    manifest = ManifestRecord(
        vault_id="vault_001",
        generation=1,
        previous_hash="GENESIS",
        manifest_hash="hash001",
        record_ids=["rec_1", "rec_2"],
        total_files=2,
        total_bytes=2048,
    )
    assert manifest.generation == 1
    assert manifest.previous_hash == "GENESIS"

    # Audit event
    event = AuditEvent(
        event_id="evt_01",
        timestamp=tx.created_at,
        operation="BACKUP_INITIATED",
        record_id="rec_1",
        result="SUCCESS",
        details="Primary and mirror upload complete",
        previous_hash="0" * 64,
        current_hash="1" * 64,
    )
    assert event.operation == "BACKUP_INITIATED"

    # Doctor finding
    finding = DoctorFinding(
        category="REDUNDANCY",
        state=DoctorHealthState.DEGRADED,
        title="Mirror Copy Missing",
        what_happened="Mirror telegram channel object was not found",
        why="Message was deleted or forward failed",
        what_is_safe="Primary copy is intact and verifiable",
        recommended_action="Run auto-healing to forward surviving copy",
        action_type="REPAIR_MIRROR",
        target_id="rec_1",
    )
    assert finding.state == DoctorHealthState.DEGRADED
    assert finding.action_type == "REPAIR_MIRROR"


def test_tv2_caption_protocol():
    # Format tv2 in Original mode
    c2 = format_tv2_caption(
        vault_id="v_123",
        record_id="rec_456",
        version=2,
        sha256="deadbeef",
        size=5000,
        timestamp=1774438800.0,
        mode="original",
        name="Contract.pdf",
        parent_hash="parent_beef",
        manifest_gen=3,
    )
    assert c2.startswith("tv2 {")
    assert '"name":"Contract.pdf"' in c2
    assert '"v_id":"v_123"' in c2
    assert '"ver":2' in c2

    # Parse tv2
    parsed = parse_caption(c2)
    assert parsed is not None
    assert parsed["_protocol"] == "tv2"
    assert parsed["v_id"] == "v_123"
    assert parsed["id"] == "rec_456"
    assert parsed["ver"] == 2
    assert parsed["name"] == "Contract.pdf"

    # Private Mode omits name for zero knowledge
    c2_priv = format_tv2_caption(
        vault_id="v_123",
        record_id="rec_789",
        version=1,
        sha256="cipherbeef",
        size=5050,
        timestamp=1774438800.0,
        mode="private",
        name="Secret.pdf",
        key_id="argon_key_1",
    )
    assert "Secret.pdf" not in c2_priv
    parsed_priv = parse_caption(c2_priv)
    assert parsed_priv is not None
    assert "name" not in parsed_priv
    assert parsed_priv["mode"] == "private"

    # Backward compatibility with tv1
    tv1_caption = 'tv1 {"id":"rec_old","name":"Old.txt","sha256":"oldhash","size":100}'
    parsed_tv1 = parse_caption(tv1_caption)
    assert parsed_tv1 is not None
    assert parsed_tv1["_protocol"] == "tv1"
    assert parsed_tv1["id"] == "rec_old"
    assert parsed_tv1["name"] == "Old.txt"

    # Unknown protocol fails safely (does not guess)
    assert parse_caption('tv99 {"id":"xyz"}') is None
    assert parse_caption('invalid caption') is None
    assert parse_caption(None) is None


def test_televault_config_directory_structure(tmp_path: Path):
    cfg = TeleVaultConfig(base_dir=tmp_path / "tele_test")
    cfg.ensure_directories()

    assert cfg.config_dir.exists()
    assert cfg.database_dir.exists()
    assert cfg.cache_dir.exists()
    assert cfg.sessions_dir.exists()
    assert cfg.logs_dir.exists()
    assert cfg.snapshots_dir.exists()
    assert cfg.recovery_dir.exists()
    assert cfg.manifests_dir.exists()
