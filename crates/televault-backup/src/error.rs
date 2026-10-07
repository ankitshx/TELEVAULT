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
