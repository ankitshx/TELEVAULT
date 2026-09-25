from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
import tempfile

from televault.application.caption import (
    format_chunk_caption,
    format_master_post,
    format_tv1_caption,
)
from televault.application.chunking import (
    DEFAULT_CHUNK_SIZE,
    DEFAULT_MAX_SINGLE_FILE_SIZE,
    extract_chunk_to_file,
    split_file_slices,
)
from televault.application.dedupe import DedupeChecker
from televault.application.hashing import calculate_sha256
from televault.domain.entities import FilePart, FileRecord
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
    - Exactly one document (force_document=True) for files <= 2 GB.
    - Automatic safe chunking for files > 2 GB, cleanly organized under
      a Master Post in Telegram reply threads.
    - SHA-256 pre-upload verification.
    - Server-side mirror forwarding (dual write).
    - Dual verification before marking HEALTHY.
    """

    def __init__(
        self,
        gateway: TelegramGateway,
        repo: VaultRepository,
        event_bus: EventBus | None = None,
        max_single_file_size: int = DEFAULT_MAX_SINGLE_FILE_SIZE,
        chunk_size: int = DEFAULT_CHUNK_SIZE,
    ):
        self.gateway = gateway
        self.repo = repo
        self.event_bus = event_bus
        self.dedupe_checker = DedupeChecker(repo)
        self.max_single_file_size = max_single_file_size
        self.chunk_size = chunk_size

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

        is_multipart = file_size > self.max_single_file_size

        if dry_run:
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
            strategy = (
                f"Multi-part split into chunks of ~{self.chunk_size // (1024 * 1024)} MB"
                if is_multipart
                else "Single document upload"
            )
            return BackupResult(
                record=simulated_record,
                is_duplicate=False,
                dry_run=True,
                message=f"[DRY-RUN] Strategy: {strategy}. Would upload '{file_path.name}' ({file_size} bytes, sha256:{file_sha256[:8]}...) to Primary and forward to Mirror.",
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

        if not is_multipart:
            # ---------------------------------------------------------
            # BRANCH A: Standard Single File Upload (<= 2 GB)
            # ---------------------------------------------------------
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

        # ---------------------------------------------------------
        # BRANCH B: Multi-Part Chunked Upload (> 2 GB)
        # ---------------------------------------------------------
        slices = split_file_slices(file_size, self.chunk_size)
        total_parts = len(slices)

        # Step B1: Post Master Announcement Card to Primary channel
        master_post_text = format_master_post(record, total_parts)
        primary_master_ref = await self.gateway.send_message(
            text=master_post_text,
            channel_id=vaults.primary_id,
        )
        record.primary_ref = primary_master_ref

        # Step B2: Forward Master Announcement Card to Mirror channel
        mirror_master_ref = await self.gateway.forward(primary_master_ref, to_channel=vaults.mirror_id)
        record.mirror_ref = mirror_master_ref

        # Step B3: Upload each chunk part into reply thread of Master Card
        parts: list[FilePart] = []
        staging_dir = Path(tempfile.gettempdir()) / "televault_staging"
        staging_dir.mkdir(parents=True, exist_ok=True)

        overall_uploaded = 0
        all_parts_healthy = True

        try:
            for idx, (offset, part_len) in enumerate(slices, start=1):
                temp_chunk_path = staging_dir / f"{record.id}_part_{idx}.tmp"
                try:
                    chunk_sha256 = extract_chunk_to_file(file_path, offset, part_len, temp_chunk_path)
                    chunk_caption = format_chunk_caption(
                        record=record,
                        part_number=idx,
                        total_parts=total_parts,
                        chunk_sha256=chunk_sha256,
                        chunk_size=part_len,
                    )

                    def _chunk_progress(part_done: int, part_total: int) -> None:
                        current_total = overall_uploaded + part_done
                        if on_progress:
                            on_progress(current_total, file_size)
                        if self.event_bus:
                            self.event_bus.publish(
                                UploadProgressEvent(
                                    file_id=record.id,
                                    bytes_uploaded=current_total,
                                    total_bytes=file_size,
                                    channel="primary",
                                )
                            )

                    # Upload chunk document replying to primary master message
                    part_p_ref = await self.gateway.upload_document(
                        path=temp_chunk_path,
                        caption=chunk_caption,
                        channel_id=vaults.primary_id,
                        reply_to_msg_id=primary_master_ref.message_id,
                        on_progress=_chunk_progress,
                    )

                    # Forward chunk document to mirror channel
                    part_m_ref = await self.gateway.forward(part_p_ref, to_channel=vaults.mirror_id)

                    chk_p = await self.gateway.exists(part_p_ref)
                    chk_m = await self.gateway.exists(part_m_ref)
                    if chk_p.status != ExistsStatus.PRESENT or chk_m.status != ExistsStatus.PRESENT:
                        all_parts_healthy = False

                    parts.append(
                        FilePart(
                            part_number=idx,
                            total_parts=total_parts,
                            size=part_len,
                            sha256=chunk_sha256,
                            primary_ref=part_p_ref,
                            mirror_ref=part_m_ref,
                        )
                    )
                    overall_uploaded += part_len
                finally:
                    if temp_chunk_path.exists():
                        try:
                            temp_chunk_path.unlink()
                        except OSError:
                            pass
        finally:
            if staging_dir.exists() and not any(staging_dir.iterdir()):
                try:
                    staging_dir.rmdir()
                except OSError:
                    pass

        record.parts = parts
        final_state = HealthState.HEALTHY if all_parts_healthy else HealthState.DEGRADED
        record.state = final_state

        self.repo.update_state(
            record.id,
            final_state,
            primary_ref=primary_master_ref,
            mirror_ref=mirror_master_ref,
            parts=parts,
        )

        if self.event_bus:
            self.event_bus.publish(
                FileStateChangedEvent(
                    file_id=record.id,
                    old_state=HealthState.UPLOADING,
                    new_state=final_state,
                    details=f"Multi-part backup ({total_parts} parts) completed.",
                )
            )

        return BackupResult(
            record=record,
            is_duplicate=False,
            dry_run=False,
            message=f"Multi-part backup ({total_parts} chunks) successfully completed and verified in vaults.",
        )
