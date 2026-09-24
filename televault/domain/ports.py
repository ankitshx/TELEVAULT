from collections.abc import AsyncIterator, Callable
from pathlib import Path
from typing import Any, Protocol, runtime_checkable

from televault.domain.entities import (
    ExistsResult,
    FileRecord,
    MessageRef,
    VaultChannels,
    VaultMessage,
)
from televault.domain.events import DomainEvent
from televault.domain.states import HealthState

# Callback types
ProgressCallback = Callable[[int, int], None]  # current_bytes, total_bytes
CodeCallback = Callable[[], str]
PasswordCallback = Callable[[], str]


@runtime_checkable
class TelegramGateway(Protocol):
    """Port for Telegram MTProto operations (implemented via Telethon user session)."""

    async def login(
        self,
        phone: str,
        code_cb: CodeCallback,
        password_cb: PasswordCallback,
    ) -> None: ...

    async def is_premium(self) -> bool: ...

    async def ensure_vaults(self) -> VaultChannels:
        """Ensure TeleVault Primary and TeleVault Mirror private channels exist."""
        ...

    async def upload_single(
        self,
        path: Path,
        caption: str,
        on_progress: ProgressCallback | None = None,
    ) -> MessageRef:
        """Upload a file as exactly one Telegram document (force_document=True)."""
        ...

    async def forward(self, ref: MessageRef, to_channel: int) -> MessageRef:
        """Forward an existing message server-side without re-uploading bytes."""
        ...

    async def exists(self, ref: MessageRef) -> ExistsResult:
        """Check if message exists in the channel and verify file integrity."""
        ...

    async def download(
        self,
        ref: MessageRef,
        dest: Path,
        on_progress: ProgressCallback | None = None,
    ) -> Path:
        """Download document bytes directly from Telegram to destination path."""
        ...

    async def scan_vault(self, channel: int) -> AsyncIterator[VaultMessage]:
        """Iterate all messages in a vault channel to reconstruct index."""
        ...

    async def read_admin_log_deletions(
        self, channel: int
    ) -> AsyncIterator[VaultMessage]:
        """Read recent channel admin deletion events to identify lost messages."""
        ...


@runtime_checkable
class VaultRepository(Protocol):
    """Port for local metadata index persistence (implemented via SQLite)."""

    def add(self, rec: FileRecord) -> None: ...

    def get(self, id: str) -> FileRecord: ...

    def find_by_hash(self, sha256: str) -> FileRecord | None: ...

    def list_all(self) -> list[FileRecord]: ...

    def update_state(self, id: str, state: HealthState, **refs: Any) -> None: ...

    def search(self, query: str) -> list[FileRecord]: ...

    def export_snapshot(self, dest: Path) -> None: ...


@runtime_checkable
class EventBus(Protocol):
    """Port for decoupled event publishing and subscription."""

    def publish(self, event: DomainEvent) -> None: ...

    def subscribe(self, kind: type[DomainEvent], handler: Callable[[Any], None]) -> None: ...
