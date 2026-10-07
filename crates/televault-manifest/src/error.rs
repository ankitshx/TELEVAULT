//! Manifest error definitions and conversions.

use televault_core::AppError;
use thiserror::Error;

/// Errors arising from manifest creation, parsing, serialization, and validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    /// Domain validation failure.
    #[error("Manifest validation error for '{field}': {message}")]
    Validation {
        /// Name of the field or component that failed validation.
        field: &'static str,
        /// Description of the validation failure.
        message: String,
    },

    /// Unsupported manifest specification version.
    #[error("Unsupported manifest version: {0}")]
    UnsupportedVersion(String),

    /// Chunk indexing or ordering violation.
    #[error("Chunk ordering error: chunk at index {position} has invalid index {actual} (expected {expected})")]
    ChunkOrdering {
        /// Position in chunks array.
        position: usize,
        /// Expected chunk index.
        expected: u32,
        /// Actual chunk index.
        actual: u32,
    },

    /// Duplicate chunk identifier detected.
    #[error("Duplicate chunk ID detected: {0}")]
    DuplicateChunkId(String),

    /// Total logical file size does not match sum of chunk plaintext sizes.
    #[error("File size mismatch: logical file size is {logical_size} bytes, but chunk plaintext sum is {chunks_sum} bytes")]
    SizeMismatch {
        /// Expected size according to logical file metadata.
        logical_size: u64,
        /// Sum of all chunk plaintext sizes.
        chunks_sum: u64,
    },

    /// Integrity hash format or length violation.
    #[error("Invalid integrity hash for {algorithm}: {reason}")]
    InvalidHash {
        /// Name of the hashing algorithm.
        algorithm: String,
        /// Reason for rejection.
        reason: String,
    },

    /// Serialization error.
    #[error("Manifest serialization error: {0}")]
    Serialization(String),

    /// Deserialization error.
    #[error("Manifest deserialization error: {0}")]
    Deserialization(String),
}

impl From<ManifestError> for AppError {
    fn from(err: ManifestError) -> Self {
        match err {
            ManifestError::Validation { field, message } => AppError::Validation { field, message },
            ManifestError::UnsupportedVersion(v) => AppError::Validation {
                field: "manifest.version",
                message: format!("unsupported version: {v}"),
            },
            ManifestError::ChunkOrdering { position, expected, actual } => AppError::Validation {
                field: "manifest.chunks.ordering",
                message: format!("ordering violation at position {position}: expected index {expected}, got {actual}"),
            },
            ManifestError::DuplicateChunkId(id) => AppError::Validation {
                field: "manifest.chunks.id",
                message: format!("duplicate chunk id: {id}"),
            },
            ManifestError::SizeMismatch { logical_size, chunks_sum } => AppError::Validation {
                field: "manifest.size",
                message: format!("size mismatch: logical={logical_size}, chunks_sum={chunks_sum}"),
            },
            ManifestError::InvalidHash { algorithm, reason } => AppError::Validation {
                field: "manifest.integrity",
                message: format!("invalid hash ({algorithm}): {reason}"),
            },
            ManifestError::Serialization(msg) => AppError::Internal(format!("manifest serialization failed: {msg}")),
            ManifestError::Deserialization(msg) => AppError::Validation {
                field: "manifest.deserialization",
                message: format!("manifest parsing failed: {msg}"),
            },
        }
    }
}

/// Convenience result alias for manifest operations.
pub type Result<T> = std::result::Result<T, ManifestError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion_to_app_error() {
        let val_err = ManifestError::Validation {
            field: "file_name",
            message: "cannot be empty".into(),
        };
        let app_err: AppError = val_err.into();
        assert_eq!(app_err.error_code(), "VALIDATION_ERROR");

        let size_err = ManifestError::SizeMismatch {
            logical_size: 1000,
            chunks_sum: 800,
        };
        let app_err2: AppError = size_err.into();
        assert_eq!(app_err2.error_code(), "VALIDATION_ERROR");
    }
}
