//! Supporting metadata types for integrity, compression, encryption, and storage.

use crate::error::{ManifestError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use televault_core::models::{CompressionAlgorithm, EncryptionAlgorithm};

/// Supported cryptographic integrity hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    /// SHA-256 (standard 256-bit digest, 64 hex characters).
    #[default]
    Sha256,
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha256 => write!(f, "sha256"),
        }
    }
}

/// Cryptographic hash digest for data integrity verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityMetadata {
    /// Hashing algorithm used.
    pub algorithm: HashAlgorithm,
    /// Hex-encoded digest string (lowercase).
    pub digest: String,
}

impl IntegrityMetadata {
    /// Creates a new [`IntegrityMetadata`] with SHA-256.
    pub fn sha256(digest: impl Into<String>) -> Self {
        Self {
            algorithm: HashAlgorithm::Sha256,
            digest: digest.into(),
        }
    }

    /// Validates digest format and length according to the specified algorithm.
    pub fn validate(&self) -> Result<()> {
        match self.algorithm {
            HashAlgorithm::Sha256 => {
                let trimmed = self.digest.trim();
                if trimmed.len() != 64 {
                    return Err(ManifestError::InvalidHash {
                        algorithm: self.algorithm.to_string(),
                        reason: format!(
                            "expected 64 hexadecimal characters, got {} characters",
                            trimmed.len()
                        ),
                    });
                }
                for ch in trimmed.chars() {
                    if !ch.is_ascii_hexdigit() {
                        return Err(ManifestError::InvalidHash {
                            algorithm: self.algorithm.to_string(),
                            reason: format!("contains invalid hexadecimal character '{ch}'"),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Compression configuration applied to the logical file or chunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressionMetadata {
    /// Compression algorithm applied.
    pub algorithm: CompressionAlgorithm,
}

impl CompressionMetadata {
    /// Uncompressed configuration.
    pub fn none() -> Self {
        Self {
            algorithm: CompressionAlgorithm::None,
        }
    }

    /// Zstandard compression configuration.
    pub fn zstd() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Zstd,
        }
    }
}

impl Default for CompressionMetadata {
    fn default() -> Self {
        Self::none()
    }
}

/// Application-level encryption configuration.
///
/// Present only when encryption is enabled. Never present when encryption is disabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    /// Cryptographic specification version (currently 1).
    pub format_version: u32,
    /// Symmetric cipher algorithm.
    pub algorithm: EncryptionAlgorithm,
    /// Key derivation parameters if key was derived from user passphrase.
    pub kdf: Option<KdfInfo>,
}

impl EncryptionMetadata {
    /// Creates standard AES-256-GCM encryption metadata.
    pub fn aes256_gcm(kdf: Option<KdfInfo>) -> Self {
        Self {
            format_version: 1,
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            kdf,
        }
    }

    /// Validates encryption metadata invariants.
    pub fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(ManifestError::Validation {
                field: "encryption.format_version",
                message: format!("unsupported crypto format version {}", self.format_version),
            });
        }
        if let Some(ref kdf) = self.kdf {
            kdf.validate()?;
        }
        Ok(())
    }
}

/// KDF parameters recorded in the manifest when key derivation was utilized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfInfo {
    /// Hex-encoded salt bytes.
    pub salt_hex: String,
    /// Memory cost in KiB.
    pub memory_kib: u32,
    /// Iterations.
    pub iterations: u32,
    /// Parallelism threads.
    pub parallelism: u32,
}

impl KdfInfo {
    /// Validates KDF parameters recorded in manifest.
    pub fn validate(&self) -> Result<()> {
        let trimmed = self.salt_hex.trim();
        if trimmed.is_empty() {
            return Err(ManifestError::Validation {
                field: "encryption.kdf.salt_hex",
                message: "salt cannot be empty".into(),
            });
        }
        for ch in trimmed.chars() {
            if !ch.is_ascii_hexdigit() {
                return Err(ManifestError::Validation {
                    field: "encryption.kdf.salt_hex",
                    message: format!("salt contains non-hex character '{ch}'"),
                });
            }
        }
        if self.memory_kib == 0 {
            return Err(ManifestError::Validation {
                field: "encryption.kdf.memory_kib",
                message: "memory cost must be positive".into(),
            });
        }
        if self.iterations == 0 {
            return Err(ManifestError::Validation {
                field: "encryption.kdf.iterations",
                message: "iterations must be positive".into(),
            });
        }
        Ok(())
    }
}

/// Authoritative reference indicating where a physical chunk is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageReference {
    /// Stored as a Telegram message/document in a channel or chat.
    Telegram {
        /// Target Telegram chat or channel ID.
        chat_id: i64,
        /// Message ID containing the document.
        message_id: i64,
        /// Telegram file_id identifier string.
        file_id: String,
    },
    /// Staged in local storage awaiting upload or after download.
    LocalStaging {
        /// Safe relative path within staging directory.
        relative_path: String,
    },
    /// In-flight or pending upload allocation.
    Pending,
}

impl StorageReference {
    /// Validates storage reference invariants.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Telegram {
                chat_id,
                message_id,
                file_id,
            } => {
                if *chat_id == 0 {
                    return Err(ManifestError::Validation {
                        field: "storage.telegram.chat_id",
                        message: "chat_id cannot be 0".into(),
                    });
                }
                if *message_id <= 0 {
                    return Err(ManifestError::Validation {
                        field: "storage.telegram.message_id",
                        message: "message_id must be positive".into(),
                    });
                }
                if file_id.trim().is_empty() {
                    return Err(ManifestError::Validation {
                        field: "storage.telegram.file_id",
                        message: "file_id cannot be empty".into(),
                    });
                }
            }
            Self::LocalStaging { relative_path } => {
                if relative_path.trim().is_empty() {
                    return Err(ManifestError::Validation {
                        field: "storage.local_staging.relative_path",
                        message: "relative_path cannot be empty".into(),
                    });
                }
            }
            Self::Pending => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrity_validation() {
        let valid_sha256 = IntegrityMetadata::sha256(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        assert!(valid_sha256.validate().is_ok());

        // Too short
        let invalid_len = IntegrityMetadata::sha256("abc123");
        assert!(invalid_len.validate().is_err());

        // Non-hex characters
        let invalid_hex = IntegrityMetadata::sha256("g".repeat(64));
        assert!(invalid_hex.validate().is_err());
    }

    #[test]
    fn test_storage_reference_validation() {
        let valid_tg = StorageReference::Telegram {
            chat_id: -1001234567890,
            message_id: 42,
            file_id: "BAACAgIAAxkBAAE...".into(),
        };
        assert!(valid_tg.validate().is_ok());

        let invalid_tg = StorageReference::Telegram {
            chat_id: 0,
            message_id: 42,
            file_id: "abc".into(),
        };
        assert!(invalid_tg.validate().is_err());

        let valid_pending = StorageReference::Pending;
        assert!(valid_pending.validate().is_ok());
    }
}
