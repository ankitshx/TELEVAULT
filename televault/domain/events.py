from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from televault.domain.states import HealthState


@dataclass(frozen=True)
class DomainEvent:
    timestamp: datetime = datetime.now(timezone.utc)


@dataclass(frozen=True)
class FileQueuedEvent(DomainEvent):
    file_id: str = ""
    path: str = ""
    size: int = 0


@dataclass(frozen=True)
class UploadProgressEvent(DomainEvent):
    file_id: str = ""
    bytes_uploaded: int = 0
    total_bytes: int = 0
    channel: str = "primary"  # "primary" or "mirror"


@dataclass(frozen=True)
class MirrorForwardedEvent(DomainEvent):
    file_id: str = ""
    primary_message_id: int = 0
    mirror_message_id: int = 0


@dataclass(frozen=True)
class FileStateChangedEvent(DomainEvent):
    file_id: str = ""
    old_state: HealthState = HealthState.PENDING
    new_state: HealthState = HealthState.PENDING
    details: str = ""


@dataclass(frozen=True)
class RestoreProgressEvent(DomainEvent):
    file_id: str = ""
    bytes_downloaded: int = 0
    total_bytes: int = 0
    dest_path: str = ""


@dataclass(frozen=True)
class RestoreCompletedEvent(DomainEvent):
    file_id: str = ""
    dest_path: str = ""
    sha256_matched: bool = False


@dataclass(frozen=True)
class VerificationResultEvent(DomainEvent):
    healthy_count: int = 0
    degraded_count: int = 0
    lost_count: int = 0


@dataclass(frozen=True)
class FloodWaitEncounteredEvent(DomainEvent):
    seconds_to_wait: int = 0
    context: str = ""
