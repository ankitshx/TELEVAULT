import os
from pathlib import Path
import sys

MUTEX_NAME = "TeleVault_Application_SingleInstance_Mutex"


class SingleInstanceLock:
    """Ensures only a single instance of TeleVault runs concurrently on Windows."""

    def __init__(self, lock_file_path: Path | None = None):
        if lock_file_path:
            self.lock_path = lock_file_path
        elif os.name == "nt" and "LOCALAPPDATA" in os.environ:
            self.lock_path = Path(os.environ["LOCALAPPDATA"]) / "TeleVault" / "televault.lock"
        else:
            self.lock_path = Path.home() / ".televault" / "televault.lock"

        self.lock_path.parent.mkdir(parents=True, exist_ok=True)
        self._lock_file = None

    def acquire(self) -> bool:
        """Attempt to acquire single instance lock. Returns True if acquired, False if already running."""
        try:
            if os.name == "nt":
                import msvcrt
                self._lock_file = open(self.lock_path, "w")
                msvcrt.locking(self._lock_file.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                self._lock_file = open(self.lock_path, "w")
                fcntl.flock(self._lock_file, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except (OSError, IOError, ImportError):
            return False

    def release(self) -> None:
        if self._lock_file:
            try:
                if os.name == "nt":
                    import msvcrt
                    msvcrt.locking(self._lock_file.fileno(), msvcrt.LK_UNLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(self._lock_file, fcntl.LOCK_UN)
                self._lock_file.close()
            except Exception:
                pass
            finally:
                self._lock_file = None
