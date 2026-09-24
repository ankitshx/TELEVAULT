from datetime import datetime, timezone
import hashlib
import json
import logging

from televault.domain.entities import ManifestRecord
from televault.domain.ports import AuditLedger, ManifestRepository, VaultRepository

logger = logging.getLogger("televault.manifest")
GENESIS_HASH = "0" * 64


class ManifestService:
    """Manages generation, storage, and cryptographic validation of tamper-evident vault manifests."""

    def __init__(
        self,
        repo: VaultRepository,
        manifest_repo: ManifestRepository,
        vault_id: str = "default",
        audit_ledger: AuditLedger | None = None,
    ):
        self.repo = repo
        self.manifest_repo = manifest_repo
        self.vault_id = vault_id
        self.audit_ledger = audit_ledger

    def get_latest(self) -> ManifestRecord | None:
        return self.manifest_repo.get_latest_manifest()

    def generate_manifest(self) -> ManifestRecord:
        """Create a new tamper-evident manifest generation chained to the previous manifest."""
        latest = self.manifest_repo.get_latest_manifest()
        new_gen = (latest.generation + 1) if latest else 1
        prev_hash = latest.manifest_hash if latest else GENESIS_HASH

        records = self.repo.list_all()
        record_ids = sorted([r.id for r in records])
        total_files = len(records)
        total_bytes = sum(r.size for r in records)

        now = datetime.now(timezone.utc)
        now_str = now.isoformat()

        # Compute payload hash
        payload = f"{self.vault_id}|{new_gen}|{prev_hash}|{total_files}|{total_bytes}|{json.dumps(record_ids)}|{now_str}"
        manifest_hash = hashlib.sha256(payload.encode("utf-8")).hexdigest()

        manifest = ManifestRecord(
            vault_id=self.vault_id,
            generation=new_gen,
            previous_hash=prev_hash,
            manifest_hash=manifest_hash,
            record_ids=record_ids,
            total_files=total_files,
            total_bytes=total_bytes,
            created_at=now,
        )

        self.manifest_repo.save_manifest(manifest)
        return manifest

    def verify_chain(self) -> tuple[bool, str]:
        """Verify the cryptographic integrity of all manifest generations in the vault."""
        manifests = self.manifest_repo.list_manifests()
        if not manifests:
            return True, "No manifests generated yet."

        expected_prev = GENESIS_HASH
        for idx, m in enumerate(manifests):
            if m.previous_hash != expected_prev:
                return False, f"Broken manifest chain at generation {m.generation} (idx {idx}): previous_hash mismatch."

            now_str = m.created_at.isoformat()
            payload = f"{m.vault_id}|{m.generation}|{m.previous_hash}|{m.total_files}|{m.total_bytes}|{json.dumps(m.record_ids)}|{now_str}"
            computed = hashlib.sha256(payload.encode("utf-8")).hexdigest()

            if computed != m.manifest_hash:
                return False, f"VAULT MANIFEST INTEGRITY FAILURE: Generation {m.generation} hash signature corrupted."

            expected_prev = m.manifest_hash

        return True, f"Manifest chain verified: {len(manifests)} generations intact and tamper-evident."
