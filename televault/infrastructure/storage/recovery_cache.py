from pathlib import Path
import shutil

DEFAULT_MAX_CACHE_BYTES = 500 * 1024 * 1024  # 500 MB


class RecoveryCache:
    """Manages temporary local recovery copies of backed-up files for resilience."""

    def __init__(self, cache_dir: Path, max_file_bytes: int = DEFAULT_MAX_CACHE_BYTES):
        self.cache_dir = cache_dir
        self.max_file_bytes = max_file_bytes
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def store(self, source_path: Path, sha256: str) -> Path | None:
        """Store a local recovery copy if the file is below the max size threshold."""
        if not source_path.is_file():
            return None

        file_size = source_path.stat().st_size
        if file_size > self.max_file_bytes:
            return None

        dest_file = self.cache_dir / sha256
        if not dest_file.exists():
            shutil.copy2(source_path, dest_file)

        return dest_file

    def get(self, sha256: str) -> Path | None:
        """Retrieve path to local recovery copy if present."""
        target = self.cache_dir / sha256
        if target.is_file():
            return target
        return None

    def has(self, sha256: str) -> bool:
        return (self.cache_dir / sha256).is_file()

    def remove(self, sha256: str) -> None:
        target = self.cache_dir / sha256
        if target.is_file():
            try:
                target.unlink()
            except OSError:
                pass

    def clear(self) -> None:
        for child in self.cache_dir.iterdir():
            if child.is_file():
                try:
                    child.unlink()
                except OSError:
                    pass
