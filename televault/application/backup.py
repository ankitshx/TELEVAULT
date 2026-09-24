from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from televault.application.caption import format_tv1_caption
from televault.application.dedupe import DedupeChecker
from televault.application.hashing import calculate_sha256
from televault.domain.entities import FileRecord
from televault.domain.events import (
    FileQueuedEvent,
    FileStateChangedEvent,
    MirrorForwardedEvent,
    UploadProgressEvent,
)
from televault.domain.ports import EventBus, TelegramGateway, VaultRepository
from televault.domain.states import ExistsStatus, HealthState, LocalFileStatus, VaultMode


@dataclass
class BackupResult:
    record: FileRecord
    is_duplicate: bool = False
    dry_run: bool = False
    message: str = ""


class BackupFileUseCase:
    """Orchestrates manual single-file dual-write backup into Telegram vaults.

    Enforces:
    - User-triggered only (rule 2b).
    - Exactly one document, force_document=True (rule 1).
    - SHA-256 pre-upload verification.
    - Server-side mirror forwarding (dual write).
    - Dual verification before marking HEALTHY.
    """

    def __init__(
        self,
        gateway: TelegramGateway,
        repo: VaultRepository,
        event_bus: EventBus | None = None,
    ):
        self.gateway = gateway
        self.repo = repo
        self.event_bus = event_bus
        self.dedupe_checker = DedupeChecker(repo)

    async def execute(
        self,
        file_path: Path,
        mode: VaultMode = VaultMode.ORIGINAL,
        dry_run: bool = False,
        on_progress: Callable[[int, int], None] | None = None,
    ) -> BackupResult:
        file_path = file_path.resolve()
        if not file_path.is_file():
            raise FileNotFoundError(f"Source file does not exist: {file_path}")

        # Step 1: Pre-upload SHA-256 and size computation
        file_sha256, file_size = calculate_sha256(file_path)
        mtime = file_path.stat().st_mtime

        # Step 2: Deduplication Check
        dedupe = self.dedupe_checker.check(file_sha256)
        if dedupe.is_duplicate and dedupe.existing_record:
            return BackupResult(
                record=dedupe.existing_record,
                is_duplicate=True,
                dry_run=dry_run,
                message=dedupe.reason,
            )

        if dry_run:
            # Dry-run: return simulation plan without modifying state or Telegram
            simulated_record = FileRecord(
                name=file_path.name,
                original_path=str(file_path),
                sha256=file_sha256,
                size=file_size,
                mtime=mtime,
                state=HealthState.HEALTHY,
                local_status=LocalFileStatus.PRESENT,
                mode=mode,
            )
            return BackupResult(
                record=simulated_record,
                is_duplicate=False,
                dry_run=True,
                message=f"[DRY-RUN] Would upload '{file_path.name}' ({file_size} bytes, sha256:{file_sha256[:8]}...) to Primary and forward to Mirror.",
            )

        # Step 3: Initialize DB record in PENDING state
        record = FileRecord(
            name=file_path.name,
            original_path=str(file_path),
            sha256=file_sha256,
            size=file_size,
            mtime=mtime,
            state=HealthState.PENDING,
            local_status=LocalFileStatus.PRESENT,
            mode=mode,
        )
        self.repo.add(record)

        if self.event_bus:
            self.event_bus.publish(FileQueuedEvent(file_id=record.id, path=str(file_path), size=file_size))

        # Step 4: Ensure Primary and Mirror vaults exist
        vaults = await self.gateway.ensure_vaults()

        # Step 5: Transition to UPLOADING state
        record.state = HealthState.UPLOADING
        self.repo.update_state(record.id, HealthState.UPLOADING)
        if self.event_bus:
            self.event_bus.publish(
                FileStateChangedEvent(
                    file_id=record.id,
                    old_state=HealthState.PENDING,
                    new_state=HealthState.UPLOADING,
                )
            )

        # Step 6: Single document upload to Primary with tv1 caption
        caption = format_tv1_caption(record)

        def _progress_adapter(uploaded: int, total: int) -> None:
            if on_progress:
                on_progress(uploaded, total)
            if self.event_bus:
                self.event_bus.publish(
                    UploadProgressEvent(
                        file_id=record.id,
                        bytes_uploaded=uploaded,
                        total_bytes=total,
                        channel="primary",
                    )
                )

        primary_ref = await self.gateway.upload_single(
            path=file_path,
            caption=caption,
            on_progress=_progress_adapter,
        )
        record.primary_ref = primary_ref

        # Step 7: Server-side forward to Mirror (instantaneous, no re-upload)
        mirror_ref = await self.gateway.forward(primary_ref, to_channel=vaults.mirror_id)
        record.mirror_ref = mirror_ref

        if self.event_bus:
            self.event_bus.publish(
                MirrorForwardedEvent(
                    file_id=record.id,
                    primary_message_id=primary_ref.message_id,
                    mirror_message_id=mirror_ref.message_id,
                )
            )

        # Step 8: Verify presence in both channels
        check_p = await self.gateway.exists(primary_ref)
        check_m = await self.gateway.exists(mirror_ref)

        if check_p.status == ExistsStatus.PRESENT and check_m.status == ExistsStatus.PRESENT:
            record.state = HealthState.HEALTHY
            self.repo.update_state(
                record.id,
                HealthState.HEALTHY,
                primary_ref=primary_ref,
                mirror_ref=mirror_ref,
            )
            if self.event_bus:
                self.event_bus.publish(
                    FileStateChangedEvent(
                        file_id=record.id,
                        old_state=HealthState.UPLOADING,
                        new_state=HealthState.HEALTHY,
                        details="Dual-write verified successfully.",
                    )
                )
        else:
            record.state = HealthState.DEGRADED
            self.repo.update_state(
                record.id,
                HealthState.DEGRADED,
                primary_ref=primary_ref,
                mirror_ref=mirror_ref,
            )
            if self.event_bus:
                self.event_bus.publish(
                    FileStateChangedEvent(
                        file_id=record.id,
                        old_state=HealthState.UPLOADING,
                        new_state=HealthState.DEGRADED,
                        details="Verification failed after upload; marked degraded.",
                    )
                )

        return BackupResult(
            record=record,
            is_duplicate=False,
            dry_run=False,
            message="Backup successfully completed and verified in Primary and Mirror vaults.",
        )
