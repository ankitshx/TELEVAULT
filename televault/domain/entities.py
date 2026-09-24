from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
import uuid

from televault.domain.states import ExistsStatus, HealthState, LocalFileStatus, VaultMode


@dataclass(frozen=True)
class MessageRef:
    channel_id: int
    message_id: int
    document_id: int | None = None


@dataclass(frozen=True)
class VaultChannels:
    primary_id: int
    mirror_id: int


@dataclass(frozen=True)
class ExistsResult:
    status: ExistsStatus
    ref: MessageRef
    document_size: int | None = None
    sha256: str | None = None
    details: str | None = None


@dataclass(frozen=True)
class VaultMessage:
    channel_id: int
    message_id: int
    date: datetime | None = None
    caption: str | None = None
    document_name: str | None = None
    document_size: int | None = None
    parsed_caption: dict[str, Any] | None = None


@dataclass
class FileRecord:
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    name: str = ""
    original_path: str = ""
    sha256: str = ""
    size: int = 0
    mtime: float = 0.0
    state: HealthState = HealthState.PENDING
    primary_ref: MessageRef | None = None
    mirror_ref: MessageRef | None = None
    local_status: LocalFileStatus = LocalFileStatus.PRESENT
    version: int = 1
    mode: VaultMode = VaultMode.ORIGINAL
    tags: list[str] = field(default_factory=list)
    recovery_path: str | None = None
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    updated_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))

    @property
    def is_healthy(self) -> bool:
        return self.state == HealthState.HEALTHY

    @property
    def is_degraded(self) -> bool:
        return self.state == HealthState.DEGRADED

    @property
    def is_lost(self) -> bool:
        return self.state == HealthState.LOST
