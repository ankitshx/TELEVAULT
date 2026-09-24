import os
from pathlib import Path


class TeleVaultConfig:
    """Manages application paths, environments, and local persistence folders."""

    def __init__(self, base_dir: Path | None = None):
        if base_dir:
            self.app_dir = base_dir
        elif os.name == "nt" and "LOCALAPPDATA" in os.environ:
            self.app_dir = Path(os.environ["LOCALAPPDATA"]) / "TeleVault"
        else:
            self.app_dir = Path.home() / ".televault"

        self.db_path = self.app_dir / "televault_index.db"
        self.cache_dir = self.app_dir / "cache"
        self.session_name = str(self.app_dir / "televault_session")

    def ensure_directories(self) -> None:
        self.app_dir.mkdir(parents=True, exist_ok=True)
        self.cache_dir.mkdir(parents=True, exist_ok=True)
