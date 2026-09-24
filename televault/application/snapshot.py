from datetime import datetime, timezone
from pathlib import Path

from televault.domain.ports import TelegramGateway, VaultRepository


class SnapshotUseCase:
    """Exports SQLite index snapshot and uploads it to the Primary vault channel."""

    def __init__(
        self,
        gateway: TelegramGateway,
        repo: VaultRepository,
        temp_dir: Path | None = None,
    ):
        self.gateway = gateway
        self.repo = repo
        self.temp_dir = temp_dir or Path.cwd() / "cache"

    async def execute(self, dry_run: bool = False) -> Path:
        self.temp_dir.mkdir(parents=True, exist_ok=True)
        timestamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
        snapshot_filename = f"televault_index_{timestamp}.db"
        snapshot_path = self.temp_dir / snapshot_filename

        # Export local SQLite database
        self.repo.export_snapshot(snapshot_path)

        if dry_run:
            return snapshot_path

        # Upload to Primary channel
        caption = f"televault_snapshot {timestamp}"
        await self.gateway.upload_single(snapshot_path, caption=caption)

        return snapshot_path
