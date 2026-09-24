import logging

logger = logging.getLogger("televault.os.keyring")
SERVICE_NAME = "TeleVault"


class KeyringStore:
    """Manages credentials in OS secure storage (Windows Credential Manager)."""

    def __init__(self, service_name: str = SERVICE_NAME):
        self.service_name = service_name
        self._memory_fallback: dict[str, str] = {}
        self._keyring_available = False

        try:
            import keyring
            self._keyring = keyring
            self._keyring_available = True
        except ImportError:
            logger.warning("keyring module not available; using in-memory store.")
            self._keyring = None

    def set_credential(self, key: str, value: str) -> None:
        if self._keyring_available and self._keyring:
            try:
                self._keyring.set_password(self.service_name, key, value)
                return
            except Exception as e:
                logger.warning(f"Keyring write error ({e}); using memory store.")
        self._memory_fallback[key] = value

    def get_credential(self, key: str) -> str | None:
        if self._keyring_available and self._keyring:
            try:
                val = self._keyring.get_password(self.service_name, key)
                if val is not None:
                    return val
            except Exception as e:
                logger.warning(f"Keyring read error ({e}); falling back to memory store.")
        return self._memory_fallback.get(key)

    def delete_credential(self, key: str) -> None:
        if self._keyring_available and self._keyring:
            try:
                self._keyring.delete_password(self.service_name, key)
            except Exception:
                pass
        self._memory_fallback.pop(key, None)
