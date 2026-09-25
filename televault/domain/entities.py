from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
import uuid

from televault.domain.states import (
    AIPermission,
    DoctorHealthState,
    ExistsStatus,
    HealthState,
    LocalFileStatus,
    MetadataProtocol,
    TransactionState,
    VaultMode,
)


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
class FilePart:
    """Metadata for an individual chunk part of a large multi-part file (> 2 GB)."""
    part_number: int  # 1-indexed (e.g., 1, 2, 3...)
    total_parts: int
    size: int
    sha256: str
    primary_ref: MessageRef | None = None
    mirror_ref: MessageRef | None = None


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
    parts: list[FilePart] = field(default_factory=list)
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

    @property
    def is_multipart(self) -> bool:
        return len(self.parts) > 1


@dataclass
class TransactionRecord:
    """Persistent backup transaction for crash safety."""
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    file_path: str = ""
    file_name: str = ""
    sha256: str = ""
    size: int = 0
    mode: VaultMode = VaultMode.ORIGINAL
    state: TransactionState = TransactionState.CREATED
    primary_ref: MessageRef | None = None
    mirror_ref: MessageRef | None = None
    record_id: str | None = None
    manifest_generation: int = 0
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    updated_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    error_message: str | None = None


@dataclass(frozen=True)
class ManifestRecord:
    """Tamper-evident, hash-chained vault manifest snapshot."""
    vault_id: str
    generation: int
    previous_hash: str
    manifest_hash: str
    record_ids: list[str]
    total_files: int
    total_bytes: int
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))


@dataclass(frozen=True)
class AuditEvent:
    """Cryptographically hash-chained append-only event ledger entry."""
    event_id: str
    timestamp: datetime
    operation: str
    record_id: str | None
    result: str
    details: str
    previous_hash: str
    current_hash: str


@dataclass(frozen=True)
class BackupReceipt:
    """Verifiable proof of successful backup."""
    backup_id: str
    timestamp: datetime
    files: list[str]
    total_size: int
    primary_message_id: int | None
    mirror_message_id: int | None
    sha256: str
    manifest_generation: int
    sha256_verified: bool = True


@dataclass(frozen=True)
class DoctorFinding:
    """Structured diagnostic finding produced by Vault Doctor."""
    category: str
    state: DoctorHealthState
    title: str
    what_happened: str
    why: str
    what_is_safe: str
    recommended_action: str
    action_type: str | None = None  # e.g., 'REPAIR_MIRROR', 'REBUILD_INDEX', 'NONE'
    target_id: str | None = None


@dataclass(frozen=True)
class ActionProposal:
    """Structured action proposed by AI or Doctor requiring user confirmation."""
    proposal_id: str
    agent_name: str
    action_title: str
    why: str
    what_will_change: str
    what_will_not_change: str
    action_type: str = "PROPOSAL"
    payload: dict[str, Any] = field(default_factory=dict)
    safe: bool = True
    reversible: bool = True
