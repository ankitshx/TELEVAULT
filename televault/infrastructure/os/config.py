import os
from pathlib import Path


class TeleVaultConfig:
    """Manages application paths, environments, and local persistence folders.
    
    Standard Windows 11 application directory layout:
      %LOCALAPPDATA%\\TeleVault\\
        config\\
        database\\
        cache\\
        sessions\\
        logs\\
        snapshots\\
        recovery\\
        manifests\\
    """

    def __init__(self, base_dir: Path | None = None, vault_id: str | None = None):
        if base_dir:
            self.app_dir = base_dir
        elif os.name == "nt" and "LOCALAPPDATA" in os.environ:
            self.app_dir = Path(os.environ["LOCALAPPDATA"]) / "TeleVault"
        else:
            self.app_dir = Path.home() / ".televault"

        self.vault_id = vault_id or "default"

        # Subdirectories
        self.config_dir = self.app_dir / "config"
        self.database_dir = self.app_dir / "database"
        self.cache_dir = self.app_dir / "cache"
        self.sessions_dir = self.app_dir / "sessions"
        self.logs_dir = self.app_dir / "logs"
        self.snapshots_dir = self.app_dir / "snapshots"
        self.recovery_dir = self.app_dir / "recovery"
        self.manifests_dir = self.app_dir / "manifests"

        # Backward compatibility for database location
        legacy_db = self.app_dir / "televault_index.db"
        if legacy_db.exists():
            self.db_path = legacy_db
        else:
            self.db_path = self.database_dir / "televault_index.db"

        # Backward compatibility for session location
        legacy_session = self.app_dir / "televault_session.session"
        if legacy_session.exists():
            self.session_name = str(self.app_dir / "televault_session")
        else:
            self.session_name = str(self.sessions_dir / "televault_session")

    def ensure_directories(self) -> None:
        """Create all required persistent and operational subdirectories."""
        for directory in [
            self.app_dir,
            self.config_dir,
            self.database_dir,
            self.cache_dir,
            self.sessions_dir,
            self.logs_dir,
            self.snapshots_dir,
            self.recovery_dir,
            self.manifests_dir,
        ]:
            directory.mkdir(parents=True, exist_ok=True)
