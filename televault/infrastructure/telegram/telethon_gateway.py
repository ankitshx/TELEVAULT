from collections.abc import AsyncIterator
import logging
from pathlib import Path
from typing import Any

from telethon import TelegramClient
from telethon.tl.functions.channels import CreateChannelRequest
from telethon.tl.types import ChannelAdminLogEventActionDeleteMessage

from televault.application.caption import parse_tv1_caption
from televault.domain.entities import (
    ExistsResult,
    MessageRef,
    VaultChannels,
    VaultMessage,
)
from televault.domain.ports import (
    CodeCallback,
    PasswordCallback,
    ProgressCallback,
    TelegramGateway,
)
from televault.domain.states import ExistsStatus
from televault.infrastructure.telegram.retry import with_flood_wait_retry

logger = logging.getLogger("televault.telegram.gateway")

PRIMARY_CHANNEL_TITLE = "TeleVault Primary"
MIRROR_CHANNEL_TITLE = "TeleVault Mirror"


class TelethonGateway(TelegramGateway):
    """Production Telegram Gateway implementation powered by Telethon MTProto."""

    def __init__(
        self,
        client: TelegramClient,
        primary_channel_id: int | None = None,
        mirror_channel_id: int | None = None,
    ):
        self.client = client
        self.primary_channel_id = primary_channel_id
        self.mirror_channel_id = mirror_channel_id

    async def login(
        self,
        phone: str,
        code_cb: CodeCallback,
        password_cb: PasswordCallback,
    ) -> None:
        if not self.client.is_connected():
            await self.client.connect()

        if await self.client.is_user_authorized():
            return

        await self.client.start(
            phone=phone,
            code_callback=code_cb,
            password=password_cb,
        )

    async def is_premium(self) -> bool:
        me = await self.client.get_me()
        return bool(getattr(me, "premium", False))

    async def ensure_vaults(self) -> VaultChannels:
        """Find or create the private Primary and Mirror channels."""
        if self.primary_channel_id and self.mirror_channel_id:
            return VaultChannels(
                primary_id=self.primary_channel_id,
                mirror_id=self.mirror_channel_id,
            )

        found_primary: int | None = self.primary_channel_id
        found_mirror: int | None = self.mirror_channel_id

        # Scan existing dialogs
        async for dialog in self.client.iter_dialogs():
            if dialog.is_channel:
                if dialog.title == PRIMARY_CHANNEL_TITLE and not found_primary:
                    found_primary = dialog.id
                elif dialog.title == MIRROR_CHANNEL_TITLE and not found_mirror:
                    found_mirror = dialog.id

        # Create missing channels as private channels
        if not found_primary:
            created = await self.client(
                CreateChannelRequest(
                    title=PRIMARY_CHANNEL_TITLE,
                    about="Private secure primary vault for TeleVault backups.",
                    megagroup=False,
                )
            )
            found_primary = created.chats[0].id

        if not found_mirror:
            created = await self.client(
                CreateChannelRequest(
                    title=MIRROR_CHANNEL_TITLE,
                    about="Private redundant mirror vault for TeleVault backups.",
                    megagroup=False,
                )
            )
            found_mirror = created.chats[0].id

        self.primary_channel_id = found_primary
        self.mirror_channel_id = found_mirror

        return VaultChannels(primary_id=found_primary, mirror_id=found_mirror)

    async def upload_single(
        self,
        path: Path,
        caption: str,
        on_progress: ProgressCallback | None = None,
    ) -> MessageRef:
        """Upload a file as exactly one document (force_document=True) to the Primary vault."""
        if not self.primary_channel_id:
            await self.ensure_vaults()

        target_entity = await self.client.get_input_entity(self.primary_channel_id)

        def _telethon_progress(current: int, total: int) -> None:
            if on_progress:
                on_progress(current, total)

        async def _upload() -> Any:
            return await self.client.send_file(
                entity=target_entity,
                file=str(path),
                caption=caption,
                force_document=True,
                progress_callback=_telethon_progress,
            )

        sent_msg = await with_flood_wait_retry(_upload)
        doc_id = sent_msg.document.id if sent_msg.document else None

        return MessageRef(
            channel_id=self.primary_channel_id,  # type: ignore
            message_id=sent_msg.id,
            document_id=doc_id,
        )

    async def forward(self, ref: MessageRef, to_channel: int) -> MessageRef:
        """Server-side forward from source channel to destination channel."""
        dest_entity = await self.client.get_input_entity(to_channel)
        src_entity = await self.client.get_input_entity(ref.channel_id)

        async def _forward() -> Any:
            msgs = await self.client.forward_messages(
                entity=dest_entity,
                messages=ref.message_id,
                from_peer=src_entity,
            )
            return msgs[0] if isinstance(msgs, list) else msgs

        forwarded_msg = await with_flood_wait_retry(_forward)
        doc_id = forwarded_msg.document.id if forwarded_msg.document else None

        return MessageRef(
            channel_id=to_channel,
            message_id=forwarded_msg.id,
            document_id=doc_id,
        )

    async def exists(self, ref: MessageRef) -> ExistsResult:
        """Check whether message exists and return its document size."""
        try:
            entity = await self.client.get_input_entity(ref.channel_id)
            msgs = await self.client.get_messages(entity, ids=[ref.message_id])
            if not msgs or msgs[0] is None:
                return ExistsResult(status=ExistsStatus.MISSING, ref=ref)

            msg = msgs[0]
            if not msg.document:
                return ExistsResult(
                    status=ExistsStatus.CHANGED,
                    ref=ref,
                    details="Message found but document is missing",
                )

            parsed = parse_tv1_caption(msg.text or msg.caption)
            expected_sha = parsed.get("sha256") if parsed else None

            return ExistsResult(
                status=ExistsStatus.PRESENT,
                ref=ref,
                document_size=msg.document.size,
                sha256=expected_sha,
            )
        except Exception as e:
            logger.warning(f"Error checking exists for {ref}: {e}")
            return ExistsResult(status=ExistsStatus.MISSING, ref=ref, details=str(e))

    async def download(
        self,
        ref: MessageRef,
        dest: Path,
        on_progress: ProgressCallback | None = None,
    ) -> Path:
        """Stream download document bytes directly to local destination."""
        entity = await self.client.get_input_entity(ref.channel_id)
        msgs = await self.client.get_messages(entity, ids=[ref.message_id])
        if not msgs or msgs[0] is None:
            raise FileNotFoundError(f"Message {ref.message_id} not found in channel {ref.channel_id}")

        msg = msgs[0]
        dest.parent.mkdir(parents=True, exist_ok=True)

        def _telethon_progress(current: int, total: int) -> None:
            if on_progress:
                on_progress(current, total)

        async def _download() -> Any:
            return await self.client.download_media(
                msg,
                file=str(dest),
                progress_callback=_telethon_progress,
            )

        await with_flood_wait_retry(_download)
        return dest

    async def scan_vault(self, channel: int) -> AsyncIterator[VaultMessage]:
        """Iterate all messages in a vault channel to reconstruct index."""
        entity = await self.client.get_input_entity(channel)
        async for msg in self.client.iter_messages(entity):
            if msg.document:
                caption = msg.text or msg.caption or ""
                doc_name = msg.file.name if msg.file else None
                yield VaultMessage(
                    channel_id=channel,
                    message_id=msg.id,
                    date=msg.date,
                    caption=caption,
                    document_name=doc_name,
                    document_size=msg.document.size,
                    parsed_caption=parse_tv1_caption(caption),
                )

    async def read_admin_log_deletions(
        self, channel: int
    ) -> AsyncIterator[VaultMessage]:
        """Scan channel admin log for deleted message events within the ~48h window."""
        entity = await self.client.get_input_entity(channel)
        async for event in self.client.iter_admin_log(entity, delete=True):
            if isinstance(event.action, ChannelAdminLogEventActionDeleteMessage):
                deleted_msg = event.action.message
                if getattr(deleted_msg, "document", None):
                    caption = deleted_msg.text or deleted_msg.caption or ""
                    yield VaultMessage(
                        channel_id=channel,
                        message_id=deleted_msg.id,
                        date=deleted_msg.date,
                        caption=caption,
                        document_name=deleted_msg.file.name if deleted_msg.file else None,
                        document_size=deleted_msg.document.size,
                        parsed_caption=parse_tv1_caption(caption),
                    )
