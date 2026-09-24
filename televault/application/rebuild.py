from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from televault.application.caption import parse_tv1_caption
from televault.application.hashing import calculate_sha256
from televault.domain.entities import FileRecord, MessageRef
from televault.domain.ports import TelegramGateway, VaultRepository
from televault.domain.states import HealthState, LocalFileStatus, VaultMode


@dataclass
class RebuildSummary:
    primary_messages_scanned: int = 0
    mirror_messages_scanned: int = 0
    records_reconstructed: int = 0
    records_healthy: int = 0
    records_degraded: int = 0


class RebuildIndexUseCase:
    """Reconstructs the entire local metadata SQLite index from Telegram vault channels

    by parsing self-describing tv1 JSON captions.
    """

    def __init__(self, gateway: TelegramGateway, repo: VaultRepository):
        self.gateway = gateway
        self.repo = repo

    async def execute(self, dry_run: bool = False) -> RebuildSummary:
        vaults = await self.gateway.ensure_vaults()
        summary = RebuildSummary()

        # Aggregate records: {file_id: {data}}
        discovered: dict[str, dict[str, Any]] = {}

        # 1. Scan Primary Channel
        async for msg in self.gateway.scan_vault(vaults.primary_id):
            summary.primary_messages_scanned += 1
            metadata = parse_tv1_caption(msg.caption)
            if not metadata or "id" not in metadata:
                continue

            file_id = metadata["id"]
            if file_id not in discovered:
                discovered[file_id] = {
                    "id": file_id,
                    "name": metadata.get("name", msg.document_name or "unknown"),
                    "sha256": metadata.get("sha256", ""),
                    "size": metadata.get("size", msg.document_size or 0),
                    "mtime": metadata.get("mtime", 0.0),
                    "version": metadata.get("ver", 1),
                    "mode": VaultMode(metadata.get("mode", "original")),
                    "primary_ref": MessageRef(channel_id=vaults.primary_id, message_id=msg.message_id),
                    "mirror_ref": None,
                    "date": msg.date or datetime.now(timezone.utc),
                }
            else:
                discovered[file_id]["primary_ref"] = MessageRef(
                    channel_id=vaults.primary_id, message_id=msg.message_id
                )

        # 2. Scan Mirror Channel
        async for msg in self.gateway.scan_vault(vaults.mirror_id):
            summary.mirror_messages_scanned += 1
            metadata = parse_tv1_caption(msg.caption)
            if not metadata or "id" not in metadata:
                continue

            file_id = metadata["id"]
            if file_id not in discovered:
                discovered[file_id] = {
                    "id": file_id,
                    "name": metadata.get("name", msg.document_name or "unknown"),
                    "sha256": metadata.get("sha256", ""),
                    "size": metadata.get("size", msg.document_size or 0),
                    "mtime": metadata.get("mtime", 0.0),
                    "version": metadata.get("ver", 1),
                    "mode": VaultMode(metadata.get("mode", "original")),
                    "primary_ref": None,
                    "mirror_ref": MessageRef(channel_id=vaults.mirror_id, message_id=msg.message_id),
                    "date": msg.date or datetime.now(timezone.utc),
                }
            else:
                discovered[file_id]["mirror_ref"] = MessageRef(
                    channel_id=vaults.mirror_id, message_id=msg.message_id
                )

        # 3. Build & Save FileRecords
        for file_id, item in discovered.items():
            p_ref = item["primary_ref"]
            m_ref = item["mirror_ref"]

            if p_ref and m_ref:
                state = HealthState.HEALTHY
                summary.records_healthy += 1
            else:
                state = HealthState.DEGRADED
                summary.records_degraded += 1

            record = FileRecord(
                id=file_id,
                name=item["name"],
                original_path="",  # Path unknown after remote rebuild unless verified
                sha256=item["sha256"],
                size=item["size"],
                mtime=item["mtime"],
                state=state,
                primary_ref=p_ref,
                mirror_ref=m_ref,
                local_status=LocalFileStatus.MISSING,  # "Only in Telegram" until checked
                version=item["version"],
                mode=item["mode"],
                created_at=item["date"],
                updated_at=item["date"],
            )

            if not dry_run:
                self.repo.add(record)

            summary.records_reconstructed += 1

        return summary
