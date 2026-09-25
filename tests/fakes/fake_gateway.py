from collections.abc import AsyncIterator
from dataclasses import dataclass, field
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
from typing import Any

from televault.domain.entities import (
    ExistsResult,
    MessageRef,
    VaultChannels,
    VaultMessage,
)
from televault.domain.ports import CodeCallback, PasswordCallback, ProgressCallback
from televault.domain.states import ExistsStatus


@dataclass
class StoredMessage:
    channel_id: int
    message_id: int
    data: bytes
    caption: str
    document_name: str
    document_size: int
    sha256: str
    is_document: bool = True
    date: datetime = field(default_factory=lambda: datetime.now(timezone.utc))


class FakeTelegramGateway:
    """In-memory implementation of TelegramGateway for testing and simulation."""

    def __init__(
        self,
        primary_id: int = -10010001,
        mirror_id: int = -10010002,
        is_premium_account: bool = False,
    ):
        self.primary_id = primary_id
        self.mirror_id = mirror_id
        self._is_premium = is_premium_account
        self.is_logged_in = False

        # Channel storage: {channel_id: {message_id: StoredMessage}}
        self.channels: dict[int, dict[int, StoredMessage]] = {
            self.primary_id: {},
            self.mirror_id: {},
        }
        self.deleted_admin_logs: dict[int, list[VaultMessage]] = {
            self.primary_id: [],
            self.mirror_id: [],
        }

        self._next_msg_id = 100

        # Auditing call logs to test invariants
        self.upload_calls: list[dict[str, Any]] = []
        self.forward_calls: list[dict[str, Any]] = []
        self.download_calls: list[dict[str, Any]] = []
        self.delete_message_calls: list[dict[str, Any]] = []

    async def login(
        self,
        phone: str,
        code_cb: CodeCallback,
        password_cb: PasswordCallback,
    ) -> None:
        self.is_logged_in = True

    async def is_premium(self) -> bool:
        return self._is_premium

    async def ensure_vaults(self) -> VaultChannels:
        if self.primary_id not in self.channels:
            self.channels[self.primary_id] = {}
        if self.mirror_id not in self.channels:
            self.channels[self.mirror_id] = {}
        return VaultChannels(primary_id=self.primary_id, mirror_id=self.mirror_id)

    async def send_message(
        self,
        text: str,
        channel_id: int | None = None,
        reply_to_msg_id: int | None = None,
    ) -> MessageRef:
        target_channel = channel_id or self.primary_id
        if target_channel not in self.channels:
            self.channels[target_channel] = {}

        self._next_msg_id += 1
        msg_id = self._next_msg_id

        stored = StoredMessage(
            channel_id=target_channel,
            message_id=msg_id,
            data=b"",
            caption=text,
            document_name="",
            document_size=0,
            sha256="",
            is_document=False,
        )
        self.channels[target_channel][msg_id] = stored
        return MessageRef(channel_id=target_channel, message_id=msg_id)

    async def upload_document(
        self,
        path: Path,
        caption: str,
        channel_id: int | None = None,
        reply_to_msg_id: int | None = None,
        on_progress: ProgressCallback | None = None,
    ) -> MessageRef:
        """Upload a file as exactly one document to the specified channel (optionally replying)."""
        target_channel = channel_id or self.primary_id
        if not path.exists():
            raise FileNotFoundError(f"File not found: {path}")

        file_bytes = path.read_bytes()
        file_size = len(file_bytes)
        file_sha256 = hashlib.sha256(file_bytes).hexdigest()

        # Simulate streaming progress
        if on_progress:
            chunk_size = max(1024 * 64, file_size // 4) if file_size > 0 else 1
            transferred = 0
            while transferred < file_size:
                transferred = min(transferred + chunk_size, file_size)
                on_progress(transferred, file_size)

        self._next_msg_id += 1
        msg_id = self._next_msg_id

        stored = StoredMessage(
            channel_id=target_channel,
            message_id=msg_id,
            data=file_bytes,
            caption=caption,
            document_name=path.name,
            document_size=file_size,
            sha256=file_sha256,
            is_document=True,
        )

        if target_channel not in self.channels:
            self.channels[target_channel] = {}
        self.channels[target_channel][msg_id] = stored
        self.upload_calls.append({
            "path": str(path),
            "size": file_size,
            "force_document": True,
            "channel_id": target_channel,
            "message_id": msg_id,
            "reply_to": reply_to_msg_id,
        })

        return MessageRef(channel_id=target_channel, message_id=msg_id)

    async def upload_single(
        self,
        path: Path,
        caption: str,
        on_progress: ProgressCallback | None = None,
    ) -> MessageRef:
        """Upload a file as exactly one document to the primary channel."""
        return await self.upload_document(
            path=path,
            caption=caption,
            channel_id=self.primary_id,
            on_progress=on_progress,
        )

    async def forward(self, ref: MessageRef, to_channel: int) -> MessageRef:
        """Emulate server-side message forward without re-upload."""
        src_channel = self.channels.get(ref.channel_id, {})
        if ref.message_id not in src_channel:
            raise ValueError(f"Message {ref.message_id} not found in channel {ref.channel_id}")

        src_msg = src_channel[ref.message_id]
        self._next_msg_id += 1
        new_msg_id = self._next_msg_id

        if to_channel not in self.channels:
            self.channels[to_channel] = {}

        # Forwarded copy points to same document payload
        forwarded = StoredMessage(
            channel_id=to_channel,
            message_id=new_msg_id,
            data=src_msg.data,
            caption=src_msg.caption,
            document_name=src_msg.document_name,
            document_size=src_msg.document_size,
            sha256=src_msg.sha256,
            is_document=src_msg.is_document,
        )
        self.channels[to_channel][new_msg_id] = forwarded

        self.forward_calls.append({
            "from_ref": ref,
            "to_channel": to_channel,
            "new_msg_id": new_msg_id,
        })

        return MessageRef(channel_id=to_channel, message_id=new_msg_id)

    async def exists(self, ref: MessageRef) -> ExistsResult:
        ch = self.channels.get(ref.channel_id, {})
        if ref.message_id not in ch:
            return ExistsResult(status=ExistsStatus.MISSING, ref=ref)

        stored = ch[ref.message_id]
        return ExistsResult(
            status=ExistsStatus.PRESENT,
            ref=ref,
            document_size=stored.document_size,
            sha256=stored.sha256,
        )

    async def download(
        self,
        ref: MessageRef,
        dest: Path,
        on_progress: ProgressCallback | None = None,
    ) -> Path:
        ch = self.channels.get(ref.channel_id, {})
        if ref.message_id not in ch:
            raise FileNotFoundError(f"Message {ref.message_id} not found in channel {ref.channel_id}")

        stored = ch[ref.message_id]
        total = len(stored.data)
        if on_progress:
            on_progress(total, total)

        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(stored.data)

        self.download_calls.append({
            "ref": ref,
            "dest": str(dest),
            "size": total,
        })
        return dest

    async def scan_vault(self, channel: int) -> AsyncIterator[VaultMessage]:
        ch = self.channels.get(channel, {})
        for msg_id, stored in list(ch.items()):
            parsed = None
            if stored.caption and stored.caption.startswith("tv1 "):
                try:
                    parsed = json.loads(stored.caption[4:])
                except json.JSONDecodeError:
                    pass

            yield VaultMessage(
                channel_id=channel,
                message_id=msg_id,
                date=stored.date,
                caption=stored.caption,
                document_name=stored.document_name,
                document_size=stored.document_size,
                parsed_caption=parsed,
            )

    async def read_admin_log_deletions(
        self, channel: int
    ) -> AsyncIterator[VaultMessage]:
        logs = self.deleted_admin_logs.get(channel, [])
        for log_entry in logs:
            yield log_entry

    # --- Test Simulation Helpers (External manipulation to simulate Telegram events) ---

    def simulate_external_delete(self, channel_id: int, message_id: int) -> None:
        """Simulate an external user/admin deleting a message from a channel."""
        ch = self.channels.get(channel_id, {})
        if message_id in ch:
            msg = ch.pop(message_id)
            parsed = None
            if msg.caption and msg.caption.startswith("tv1 "):
                try:
                    parsed = json.loads(msg.caption[4:])
                except Exception:
                    pass

            log_entry = VaultMessage(
                channel_id=channel_id,
                message_id=message_id,
                caption=msg.caption,
                document_name=msg.document_name,
                document_size=msg.document_size,
                parsed_caption=parsed,
            )
            self.deleted_admin_logs.setdefault(channel_id, []).append(log_entry)
