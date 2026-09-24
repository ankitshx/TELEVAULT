from collections.abc import Callable
from dataclasses import dataclass
import os
from pathlib import Path

from televault.application.hashing import calculate_sha256
from televault.domain.entities import FileRecord
from televault.domain.events import RestoreCompletedEvent, RestoreProgressEvent
from televault.domain.ports import EventBus, TelegramGateway, VaultRepository
from televault.domain.states import ExistsStatus, HealthState


@dataclass
class RestoreResult:
    record: FileRecord
    restored_path: Path | None = None
    success: bool = False
    sha256_matched: bool = False
    dry_run: bool = False
    message: str = ""


class RestoreFileUseCase:
    """Orchestrates file restoration with strict Invariant 4 SHA-256 verification."""

    def __init__(
        self,
        gateway: TelegramGateway,
        repo: VaultRepository,
        event_bus: EventBus | None = None,
    ):
        self.gateway = gateway
        self.repo = repo
        self.event_bus = event_bus

    async def execute(
        self,
        record_id: str,
        dest_dir: Path,
        dry_run: bool = False,
        on_progress: Callable[[int, int], None] | None = None,
    ) -> RestoreResult:
        record = self.repo.get(record_id)
        dest_dir = dest_dir.resolve()
        dest_file = dest_dir / record.name

        if dry_run:
            return RestoreResult(
                record=record,
                restored_path=dest_file,
                success=True,
                sha256_matched=True,
                dry_run=True,
                message=f"[DRY-RUN] Would download '{record.name}' from Telegram and restore to '{dest_file}'.",
            )

        # Determine download source: Primary first, fallback to Mirror
        source_ref = None
        if record.primary_ref:
            check_p = await self.gateway.exists(record.primary_ref)
            if check_p.status == ExistsStatus.PRESENT:
                source_ref = record.primary_ref

        if not source_ref and record.mirror_ref:
            check_m = await self.gateway.exists(record.mirror_ref)
            if check_m.status == ExistsStatus.PRESENT:
                source_ref = record.mirror_ref

        if not source_ref:
            return RestoreResult(
                record=record,
                restored_path=None,
                success=False,
                sha256_matched=False,
                message="Cannot restore: File is missing from both Primary and Mirror vaults.",
            )

        # Download document directly to destination
        dest_dir.mkdir(parents=True, exist_ok=True)

        def _progress_adapter(downloaded: int, total: int) -> None:
            if on_progress:
                on_progress(downloaded, total)
            if self.event_bus:
                self.event_bus.publish(
                    RestoreProgressEvent(
                        file_id=record.id,
                        bytes_downloaded=downloaded,
                        total_bytes=total,
                        dest_path=str(dest_file),
                    )
                )

        downloaded_path = await self.gateway.download(
            ref=source_ref,
            dest=dest_file,
            on_progress=_progress_adapter,
        )

        # Invariant 4: Mandatory SHA-256 integrity verification
        restored_sha256, _ = calculate_sha256(downloaded_path)
        if restored_sha256.lower() != record.sha256.lower():
            # Tampered or corrupted download -> delete corrupted artifact immediately
            if downloaded_path.exists():
                downloaded_path.unlink()

            if self.event_bus:
                self.event_bus.publish(
                    RestoreCompletedEvent(
                        file_id=record.id,
                        dest_path=str(downloaded_path),
                        sha256_matched=False,
                    )
                )

            return RestoreResult(
                record=record,
                restored_path=None,
                success=False,
                sha256_matched=False,
                message=f"Integrity check failed! Expected hash {record.sha256[:12]}..., got {restored_sha256[:12]}...",
            )

        # Restore original modification time
        if record.mtime > 0:
            os.utime(downloaded_path, (record.mtime, record.mtime))

        if self.event_bus:
            self.event_bus.publish(
                RestoreCompletedEvent(
                    file_id=record.id,
                    dest_path=str(downloaded_path),
                    sha256_matched=True,
                )
            )

        return RestoreResult(
            record=record,
            restored_path=downloaded_path,
            success=True,
            sha256_matched=True,
            dry_run=False,
            message="Restore completed successfully with verified SHA-256 match.",
        )
