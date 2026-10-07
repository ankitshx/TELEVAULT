//! Strongly-typed storage errors and results for TELEVAULT.

use thiserror::Error;

/// Error conditions encountered across the storage abstraction and remote providers.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Underlying I/O error during stream reading, writing, or temp file handling.
    #[error("Storage I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid or malformed storage reference.
    #[error("Invalid storage reference: {0}")]
    InvalidReference(String),

    /// Remote upload failure.
    #[error("Upload failed (retryable={retryable}): {reason}")]
    UploadFailed {
        /// Explanation of failure.
        reason: String,
        /// Whether the upload may be safely retried.
        retryable: bool,
    },

    /// Remote download failure.
    #[error("Download failed (retryable={retryable}): {reason}")]
    DownloadFailed {
        /// Explanation of failure.
        reason: String,
        /// Whether the download may be safely retried.
        retryable: bool,
    },

    /// Remote deletion failure.
    #[error("Delete failed (retryable={retryable}): {reason}")]
    DeleteFailed {
        /// Explanation of failure.
        reason: String,
        /// Whether the deletion may be safely retried.
        retryable: bool,
    },

    /// Remote object integrity or existence verification failure.
    #[error("Remote verification failed: {reason}")]
    VerificationFailed {
        /// Explanation of verification mismatch or failure.
        reason: String,
    },

    /// Failure in the temporary staging area.
    #[error("Temporary storage error: {0}")]
    TempStorage(String),

    /// Remote storage provider unavailable or offline.
    #[error("Storage provider unavailable: {0}")]
    ProviderUnavailable(String),

    /// Invalid state transition for storage object.
    #[error("Invalid storage state: current '{current}', expected '{expected}'")]
    InvalidState {
        /// Current object state.
        current: String,
        /// Expected object state.
        expected: String,
    },

    /// Requested remote object not found.
    #[error("Remote storage object not found: {0}")]
    NotFound(String),

    /// Chunk payload exceeds allowed bounds.
    #[error("Chunk payload exceeds size limit: {actual_bytes} > {limit_bytes}")]
    ChunkTooLarge {
        /// Actual byte size.
        actual_bytes: u64,
        /// Maximum allowed bytes.
        limit_bytes: u64,
    },

    /// Serialization or deserialization error.
    #[error("Storage serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<StorageError> for televault_core::AppError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::Io(e) => televault_core::AppError::Io(e),
            StorageError::NotFound(id) => televault_core::AppError::not_found("StorageObject", id),
            StorageError::InvalidReference(msg) => {
                televault_core::AppError::validation("storage_reference", msg)
            }
            StorageError::VerificationFailed { reason } => televault_core::AppError::Conflict {
                entity: "StorageVerification",
                reason,
            },
            StorageError::InvalidState { current, expected } => {
                televault_core::AppError::InvalidState { current, expected }
            }
            StorageError::ChunkTooLarge {
                actual_bytes,
                limit_bytes,
            } => televault_core::AppError::validation(
                "chunk_size",
                format!("size {actual_bytes} exceeds maximum {limit_bytes}"),
            ),
            StorageError::UploadFailed { reason, .. } => televault_core::AppError::Internal(reason),
            StorageError::DownloadFailed { reason, .. } => {
                televault_core::AppError::Internal(reason)
            }
            StorageError::DeleteFailed { reason, .. } => televault_core::AppError::Internal(reason),
            StorageError::TempStorage(msg) => televault_core::AppError::Internal(msg),
            StorageError::ProviderUnavailable(msg) => televault_core::AppError::Internal(msg),
            StorageError::Serialization(e) => televault_core::AppError::Internal(e.to_string()),
        }
    }
}

/// Convenience result alias for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_error_mapping() {
        let not_found = StorageError::NotFound("obj-123".into());
        let app_err: televault_core::AppError = not_found.into();
        assert_eq!(app_err.error_code(), "NOT_FOUND");

        let state_err = StorageError::InvalidState {
            current: "Pending".into(),
            expected: "Verified".into(),
        };
        let app_err: televault_core::AppError = state_err.into();
        assert_eq!(app_err.error_code(), "INVALID_STATE");
    }
}
