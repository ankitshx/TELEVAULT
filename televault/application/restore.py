from collections.abc import Callable
from dataclasses import dataclass
import os
from pathlib import Path
import tempfile

from televault.application.chunking import merge_chunks
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
    """Orchestrates file restoration with strict Invariant 4 SHA-256 verification.
    
    Supports both standard single-document downloads and seamless multi-part chunk
    assembly for files > 2 GB.
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
            strategy = (
                f"Multi-part download & merge ({len(record.parts)} parts)"
                if record.is_multipart
                else "Single document download"
            )
            return RestoreResult(
                record=record,
                restored_path=dest_file,
                success=True,
                sha256_matched=True,
                dry_run=True,
                message=f"[DRY-RUN] Strategy: {strategy}. Would download '{record.name}' from Telegram and restore to '{dest_file}'.",
            )

        dest_dir.mkdir(parents=True, exist_ok=True)

        # ---------------------------------------------------------
        # BRANCH A: Standard Single File Restore (<= 2 GB)
        # ---------------------------------------------------------
        if not record.is_multipart:
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

        # ---------------------------------------------------------
        # BRANCH B: Multi-Part Restore & Merge (> 2 GB)
        # ---------------------------------------------------------
        staging_dir = Path(tempfile.gettempdir()) / "televault_staging"
        staging_dir.mkdir(parents=True, exist_ok=True)
        chunk_files: list[Path] = []
        total_downloaded = 0

        try:
            for part in sorted(record.parts, key=lambda p: p.part_number):
                source_ref = None
                if part.primary_ref:
                    chk_p = await self.gateway.exists(part.primary_ref)
                    if chk_p.status == ExistsStatus.PRESENT:
                        source_ref = part.primary_ref

                if not source_ref and part.mirror_ref:
                    chk_m = await self.gateway.exists(part.mirror_ref)
                    if chk_m.status == ExistsStatus.PRESENT:
                        source_ref = part.mirror_ref

                if not source_ref:
                    return RestoreResult(
                        record=record,
                        restored_path=None,
                        success=False,
                        sha256_matched=False,
                        message=f"Cannot restore: Chunk part {part.part_number}/{part.total_parts} missing from vaults.",
                    )

                part_dest = staging_dir / f"{record.id}_restore_part_{part.part_number}.tmp"

                def _part_progress(part_done: int, part_total: int) -> None:
                    curr = total_downloaded + part_done
                    if on_progress:
                        on_progress(curr, record.size)
                    if self.event_bus:
                        self.event_bus.publish(
                            RestoreProgressEvent(
                                file_id=record.id,
                                bytes_downloaded=curr,
                                total_bytes=record.size,
                                dest_path=str(dest_file),
                            )
                        )

                await self.gateway.download(
                    ref=source_ref,
                    dest=part_dest,
                    on_progress=_part_progress,
                )

                # Verify chunk checksum
                part_sha256, _ = calculate_sha256(part_dest)
                if part_sha256.lower() != part.sha256.lower():
                    if part_dest.exists():
                        part_dest.unlink()
                    return RestoreResult(
                        record=record,
                        restored_path=None,
                        success=False,
                        sha256_matched=False,
                        message=f"Integrity check failed on part {part.part_number}! Expected {part.sha256[:8]}, got {part_sha256[:8]}.",
                    )

                chunk_files.append(part_dest)
                total_downloaded += part.size

            # Concatenate chunks into final dest_file
            merged_sha256 = merge_chunks(chunk_files, dest_file)
            if merged_sha256.lower() != record.sha256.lower():
                if dest_file.exists():
                    dest_file.unlink()

                if self.event_bus:
                    self.event_bus.publish(
                        RestoreCompletedEvent(
                            file_id=record.id,
                            dest_path=str(dest_file),
                            sha256_matched=False,
                        )
                    )

                return RestoreResult(
                    record=record,
                    restored_path=None,
                    success=False,
                    sha256_matched=False,
                    message=f"Integrity check failed on merged file! Expected {record.sha256[:12]}, got {merged_sha256[:12]}...",
                )

            # Restore original modification time
            if record.mtime > 0:
                os.utime(dest_file, (record.mtime, record.mtime))

            if self.event_bus:
                self.event_bus.publish(
                    RestoreCompletedEvent(
                        file_id=record.id,
                        dest_path=str(dest_file),
                        sha256_matched=True,
                    )
                )

            return RestoreResult(
                record=record,
                restored_path=dest_file,
                success=True,
                sha256_matched=True,
                dry_run=False,
                message=f"Multi-part restore ({len(chunk_files)} parts) completed successfully with verified SHA-256 match.",
            )
        finally:
            for cf in chunk_files:
                if cf.exists():
                    try:
                        cf.unlink()
                    except OSError:
                        pass
            if staging_dir.exists() and not any(staging_dir.iterdir()):
                try:
                    staging_dir.rmdir()
                except OSError:
                    pass
