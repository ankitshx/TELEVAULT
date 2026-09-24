from dataclasses import dataclass
from datetime import datetime, timezone
import logging
from pathlib import Path
import shutil
import tempfile
import time

from televault.application.hashing import calculate_sha256
from televault.domain.ports import AuditLedger, TelegramGateway, VaultRepository
from televault.domain.states import HealthState, VaultMode
from televault.infrastructure.crypto.stream_cipher import decrypt_file_stream

logger = logging.getLogger("televault.drill")


@dataclass(frozen=True)
class RecoveryDrillReport:
    """Verifiable proof of successful recovery drill without data retention."""
    record_id: str
    file_name: str
    file_size: int
    sha256: str
    sha256_matched: bool
    size_matched: bool
    passed: bool
    duration_seconds: float
    message: str


class RecoveryDrillUseCase:
    """Performs a non-destructive recovery drill: download, decrypt, hash, verify, and clean up."""

    def __init__(
        self,
        gateway: TelegramGateway,
        repo: VaultRepository,
        scratch_dir: Path | None = None,
        audit_ledger: AuditLedger | None = None,
    ):
        self.gateway = gateway
        self.repo = repo
        self.scratch_dir = scratch_dir or Path(tempfile.gettempdir()) / "televault_drills"
        self.audit_ledger = audit_ledger

    async def execute(
        self,
        record_id: str | None = None,
        passphrase: str | None = None,
    ) -> RecoveryDrillReport:
        start_time = time.time()

        # Select target record
        if record_id:
            record = self.repo.get(record_id)
        else:
            records = [r for r in self.repo.list_all() if r.state != HealthState.LOST]
            if not records:
                raise ValueError("Vault is empty or has no recoverable files.")
            record = records[0]  # Most recent recoverable record

        ref = record.primary_ref or record.mirror_ref
        if not ref:
            raise ValueError(f"Record {record.id} has no valid Telegram cloud reference.")

        drill_folder = self.scratch_dir / f"drill_{record.id}_{int(start_time)}"
        drill_folder.mkdir(parents=True, exist_ok=True)
        temp_download_path = drill_folder / f"raw_{record.name}"

        try:
            # 1. Download
            await self.gateway.download(ref, dest=temp_download_path)

            # 2. Decrypt if Private Mode
            final_file = temp_download_path
            if record.mode == VaultMode.PRIVATE:
                if not passphrase:
                    raise ValueError(f"File {record.name} is in Private Mode; passphrase is required for drill.")
                decrypted_dir = drill_folder / "decrypted"
                final_file = decrypt_file_stream(temp_download_path, dest_dir=decrypted_dir, passphrase=passphrase)

            # 3. Hash & Size verification
            computed_sha, actual_size = calculate_sha256(final_file)

            sha_matched = (computed_sha.lower() == record.sha256.lower())
            size_matched = (actual_size == record.size)
            passed = sha_matched and size_matched

            duration = round(time.time() - start_time, 2)
            msg = "Recovery drill passed: Byte-for-byte SHA-256 match confirmed." if passed else "Recovery drill failed: Hash or size mismatch."

            if self.audit_ledger:
                self.audit_ledger.append_event(
                    operation="RECOVERY_DRILL",
                    record_id=record.id,
                    result="PASSED" if passed else "FAILED",
                    details=f"Drill for {record.name}: sha_matched={sha_matched}, size_matched={size_matched}, duration={duration}s",
                )

            return RecoveryDrillReport(
                record_id=record.id,
                file_name=record.name,
                file_size=record.size,
                sha256=computed_sha,
                sha256_matched=sha_matched,
                size_matched=size_matched,
                passed=passed,
                duration_seconds=duration,
                message=msg,
            )
        finally:
            # 4. Clean up temporary files completely
            if drill_folder.exists():
                shutil.rmtree(drill_folder, ignore_errors=True)
