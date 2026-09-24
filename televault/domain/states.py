from enum import Enum


class HealthState(str, Enum):
    PENDING = "PENDING"
    UPLOADING = "UPLOADING"
    HEALTHY = "HEALTHY"
    DEGRADED = "DEGRADED"
    LOST = "LOST"


class LocalFileStatus(str, Enum):
    PRESENT = "PRESENT"
    CHANGED = "CHANGED"
    MISSING = "MISSING"  # "Only in Telegram"


class ExistsStatus(str, Enum):
    PRESENT = "PRESENT"
    MISSING = "MISSING"
    CHANGED = "CHANGED"


class VaultMode(str, Enum):
    ORIGINAL = "original"
    PRIVATE = "private"
