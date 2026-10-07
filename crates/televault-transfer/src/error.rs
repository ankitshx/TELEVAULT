//! Typed errors for the TELEVAULT transfer subsystem.

use televault_core::AppError;
use televault_storage::StorageError;
use thiserror::Error;

/// Result type alias for transfer operations.
pub type Result<T> = std::result::Result<T, TransferError>;

/// Strongly typed error enum for transfer engine operations.
#[derive(Debug, Error)]
pub enum TransferError {
    /// Invalid state transition attempted.
    #[error("Invalid transfer state transition from '{from}' to '{to}'")]
    InvalidStateTransition {
        /// Starting state.
        from: String,
        /// Attempted destination state.
        to: String,
    },

    /// Transfer job definition is invalid.
    #[error("Invalid transfer job: {0}")]
    InvalidJob(String),

    /// Source resource could not be found or opened.
    #[error("Transfer source unavailable: {0}")]
    SourceUnavailable(String),

    /// Destination resource could not be found or opened.
    #[error("Transfer destination unavailable: {0}")]
    DestinationUnavailable(String),

    /// Underlying storage provider failure.
    #[error("Storage provider error: {0}")]
    Storage(#[from] StorageError),

    /// Local I/O error during transfer read/write.
    #[error("I/O error during transfer: {0}")]
    Io(#[from] std::io::Error),

    /// Remote or local verification check failed.
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// All configured retry attempts have been exhausted.
    #[error("Transfer retries exhausted after {attempts} attempts. Last error: {last_error}")]
    RetriesExhausted {
        /// Number of attempts made.
        attempts: u32,
        /// Description of the last failure.
        last_error: String,
    },

    /// Transfer operation was cooperatively cancelled.
    #[error("Transfer operation cancelled")]
    Cancelled,

    /// Persistence database error.
    #[error("Database error in transfer engine: {0}")]
    Database(#[from] televault_db::DbError),

    /// Concurrency or lock acquisition failure.
    #[error("Transfer concurrency error: {0}")]
    Concurrency(String),

    /// General unclassified transfer error.
    #[error("Transfer error: {0}")]
    Other(String),
}

impl TransferError {
    /// Determines whether the error represents a transient failure that can be retried.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Storage(storage_err) => match storage_err {
                StorageError::UploadFailed { retryable, .. } => *retryable,
                StorageError::DownloadFailed { retryable, .. } => *retryable,
                StorageError::ProviderUnavailable(_) => true,
                _ => false,
            },
            Self::Io(io_err) => {
                matches!(
                    io_err.kind(),
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::Interrupted
                )
            }
            Self::RetriesExhausted { .. } => false,
            Self::Cancelled => false,
            Self::InvalidStateTransition { .. } => false,
            Self::InvalidJob(_) => false,
            Self::VerificationFailed(_) => false,
            Self::SourceUnavailable(_) => false,
            Self::DestinationUnavailable(_) => false,
            Self::Database(_) => false,
            Self::Concurrency(_) => false,
            Self::Other(_) => false,
        }
    }
}

impl From<TransferError> for AppError {
    fn from(err: TransferError) -> Self {
        match err {
            TransferError::InvalidStateTransition { from, to } => AppError::InvalidState {
                current: from,
                expected: to,
            },
            TransferError::InvalidJob(msg) => AppError::validation("job", msg),
            TransferError::SourceUnavailable(msg) => AppError::not_found("source", msg),
            TransferError::DestinationUnavailable(msg) => AppError::not_found("destination", msg),
            TransferError::Storage(storage_err) => storage_err.into(),
            TransferError::Io(io_err) => AppError::Io(io_err),
            TransferError::VerificationFailed(msg) => AppError::Validation {
                field: "integrity",
                message: format!("Transfer verification failed: {msg}"),
            },
            TransferError::RetriesExhausted {
                attempts,
                last_error,
            } => AppError::Internal(format!(
                "Transfer retries exhausted after {attempts} attempts: {last_error}"
            )),
            TransferError::Cancelled => AppError::Internal("Transfer cancelled by user".into()),
            TransferError::Database(db_err) => db_err.into(),
            TransferError::Concurrency(msg) => AppError::Internal(msg),
            TransferError::Other(msg) => AppError::Internal(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_error_retryability() {
        let cancel = TransferError::Cancelled;
        assert!(!cancel.is_retryable());

        let invalid = TransferError::InvalidJob("bad config".into());
        assert!(!invalid.is_retryable());

        let retryable_storage = TransferError::Storage(StorageError::UploadFailed {
            reason: "temporary network drop".into(),
            retryable: true,
        });
        assert!(retryable_storage.is_retryable());

        let non_retryable_storage = TransferError::Storage(StorageError::UploadFailed {
            reason: "file corrupted".into(),
            retryable: false,
        });
        assert!(!non_retryable_storage.is_retryable());
    }

    #[test]
    fn test_transfer_error_to_app_error() {
        let err = TransferError::Cancelled;
        let app_err: AppError = err.into();
        assert_eq!(app_err.error_code(), "INTERNAL_ERROR");
    }
}
