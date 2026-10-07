//! Cryptographic error types.

use televault_core::AppError;
use thiserror::Error;

/// Errors arising from cryptographic operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    /// Ciphertext authentication failed (invalid key, tampered ciphertext, or mismatched AAD).
    #[error("Authentication failed: invalid key, corrupted ciphertext, or tampered context")]
    AuthenticationFailed,

    /// Password-based key derivation failed.
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    /// Nonce length or format is invalid.
    #[error("Invalid nonce: expected 12 bytes, got {0}")]
    InvalidNonceLength(usize),

    /// Key length is invalid.
    #[error("Invalid key length: expected {expected} bytes, got {actual}")]
    InvalidKeyLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },

    /// Unsupported encrypted payload version.
    #[error("Unsupported encrypted payload version: {0}")]
    UnsupportedVersion(u32),

    /// Payload structure is malformed or truncated.
    #[error("Malformed encrypted payload: {0}")]
    MalformedPayload(String),

    /// Authenticated associated data mismatch.
    #[error("AAD validation failed: {0}")]
    AadMismatch(String),
}

impl From<CryptoError> for AppError {
    fn from(err: CryptoError) -> Self {
        match err {
            CryptoError::AuthenticationFailed => AppError::Validation {
                field: "crypto.auth",
                message: "authentication failed: data was tampered with or key is incorrect".into(),
            },
            CryptoError::KeyDerivation(msg) => AppError::Internal(format!("kdf error: {msg}")),
            CryptoError::InvalidNonceLength(len) => AppError::Validation {
                field: "crypto.nonce",
                message: format!("invalid nonce length: {len}"),
            },
            CryptoError::InvalidKeyLength { expected, actual } => AppError::Validation {
                field: "crypto.key",
                message: format!("invalid key length: expected {expected}, got {actual}"),
            },
            CryptoError::UnsupportedVersion(v) => AppError::Validation {
                field: "crypto.version",
                message: format!("unsupported crypto format version {v}"),
            },
            CryptoError::MalformedPayload(msg) => AppError::Validation {
                field: "crypto.payload",
                message: format!("malformed payload: {msg}"),
            },
            CryptoError::AadMismatch(msg) => AppError::Validation {
                field: "crypto.aad",
                message: format!("AAD mismatch: {msg}"),
            },
        }
    }
}

/// Convenience result alias for crypto operations.
pub type Result<T> = std::result::Result<T, CryptoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion_to_app_error() {
        let auth_err = CryptoError::AuthenticationFailed;
        let app_err: AppError = auth_err.into();
        assert_eq!(app_err.error_code(), "VALIDATION_ERROR");

        let kdf_err = CryptoError::KeyDerivation("out of memory".into());
        let app_err: AppError = kdf_err.into();
        assert_eq!(app_err.error_code(), "INTERNAL_ERROR");
    }
}
