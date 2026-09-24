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


class TransactionState(str, Enum):
    """Persistent backup transaction states (Crash-Safe Transactions)."""
    CREATED = "CREATED"
    HASHED = "HASHED"
    UPLOADING = "UPLOADING"
    PRIMARY_COMPLETE = "PRIMARY_COMPLETE"
    MIRROR_COMPLETE = "MIRROR_COMPLETE"
    VERIFYING = "VERIFYING"
    COMPLETED = "COMPLETED"
    FAILED = "FAILED"
    RECOVERABLE = "RECOVERABLE"


class DoctorHealthState(str, Enum):
    """Vault Doctor diagnosis states."""
    HEALTHY = "HEALTHY"
    ATTENTION = "ATTENTION"
    DEGRADED = "DEGRADED"
    RECOVERABLE = "RECOVERABLE"
    LOST = "LOST"


class AIPermission(str, Enum):
    """Explicit capability permissions for AI agents."""
    READ_FILE_METADATA = "READ_FILE_METADATA"
    READ_HASH = "READ_HASH"
    READ_STATUS = "READ_STATUS"
    READ_HEALTH = "READ_HEALTH"
    READ_ACTIVITY = "READ_ACTIVITY"
    READ_AUDIT = "READ_AUDIT"
    READ_VERSION_HISTORY = "READ_VERSION_HISTORY"
    READ_TRANSACTION_STATE = "READ_TRANSACTION_STATE"
    PROPOSE_BACKUP = "PROPOSE_BACKUP"
    PROPOSE_RESTORE = "PROPOSE_RESTORE"
    PROPOSE_HEAL = "PROPOSE_HEAL"
    PROPOSE_REBUILD = "PROPOSE_REBUILD"


class MetadataProtocol(str, Enum):
    """Telegram message caption metadata protocol version."""
    TV1 = "tv1"
    TV2 = "tv2"
