from collections.abc import AsyncIterator, Callable
from pathlib import Path
from typing import Any, Protocol, runtime_checkable

from televault.domain.entities import (
    AuditEvent,
    ExistsResult,
    FileRecord,
    ManifestRecord,
    MessageRef,
    TransactionRecord,
    VaultChannels,
    VaultMessage,
)
from televault.domain.events import DomainEvent
from televault.domain.states import HealthState, TransactionState

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

    async def send_message(
        self,
        text: str,
        channel_id: int | None = None,
        reply_to_msg_id: int | None = None,
    ) -> MessageRef:
        """Post a text card or master announcement message (optionally to a specific channel or reply)."""
        ...

    async def upload_document(
        self,
        path: Path,
        caption: str,
        channel_id: int | None = None,
        reply_to_msg_id: int | None = None,
        on_progress: ProgressCallback | None = None,
    ) -> MessageRef:
        """Upload a file as a Telegram document (optionally in a reply thread)."""
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
class TransactionJournal(Protocol):
    """Port for crash-safe persistent backup transactions."""

    def create(self, tx: TransactionRecord) -> None: ...

    def update_transaction_state(self, tx_id: str, state: TransactionState, **kwargs: Any) -> None: ...

    def update_state(self, tx_id: str, state: TransactionState, **kwargs: Any) -> None: ...

    def get_transaction(self, tx_id: str) -> TransactionRecord | None: ...

    def list_pending(self) -> list[TransactionRecord]: ...

    def complete(self, tx_id: str, record_id: str) -> None: ...

    def fail(self, tx_id: str, error: str) -> None: ...


@runtime_checkable
class ManifestRepository(Protocol):
    """Port for tamper-evident, hash-chained vault manifests."""

    def save_manifest(self, manifest: ManifestRecord) -> None: ...

    def get_latest_manifest(self) -> ManifestRecord | None: ...

    def get_manifest_by_generation(self, generation: int) -> ManifestRecord | None: ...

    def list_manifests(self) -> list[ManifestRecord]: ...


@runtime_checkable
class AuditLedger(Protocol):
    """Port for append-only hash-chained audit event logs."""

    def append_event(
        self,
        operation: str,
        record_id: str | None,
        result: str,
        details: str,
    ) -> AuditEvent: ...

    def list_events(self, limit: int = 100) -> list[AuditEvent]: ...

    def verify_integrity(self) -> tuple[bool, str]: ...


@runtime_checkable
class EventBus(Protocol):
    """Port for decoupled event publishing and subscription."""

    def publish(self, event: DomainEvent) -> None: ...

    def subscribe(self, kind: type[DomainEvent], handler: Callable[[Any], None]) -> None: ...


@runtime_checkable
class AIProvider(Protocol):
    """Port for AI provider abstraction (local heuristics, local LLM, cloud API)."""

    async def analyze(
        self,
        system_prompt: str,
        user_prompt: str,
        context: dict[str, Any],
    ) -> str: ...
