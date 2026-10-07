//! Root Manifest V1 specification, logical file metadata, and validation engine.

use crate::chunk::ChunkManifest;
use crate::error::{ManifestError, Result};
use crate::types::{CompressionMetadata, EncryptionMetadata, IntegrityMetadata};
use crate::version::ManifestVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use televault_core::ids::FileId;
use televault_core::validation::validate_safe_relative_path;

/// Metadata describing the original user-facing logical file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalFileMetadata {
    /// Unique identifier for the logical file.
    pub file_id: FileId,
    /// Original user-visible file name (e.g. "MyVideo.mp4").
    pub file_name: String,
    /// Safe relative path from the backup root (e.g. "Videos/MyVideo.mp4").
    pub relative_path: String,
    /// Total original file size in bytes before compression or encryption.
    pub original_size: u64,
    /// File creation timestamp in milliseconds since Unix epoch, if available.
    pub created_at: Option<u64>,
    /// File modification timestamp in milliseconds since Unix epoch, if available.
    pub modified_at: Option<u64>,
    /// MIME type string if detected (e.g. "video/mp4").
    pub mime_type: Option<String>,
}

impl LogicalFileMetadata {
    /// Validates logical file metadata invariants.
    pub fn validate(&self) -> Result<()> {
        if self.file_name.trim().is_empty() {
            return Err(ManifestError::Validation {
                field: "logical_file.file_name",
                message: "file_name cannot be empty".into(),
            });
        }
        if self.relative_path.trim().is_empty() {
            return Err(ManifestError::Validation {
                field: "logical_file.relative_path",
                message: "relative_path cannot be empty".into(),
            });
        }
        validate_safe_relative_path(Path::new(&self.relative_path)).map_err(|e| {
            ManifestError::Validation {
                field: "logical_file.relative_path",
                message: format!("path safety validation failed: {e}"),
            }
        })?;
        Ok(())
    }
}

/// Root Manifest V1 representing a logical file and all its physical storage chunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestV1 {
    /// Manifest specification format version (`v1`).
    pub manifest_version: ManifestVersion,
    /// Unique identifier for this manifest instance.
    pub manifest_id: String,
    /// Metadata describing the logical user-facing file.
    pub logical_file: LogicalFileMetadata,
    /// Optional application-level encryption metadata.
    ///
    /// Must be `Some` when encryption is enabled, or `None` when encryption is disabled.
    pub encryption: Option<EncryptionMetadata>,
    /// Compression configuration applied to chunk payloads.
    pub compression: CompressionMetadata,
    /// Cryptographic integrity hash of the complete original logical file.
    pub integrity: IntegrityMetadata,
    /// Authoritative ordered list of physical storage chunks.
    pub chunks: Vec<ChunkManifest>,
}

impl ManifestV1 {
    /// Validates all manifest invariants, chunk orderings, and size consistency.
    pub fn validate(&self) -> Result<()> {
        // 1. Version check
        if self.manifest_version != ManifestVersion::V1 {
            return Err(ManifestError::UnsupportedVersion(
                self.manifest_version.to_string(),
            ));
        }

        // 2. Manifest ID check
        if self.manifest_id.trim().is_empty() {
            return Err(ManifestError::Validation {
                field: "manifest_id",
                message: "manifest_id cannot be empty".into(),
            });
        }

        // 3. Logical file validation
        self.logical_file.validate()?;

        // 4. File-level integrity validation
        self.integrity.validate()?;

        // 5. Encryption validation if enabled
        if let Some(ref enc) = self.encryption {
            enc.validate()?;
        }

        // 6. Chunks list validation
        if self.chunks.is_empty() {
            return Err(ManifestError::Validation {
                field: "chunks",
                message: "manifest must contain at least one chunk".into(),
            });
        }

        let mut seen_chunk_ids = HashSet::with_capacity(self.chunks.len());
        let mut plaintext_sum: u64 = 0;

        for (expected_idx, chunk) in self.chunks.iter().enumerate() {
            // Validate chunk-specific fields
            chunk.validate()?;

            // Authoritative contiguous ordering: 0, 1, ..., N-1
            if chunk.index != expected_idx as u32 {
                return Err(ManifestError::ChunkOrdering {
                    position: expected_idx,
                    expected: expected_idx as u32,
                    actual: chunk.index,
                });
            }

            // Duplicate ChunkId check
            if !seen_chunk_ids.insert(chunk.chunk_id.as_str()) {
                return Err(ManifestError::DuplicateChunkId(chunk.chunk_id.to_string()));
            }

            plaintext_sum = plaintext_sum
                .checked_add(chunk.plaintext_size)
                .ok_or_else(|| ManifestError::Validation {
                    field: "chunks.plaintext_size",
                    message: "integer overflow calculating chunk plaintext sum".into(),
                })?;
        }

        // 7. Verify plaintext sum equals original logical file size
        if plaintext_sum != self.logical_file.original_size {
            return Err(ManifestError::SizeMismatch {
                logical_size: self.logical_file.original_size,
                chunks_sum: plaintext_sum,
            });
        }

        Ok(())
    }

    /// Serializes the manifest into a standard JSON string.
    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string(self).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    /// Serializes the manifest into a formatted, human-readable JSON string.
    pub fn to_json_pretty(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    /// Deserializes a manifest from JSON and validates all invariants.
    pub fn from_json(json: &str) -> Result<Self> {
        let manifest: Self = serde_json::from_str(json)
            .map_err(|e| ManifestError::Deserialization(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{KdfInfo, StorageReference};
    use televault_core::ids::{ChunkId, FileId};
    use televault_core::models::EncryptionAlgorithm;

    fn sample_valid_manifest(chunks_count: usize, total_size: u64) -> ManifestV1 {
        let file_id = FileId::new("file-sample-01").unwrap();
        let logical_file = LogicalFileMetadata {
            file_id,
            file_name: "document.pdf".into(),
            relative_path: "documents/document.pdf".into(),
            original_size: total_size,
            created_at: Some(1700000000000),
            modified_at: Some(1700000001000),
            mime_type: Some("application/pdf".into()),
        };

        let chunk_size = if chunks_count > 0 {
            total_size / (chunks_count as u64)
        } else {
            0
        };
        let remainder = if chunks_count > 0 {
            total_size % (chunks_count as u64)
        } else {
            0
        };

        let mut chunks = Vec::with_capacity(chunks_count);
        for i in 0..chunks_count {
            let size = chunk_size + if i == chunks_count - 1 { remainder } else { 0 };
            chunks.push(ChunkManifest {
                chunk_id: ChunkId::new(format!("chunk-{i:03}")).unwrap(),
                index: i as u32,
                plaintext_size: size,
                stored_size: size,
                integrity: IntegrityMetadata::sha256(
                    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                ),
                storage_reference: StorageReference::Telegram {
                    chat_id: -1001234567890,
                    message_id: (i + 10) as i64,
                    file_id: format!("tg_file_{i}"),
                },
            });
        }

        ManifestV1 {
            manifest_version: ManifestVersion::V1,
            manifest_id: "manifest-001".into(),
            logical_file,
            encryption: None,
            compression: CompressionMetadata::none(),
            integrity: IntegrityMetadata::sha256(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            chunks,
        }
    }

    #[test]
    fn test_valid_single_file_manifest() {
        let manifest = sample_valid_manifest(1, 1024 * 1024);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_valid_multi_chunk_manifest_5gb() {
        let total_size = (5.2 * 1024.0 * 1024.0 * 1024.0) as u64;
        let manifest = sample_valid_manifest(3, total_size);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_encrypted_manifest_metadata() {
        let mut manifest = sample_valid_manifest(1, 1024);
        manifest.encryption = Some(EncryptionMetadata {
            format_version: 1,
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            kdf: Some(KdfInfo {
                salt_hex: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".into(),
                memory_kib: 65536,
                iterations: 3,
                parallelism: 2,
            }),
        });

        assert!(manifest.validate().is_ok());
        let json = manifest.to_json().unwrap();
        let parsed = ManifestV1::from_json(&json).unwrap();
        assert_eq!(parsed, manifest);
        assert!(parsed.encryption.is_some());
    }

    #[test]
    fn test_unencrypted_manifest_metadata() {
        let manifest = sample_valid_manifest(1, 1024);
        assert!(manifest.encryption.is_none());
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_compression_combinations() {
        // 1. None + None
        let mut m1 = sample_valid_manifest(1, 512);
        m1.compression = CompressionMetadata::none();
        m1.encryption = None;
        assert!(m1.validate().is_ok());

        // 2. Zstd + None
        let mut m2 = sample_valid_manifest(1, 512);
        m2.compression = CompressionMetadata::zstd();
        m2.encryption = None;
        assert!(m2.validate().is_ok());

        // 3. None + Aes256Gcm
        let mut m3 = sample_valid_manifest(1, 512);
        m3.compression = CompressionMetadata::none();
        m3.encryption = Some(EncryptionMetadata::aes256_gcm(None));
        assert!(m3.validate().is_ok());

        // 4. Zstd + Aes256Gcm
        let mut m4 = sample_valid_manifest(1, 512);
        m4.compression = CompressionMetadata::zstd();
        m4.encryption = Some(EncryptionMetadata::aes256_gcm(None));
        assert!(m4.validate().is_ok());
    }

    #[test]
    fn test_json_serialization_roundtrip() {
        let manifest = sample_valid_manifest(2, 2048);
        let json = manifest.to_json().unwrap();
        let parsed = ManifestV1::from_json(&json).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn test_empty_filename_rejected() {
        let mut manifest = sample_valid_manifest(1, 100);
        manifest.logical_file.file_name = "   ".into();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_unsafe_path_rejected() {
        let mut manifest = sample_valid_manifest(1, 100);
        manifest.logical_file.relative_path = "../escape.pdf".into();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_non_contiguous_chunk_index_rejected() {
        let mut manifest = sample_valid_manifest(3, 300);
        manifest.chunks[2].index = 3;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_duplicate_chunk_id_rejected() {
        let mut manifest = sample_valid_manifest(2, 200);
        manifest.chunks[1].chunk_id = manifest.chunks[0].chunk_id.clone();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_size_mismatch_rejected() {
        let mut manifest = sample_valid_manifest(1, 1000);
        manifest.chunks[0].plaintext_size = 800;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_invalid_hash_rejected() {
        let mut manifest = sample_valid_manifest(1, 100);
        manifest.integrity.digest = "not-a-valid-sha256".into();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_invalid_encryption_metadata_rejected() {
        let mut manifest = sample_valid_manifest(1, 100);
        manifest.encryption = Some(EncryptionMetadata {
            format_version: 99,
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            kdf: None,
        });
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_malformed_json_fails_safely() {
        let bad_json = "{ invalid json: true }";
        assert!(ManifestV1::from_json(bad_json).is_err());
    }
}
