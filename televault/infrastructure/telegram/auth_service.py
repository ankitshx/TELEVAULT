import logging
import os
from pathlib import Path
from typing import Any

from telethon import TelegramClient
from telethon.errors import SessionPasswordNeededError

from televault.domain.entities import VaultChannels
from televault.infrastructure.os.config import TeleVaultConfig
from televault.infrastructure.os.keyring_store import KeyringStore
from televault.infrastructure.telegram.telethon_gateway import TelethonGateway

logger = logging.getLogger("televault.telegram.auth")


class TelegramAuthService:
    """Manages Telegram MTProto authentication, credential storage, and vault bootstrap."""

    def __init__(
        self,
        config: TeleVaultConfig | None = None,
        keyring_store: KeyringStore | None = None,
    ):
        self.config = config or TeleVaultConfig()
        self.config.ensure_directories()
        self.keyring_store = keyring_store or KeyringStore()
        self._client: TelegramClient | None = None
        self._pending_phone: str | None = None
        self._pending_hash: str | None = None

    def get_stored_credentials(self) -> dict[str, str | None]:
        api_id = self.keyring_store.get_credential("api_id") or os.environ.get("TELEVAULT_API_ID")
        api_hash = self.keyring_store.get_credential("api_hash") or os.environ.get("TELEVAULT_API_HASH")
        phone = self.keyring_store.get_credential("phone") or os.environ.get("TELEVAULT_PHONE")
        primary_id = self.keyring_store.get_credential("primary_channel_id")
        mirror_id = self.keyring_store.get_credential("mirror_channel_id")
        return {
            "api_id": api_id,
            "api_hash": api_hash,
            "phone": phone,
            "primary_channel_id": primary_id,
            "mirror_channel_id": mirror_id,
        }

    def save_credentials(self, api_id: str, api_hash: str, phone: str | None = None) -> None:
        self.keyring_store.set_credential("api_id", str(api_id).strip())
        self.keyring_store.set_credential("api_hash", str(api_hash).strip())
        if phone:
            self.keyring_store.set_credential("phone", str(phone).strip())

    def save_channels(self, primary_id: int, mirror_id: int) -> None:
        self.keyring_store.set_credential("primary_channel_id", str(primary_id))
        self.keyring_store.set_credential("mirror_channel_id", str(mirror_id))

    def clear_credentials(self) -> None:
        self.keyring_store.delete_credential("api_id")
        self.keyring_store.delete_credential("api_hash")
        self.keyring_store.delete_credential("phone")
        self.keyring_store.delete_credential("primary_channel_id")
        self.keyring_store.delete_credential("mirror_channel_id")
        session_file = Path(f"{self.config.session_name}.session")
        if session_file.exists():
            try:
                session_file.unlink()
            except Exception:
                pass

    def get_client(self, api_id: int | str | None = None, api_hash: str | None = None) -> TelegramClient:
        if self._client and self._client.is_connected():
            return self._client

        creds = self.get_stored_credentials()
        eff_api_id = api_id or creds.get("api_id")
        eff_api_hash = api_hash or creds.get("api_hash")

        if not eff_api_id or not eff_api_hash:
            raise ValueError("Telegram API ID and API Hash are required.")

        self._client = TelegramClient(self.config.session_name, int(eff_api_id), str(eff_api_hash))
        return self._client

    async def is_authorized(self) -> bool:
        try:
            creds = self.get_stored_credentials()
            if not creds.get("api_id") or not creds.get("api_hash"):
                return False

            client = self.get_client()
            if not client.is_connected():
                await client.connect()
            return await client.is_user_authorized()
        except Exception as e:
            logger.warning(f"Error checking Telegram authorization: {e}")
            return False

    async def get_current_user(self) -> dict[str, Any] | None:
        try:
            if not await self.is_authorized():
                return None
            client = self.get_client()
            me = await client.get_me()
            if not me:
                return None
            return {
                "id": me.id,
                "first_name": getattr(me, "first_name", "") or "",
                "last_name": getattr(me, "last_name", "") or "",
                "username": getattr(me, "username", "") or "",
                "phone": getattr(me, "phone", "") or "",
                "is_premium": bool(getattr(me, "premium", False)),
            }
        except Exception as e:
            logger.warning(f"Failed to fetch current user info: {e}")
            return None

    async def send_login_code(self, api_id: str, api_hash: str, phone: str) -> dict[str, Any]:
        self.save_credentials(api_id, api_hash, phone)
        client = self.get_client(api_id=api_id, api_hash=api_hash)
        if not client.is_connected():
            await client.connect()
        res = await client.send_code_request(phone)
        self._pending_phone = phone
        self._pending_hash = res.phone_code_hash
        return {
            "status": "code_sent",
            "phone": phone,
            "phone_code_hash": res.phone_code_hash,
        }

    async def complete_login(
        self,
        code: str,
        phone_code_hash: str | None = None,
        password: str | None = None,
    ) -> dict[str, Any]:
        creds = self.get_stored_credentials()
        phone = self._pending_phone or creds.get("phone")
        code_hash = phone_code_hash or self._pending_hash

        client = self.get_client()
        if not client.is_connected():
            await client.connect()

        try:
            await client.sign_in(phone=phone, code=code, phone_code_hash=code_hash)
        except SessionPasswordNeededError:
            if not password:
                return {
                    "status": "2fa_required",
                    "message": "Two-factor authentication (cloud password) is required.",
                }
            await client.sign_in(password=password)

        # Bootstrap vault channels
        gateway = TelethonGateway(client)
        channels: VaultChannels = await gateway.ensure_vaults()
        self.save_channels(channels.primary_id, channels.mirror_id)

        user_info = await self.get_current_user()
        return {
            "status": "success",
            "user": user_info,
            "channels": {
                "primary_id": channels.primary_id,
                "mirror_id": channels.mirror_id,
            },
        }

    async def get_live_gateway(self) -> TelethonGateway | None:
        """Return configured TelethonGateway if logged in, else None."""
        if not await self.is_authorized():
            return None

        client = self.get_client()
        creds = self.get_stored_credentials()
        p_id = int(creds["primary_channel_id"]) if creds.get("primary_channel_id") else None
        m_id = int(creds["mirror_channel_id"]) if creds.get("mirror_channel_id") else None

        gateway = TelethonGateway(client, primary_channel_id=p_id, mirror_channel_id=m_id)
        if not p_id or not m_id:
            channels = await gateway.ensure_vaults()
            self.save_channels(channels.primary_id, channels.mirror_id)
        return gateway
