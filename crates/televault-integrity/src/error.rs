//! Typed integrity verification errors for TELEVAULT.

use televault_core::error::AppError;
use thiserror::Error;

/// Error conditions encountered during integrity verification.
#[derive(Debug, Error)]
pub enum IntegrityError {
    /// Underlying I/O error during stream hashing.
    #[error("Integrity I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Checksum or cryptographic hash mismatch.
    #[error("Hash mismatch: expected '{expected}', calculated '{calculated}'")]
    HashMismatch {
        /// Expected hash hex.
        expected: String,
        /// Actual calculated hash hex.
        calculated: String,
    },

    /// Object size mismatch.
    #[error("Size mismatch: expected {expected} bytes, found {actual} bytes")]
    SizeMismatch {
        /// Expected byte size.
        expected: u64,
        /// Actual byte size.
        actual: u64,
    },

    /// Operation cancelled during streaming verification.
    #[error("Verification cancelled by user")]
    Cancelled,

    /// Serialization failure.
    #[error("Integrity serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error.
    #[error("Integrity internal error: {0}")]
    Internal(String),
}

impl From<IntegrityError> for AppError {
    fn from(err: IntegrityError) -> Self {
        match err {
            IntegrityError::Io(e) => AppError::Io(e),
            IntegrityError::HashMismatch {
                expected,
                calculated,
            } => AppError::validation(
                "integrity_hash",
                format!("expected '{expected}', found '{calculated}'"),
            ),
            IntegrityError::SizeMismatch { expected, actual } => AppError::validation(
                "integrity_size",
                format!("expected {expected} bytes, found {actual} bytes"),
            ),
            IntegrityError::Cancelled => {
                AppError::validation("verification", "Verification was cancelled by user")
            }
            IntegrityError::Serialization(e) => {
                AppError::validation("serialization", e.to_string())
            }
            IntegrityError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

/// Convenience alias for `Result<T, IntegrityError>`.
pub type Result<T> = std::result::Result<T, IntegrityError>;
