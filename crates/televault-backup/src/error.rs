//! Strongly-typed domain errors for backup orchestration.

use televault_core::AppError;

/// Result type alias for televault-backup operations.
pub type Result<T> = std::result::Result<T, BackupError>;

/// Strongly typed errors encountered during backup operations.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    /// Specified backup profile does not exist.
    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),

    /// Profile configuration is invalid.
    #[error("Invalid profile configuration: {0}")]
    InvalidProfile(String),

    /// Filesystem scanning error.
    #[error("Filesystem scan error: {0}")]
    ScanError(String),

    /// Inaccessible file encountered during backup.
    #[error("Inaccessible file at '{path}': {reason}")]
    InaccessibleFile {
        /// Path to the inaccessible file.
        path: String,
        /// Root cause description.
        reason: String,
    },

    /// Path traversal or invalid subpath violation.
    #[error("Path validation error: {0}")]
    PathError(String),

    /// Snapshot operation or consistency error.
    #[error("Snapshot error: {0}")]
    SnapshotError(String),

    /// Manifest generation or validation error.
    #[error("Manifest error: {0}")]
    ManifestError(String),

    /// Cryptographic encryption or key derivation error.
    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    /// Storage provider error.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Transfer engine error.
    #[error("Transfer error: {0}")]
    TransferError(String),

    /// Database persistence error.
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Standard I/O error.
    #[error("I/O error: {0}")]
    Io(String),

    /// Operation was cancelled.
    #[error("Backup operation was cancelled")]
    Cancelled,
}

impl From<std::io::Error> for BackupError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<televault_core::AppError> for BackupError {
    fn from(err: televault_core::AppError) -> Self {
        Self::PathError(err.to_string())
    }
}

impl From<televault_db::DbError> for BackupError {
    fn from(err: televault_db::DbError) -> Self {
        Self::DatabaseError(err.to_string())
    }
}

impl From<televault_manifest::ManifestError> for BackupError {
    fn from(err: televault_manifest::ManifestError) -> Self {
        Self::ManifestError(err.to_string())
    }
}

impl From<televault_crypto::CryptoError> for BackupError {
    fn from(err: televault_crypto::CryptoError) -> Self {
        Self::CryptoError(err.to_string())
    }
}

impl From<televault_storage::StorageError> for BackupError {
    fn from(err: televault_storage::StorageError) -> Self {
        Self::StorageError(err.to_string())
    }
}

impl From<televault_transfer::TransferError> for BackupError {
    fn from(err: televault_transfer::TransferError) -> Self {
        match err {
            televault_transfer::TransferError::Cancelled => Self::Cancelled,
            other => Self::TransferError(other.to_string()),
        }
    }
}

impl From<BackupError> for AppError {
    fn from(err: BackupError) -> Self {
        match err {
            BackupError::ProfileNotFound(msg) => AppError::NotFound {
                entity: "Profile",
                id: msg,
            },
            BackupError::InvalidProfile(msg) => AppError::Validation {
                field: "profile",
                message: msg,
            },
            BackupError::ScanError(msg) => AppError::Internal(format!("Scan error: {msg}")),
            BackupError::InaccessibleFile { path, reason } => {
                AppError::Path(format!("Inaccessible {path}: {reason}"))
            }
            BackupError::PathError(msg) => AppError::Path(msg),
            BackupError::SnapshotError(msg) => AppError::Internal(format!("Snapshot error: {msg}")),
            BackupError::ManifestError(msg) => AppError::Internal(format!("Manifest error: {msg}")),
            BackupError::CryptoError(msg) => AppError::Internal(format!("Crypto error: {msg}")),
            BackupError::StorageError(msg) => AppError::Internal(format!("Storage error: {msg}")),
            BackupError::TransferError(msg) => AppError::Internal(format!("Transfer error: {msg}")),
            BackupError::DatabaseError(msg) => AppError::Internal(format!("Database error: {msg}")),
            BackupError::Io(msg) => AppError::Internal(format!("I/O error: {msg}")),
            BackupError::Cancelled => AppError::Internal("Backup operation was cancelled".into()),
        }
    }
}

/// Result type alias for televault restore operations.
pub type RestoreOpResult<T> = std::result::Result<T, RestoreError>;

/// Strongly typed errors encountered during file restore and backup verification.
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    /// Manifest error or unsupported schema.
    #[error("Manifest error: {0}")]
    Manifest(#[from] televault_manifest::ManifestError),

    /// Database retrieval or consistency error.
    #[error("Database error: {0}")]
    Database(#[from] televault_db::DbError),

    /// Storage provider error during retrieval.
    #[error("Storage error: {0}")]
    Storage(#[from] televault_storage::StorageError),

    /// Transfer engine error during chunk streaming.
    #[error("Transfer error: {0}")]
    Transfer(#[from] televault_transfer::TransferError),

    /// Cryptographic decryption or authentication failure.
    #[error("Cryptographic error: {0}")]
    Crypto(#[from] televault_crypto::CryptoError),

    /// Manifest represents an incomplete or corrupted backup state.
    #[error("Incomplete backup: {reason}")]
    IncompleteBackup {
        /// Explanation of why the backup is incomplete.
        reason: String,
    },

    /// A required chunk is missing from the manifest.
    #[error("Missing chunk in manifest: chunk_id={chunk_id}, index={index}")]
    MissingChunk {
        /// Chunk identifier.
        chunk_id: String,
        /// Chunk index.
        index: u32,
    },

    /// Chunk contains a pending, incomplete storage reference.
    #[error("Pending chunk reference in manifest: chunk_id={chunk_id}, index={index}")]
    PendingChunk {
        /// Chunk identifier.
        chunk_id: String,
        /// Chunk index.
        index: u32,
    },

    /// Remote object unavailable in cloud storage.
    #[error("Remote object unavailable: {reference:?}")]
    RemoteObjectUnavailable {
        /// The missing storage reference.
        reference: televault_manifest::StorageReference,
    },

    /// Decryption failed for a chunk payload.
    #[error("Decryption failed for chunk {chunk_id}: {reason}")]
    DecryptionFailed {
        /// Chunk identifier.
        chunk_id: String,
        /// Reason for failure.
        reason: String,
    },

    /// Decompression failed for a chunk payload.
    #[error("Decompression failed for chunk {chunk_id}: {reason}")]
    DecompressionFailed {
        /// Chunk identifier.
        chunk_id: String,
        /// Reason for failure.
        reason: String,
    },

    /// Whole-file integrity hash does not match the manifest.
    #[error("Integrity mismatch for file '{file_id}': expected {expected}, actual {actual}")]
    IntegrityMismatch {
        /// File identifier.
        file_id: String,
        /// Expected SHA-256 hash.
        expected: String,
        /// Actual calculated SHA-256 hash.
        actual: String,
    },

    /// Total restored byte size does not match original size in manifest.
    #[error(
        "Size mismatch for file '{file_id}': expected {expected} bytes, actual {actual} bytes"
    )]
    SizeMismatch {
        /// File identifier.
        file_id: String,
        /// Expected byte length.
        expected: u64,
        /// Actual byte length.
        actual: u64,
    },

    /// Stored chunk hash does not match chunk integrity metadata.
    #[error(
        "Chunk integrity mismatch for chunk '{chunk_id}': expected {expected}, actual {actual}"
    )]
    ChunkIntegrityMismatch {
        /// Chunk identifier.
        chunk_id: String,
        /// Expected SHA-256 hash.
        expected: String,
        /// Actual calculated SHA-256 hash.
        actual: String,
    },

    /// Missing decryption key or encryption policy conflict.
    #[error("Encryption policy mismatch: {reason}")]
    EncryptionMismatch {
        /// Reason for policy mismatch.
        reason: String,
    },

    /// Destination file already exists and policy is not Overwrite or KeepBoth.
    #[error("Destination conflict for '{path}': file already exists")]
    DestinationConflict {
        /// Path to the conflicting file.
        path: String,
    },

    /// Destination path validation or path traversal failure.
    #[error("Unsafe destination path '{path}': {reason}")]
    UnsafePath {
        /// Target path.
        path: String,
        /// Reason.
        reason: String,
    },

    /// File not found in local SQLite catalog.
    #[error("File '{0}' not found in catalog")]
    FileNotFound(String),

    /// File version not found in catalog.
    #[error("Version '{0}' not found in catalog")]
    VersionNotFound(String),

    /// Snapshot not found in catalog.
    #[error("Snapshot '{0}' not found in catalog")]
    SnapshotNotFound(String),

    /// Standard I/O error during local staging or final write.
    #[error("I/O error: {0}")]
    Io(String),

    /// Restore operation was cancelled by the user.
    #[error("Restore operation was cancelled")]
    Cancelled,
}

impl From<std::io::Error> for RestoreError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<RestoreError> for BackupError {
    fn from(err: RestoreError) -> Self {
        match err {
            RestoreError::Cancelled => BackupError::Cancelled,
            other => BackupError::StorageError(other.to_string()),
        }
    }
}

impl From<RestoreError> for AppError {
    fn from(err: RestoreError) -> Self {
        match err {
            RestoreError::FileNotFound(msg) => AppError::NotFound {
                entity: "File",
                id: msg,
            },
            RestoreError::VersionNotFound(msg) => AppError::NotFound {
                entity: "Version",
                id: msg,
            },
            RestoreError::SnapshotNotFound(msg) => AppError::NotFound {
                entity: "Snapshot",
                id: msg,
            },
            RestoreError::UnsafePath { path: _, reason } => AppError::Path(reason),
            RestoreError::Cancelled => AppError::Internal("Restore operation was cancelled".into()),
            other => AppError::Internal(other.to_string()),
        }
    }
}

/// Strongly typed errors encountered during retention policy evaluation and snapshot pruning.
#[derive(Debug, thiserror::Error)]
pub enum RetentionError {
    /// Specified backup profile does not exist.
    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),

    /// Invalid retention policy configuration.
    #[error("Invalid retention policy: {0}")]
    InvalidPolicy(String),

    /// Profile is currently busy with an active backup or restore operation.
    #[error("Profile '{0}' is currently busy with an active operation")]
    ProfileBusy(String),

    /// Snapshot is currently active or protected from pruning.
    #[error("Snapshot '{0}' is currently active or protected: {1}")]
    SnapshotProtected(String, String),

    /// Snapshot was not found in catalog.
    #[error("Snapshot '{0}' not found in catalog")]
    SnapshotNotFound(String),

    /// Database persistence or transaction error.
    #[error("Database error: {0}")]
    Database(String),

    /// Retention operation was cancelled.
    #[error("Retention operation was cancelled")]
    Cancelled,

    /// Internal retention failure.
    #[error("Retention internal error: {0}")]
    Internal(String),
}

impl From<televault_db::DbError> for RetentionError {
    fn from(err: televault_db::DbError) -> Self {
        match err {
            televault_db::DbError::NotFound {
                entity: "Snapshot",
                id,
            } => Self::SnapshotNotFound(id),
            televault_db::DbError::NotFound {
                entity: "Profile",
                id,
            } => Self::ProfileNotFound(id),
            other => Self::Database(other.to_string()),
        }
    }
}

impl From<RetentionError> for AppError {
    fn from(err: RetentionError) -> Self {
        match err {
            RetentionError::ProfileNotFound(msg) => AppError::NotFound {
                entity: "Profile",
                id: msg,
            },
            RetentionError::SnapshotNotFound(msg) => AppError::NotFound {
                entity: "Snapshot",
                id: msg,
            },
            RetentionError::InvalidPolicy(msg) => AppError::Validation {
                field: "retention_policy",
                message: msg,
            },
            RetentionError::ProfileBusy(msg) => AppError::Conflict {
                entity: "Profile",
                reason: msg,
            },
            RetentionError::SnapshotProtected(id, reason) => AppError::Conflict {
                entity: "Snapshot",
                reason: format!("Snapshot '{id}' is protected: {reason}"),
            },
            RetentionError::Database(msg) => AppError::Internal(format!("Database error: {msg}")),
            RetentionError::Cancelled => {
                AppError::Internal("Retention operation was cancelled".into())
            }
            RetentionError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<RetentionError> for BackupError {
    fn from(err: RetentionError) -> Self {
        match err {
            RetentionError::ProfileNotFound(msg) => BackupError::ProfileNotFound(msg),
            RetentionError::SnapshotNotFound(msg) => {
                BackupError::SnapshotError(format!("Snapshot not found: {msg}"))
            }
            RetentionError::InvalidPolicy(msg) => BackupError::InvalidProfile(msg),
            RetentionError::Database(msg) => BackupError::DatabaseError(msg),
            RetentionError::Cancelled => BackupError::Cancelled,
            other => BackupError::SnapshotError(other.to_string()),
        }
    }
}

/// Result type alias for televault repair operations.
pub type RepairOpResult<T> = std::result::Result<T, RepairError>;

/// Strongly typed errors encountered during remote repair operations.
#[derive(Debug, thiserror::Error)]
pub enum RepairError {
    /// Finding or target is ineligible for repair.
    #[error("Ineligible for repair: {reason}")]
    Ineligible {
        /// Reason why repair is prohibited.
        reason: String,
    },

    /// Strict ownership validation failed.
    #[error("Ownership violation: {reason}")]
    OwnershipViolation {
        /// Explanation of ownership breach.
        reason: String,
    },

    /// Local source file is missing.
    #[error("Local source file not found at '{path}'")]
    SourceNotFound {
        /// Filesystem path.
        path: String,
    },

    /// Local source file cannot be read.
    #[error("Local source file unreadable at '{path}': {reason}")]
    SourceUnreadable {
        /// Filesystem path.
        path: String,
        /// Reason.
        reason: String,
    },

    /// Local source file size does not match expected size in manifest.
    #[error(
        "Local source size mismatch for '{path}': expected {expected} bytes, found {actual} bytes"
    )]
    SourceSizeMismatch {
        /// Filesystem path.
        path: String,
        /// Expected byte length.
        expected: u64,
        /// Actual byte length.
        actual: u64,
    },

    /// Required encryption key is unavailable or missing.
    #[error("Encryption key unavailable: {reason}")]
    EncryptionKeyMissing {
        /// Reason.
        reason: String,
    },

    /// Target manifest is corrupted or invalid.
    #[error("Manifest error: {0}")]
    Manifest(#[from] televault_manifest::ManifestError),

    /// Target entity not found in SQLite catalog.
    #[error("Not found in catalog: {0}")]
    NotFound(String),

    /// Storage provider error during upload or verification.
    #[error("Storage error: {0}")]
    Storage(#[from] televault_storage::StorageError),

    /// Transfer engine error during chunk transmission.
    #[error("Transfer error: {0}")]
    Transfer(#[from] televault_transfer::TransferError),

    /// Cryptographic error during chunk encryption.
    #[error("Cryptographic error: {0}")]
    Crypto(#[from] televault_crypto::CryptoError),

    /// Database persistence error.
    #[error("Database error: {0}")]
    Database(#[from] televault_db::DbError),

    /// Profile is currently busy with an active backup or repair operation.
    #[error("Profile '{0}' is busy with an active operation")]
    ProfileBusy(String),

    /// Remote object post-upload verification failed.
    #[error("Remote verification failed for chunk '{chunk_id}': {reason}")]
    RemoteVerificationFailed {
        /// Chunk identifier.
        chunk_id: String,
        /// Reason.
        reason: String,
    },

    /// Standard I/O error.
    #[error("I/O error: {0}")]
    Io(String),

    /// Repair operation was cancelled.
    #[error("Repair operation was cancelled")]
    Cancelled,

    /// Internal error.
    #[error("Internal repair error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for RepairError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<RepairError> for AppError {
    fn from(err: RepairError) -> Self {
        match err {
            RepairError::Ineligible { reason } => AppError::Validation {
                field: "repair",
                message: reason,
            },
            RepairError::OwnershipViolation { reason } => AppError::Conflict {
                entity: "Ownership",
                reason,
            },
            RepairError::SourceNotFound { path } => AppError::NotFound {
                entity: "SourceFile",
                id: path,
            },
            RepairError::SourceUnreadable { path, reason } => {
                AppError::Path(format!("Unreadable source '{path}': {reason}"))
            }
            RepairError::SourceSizeMismatch {
                path,
                expected,
                actual,
            } => AppError::Validation {
                field: "source_size",
                message: format!(
                    "Size mismatch for '{path}': expected {expected}, actual {actual}"
                ),
            },
            RepairError::EncryptionKeyMissing { reason } => AppError::Validation {
                field: "encryption_key",
                message: reason,
            },
            RepairError::NotFound(msg) => AppError::NotFound {
                entity: "Entity",
                id: msg,
            },
            RepairError::ProfileBusy(msg) => AppError::Conflict {
                entity: "Profile",
                reason: msg,
            },
            RepairError::Cancelled => AppError::Internal("Repair operation was cancelled".into()),
            other => AppError::Internal(other.to_string()),
        }
    }
}

impl From<RepairError> for BackupError {
    fn from(err: RepairError) -> Self {
        match err {
            RepairError::Cancelled => BackupError::Cancelled,
            RepairError::Database(e) => BackupError::DatabaseError(e.to_string()),
            RepairError::Storage(e) => BackupError::StorageError(e.to_string()),
            RepairError::Transfer(e) => BackupError::TransferError(e.to_string()),
            RepairError::Crypto(e) => BackupError::CryptoError(e.to_string()),
            RepairError::Manifest(e) => BackupError::ManifestError(e.to_string()),
            RepairError::Io(e) => BackupError::Io(e),
            other => BackupError::StorageError(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_error_to_app_error() {
        let err = BackupError::ProfileNotFound("prof-1".into());
        let app_err: AppError = err.into();
        assert_eq!(app_err.error_code(), "NOT_FOUND");

        let cancel = BackupError::Cancelled;
        let cancel_app: AppError = cancel.into();
        assert_eq!(cancel_app.error_code(), "INTERNAL_ERROR");
    }
}
