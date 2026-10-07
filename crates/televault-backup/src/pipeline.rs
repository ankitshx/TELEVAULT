//! Streaming payload processing, chunk partitioning, and staging pipeline.

use crate::error::{BackupError, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use televault_core::ids::{ChunkId, FileId};
use televault_core::models::CompressionAlgorithm;
use televault_crypto::aad::ChunkAad;
use televault_crypto::cipher::encrypt_chunk;
use televault_crypto::policy::EncryptionPolicy;
use televault_manifest::chunk::{
    calculate_expected_chunk_count, ChunkManifest, TARGET_CHUNK_SIZE_BYTES,
};
use televault_manifest::manifest::ManifestV1;
use televault_manifest::types::{
    CompressionMetadata, EncryptionMetadata, IntegrityMetadata, StorageReference,
};
use televault_manifest::version::ManifestVersion;
use televault_storage::temp::{TempPayloadFile, TempPayloadManager};
use televault_storage::types::STREAM_CHUNK_BUFFER_SIZE;

/// Processing options governing optional encryption and compression.
#[derive(Debug, Clone)]
pub struct ProcessingOptions {
    /// Cryptographic encryption policy (Enabled or Disabled).
    pub encryption_policy: EncryptionPolicy,
    /// Payload compression algorithm (None or Zstd).
    pub compression_algorithm: CompressionAlgorithm,
}

impl PartialEq for ProcessingOptions {
    fn eq(&self, other: &Self) -> bool {
        self.encryption_policy.is_enabled() == other.encryption_policy.is_enabled()
            && self.compression_algorithm == other.compression_algorithm
    }
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            encryption_policy: EncryptionPolicy::Disabled,
            compression_algorithm: CompressionAlgorithm::None,
        }
    }
}

/// A processed physical chunk ready for handoff to the transfer engine.
pub struct ProcessedChunk {
    /// Physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Zero-based sequential chunk index (0..N-1).
    pub chunk_index: u32,
    /// Total count of chunks in this logical file.
    pub total_chunks: u32,
    /// Uncompressed plaintext size in bytes.
    pub plaintext_size: u64,
    /// Final stored size in bytes after optional compression and encryption.
    pub stored_size: u64,
    /// Hexadecimal SHA-256 digest of original plaintext chunk.
    pub plaintext_sha256: String,
    /// Hexadecimal SHA-256 digest of final stored chunk payload.
    pub stored_sha256: String,
    /// Managed temporary staging file holding the payload for upload.
    pub staging_file: TempPayloadFile,
}

/// Complete result of processing a logical file through the pipeline.
pub struct ProcessedFile {
    /// Logical file identifier.
    pub file_id: FileId,
    /// Original uncompressed, unencrypted logical file size.
    pub total_size: u64,
    /// All processed physical chunks in strict index order.
    pub chunks: Vec<ProcessedChunk>,
    /// Cryptographic SHA-256 digest of the complete original logical file.
    pub logical_file_hash: String,
    /// Authoritative manifest specification representing this file.
    pub manifest: ManifestV1,
}

/// Pipeline processor coordinating bounded reading, partitioning, hashing, and staging.
pub struct PayloadPipeline;

impl PayloadPipeline {
    /// Processes a stream into bounded staged chunks and an authoritative manifest.
    pub fn process_file<R: Read>(
        file_id: &FileId,
        relative_path: &str,
        mut reader: R,
        total_size: u64,
        temp_manager: &TempPayloadManager,
        options: &ProcessingOptions,
    ) -> Result<ProcessedFile> {
        let total_chunks = calculate_expected_chunk_count(total_size);
        let mut processed_chunks = Vec::new();
        let mut chunk_manifests = Vec::new();
        let mut whole_file_hasher = Sha256::new();

        let mut buffer = [0u8; STREAM_CHUNK_BUFFER_SIZE];
        let mut total_bytes_read: u64 = 0;

        for chunk_idx in 0..total_chunks {
            let chunk_id = ChunkId::new(format!("{file_id}-chk-{chunk_idx}"))
                .map_err(|e| BackupError::ManifestError(e.to_string()))?;

            // Determine chunk target size
            let expected_chunk_bytes = if total_chunks == 1 {
                total_size
            } else if chunk_idx == total_chunks - 1 {
                total_size - (chunk_idx as u64 * TARGET_CHUNK_SIZE_BYTES)
            } else {
                TARGET_CHUNK_SIZE_BYTES
            };

            let staging_file = temp_manager
                .create_staging_file("chunk")
                .map_err(|e| BackupError::StorageError(e.to_string()))?;

            let mut chunk_plaintext_hasher = Sha256::new();
            let mut chunk_stored_hasher = Sha256::new();
            let mut chunk_bytes_read: u64 = 0;
            let chunk_stored_bytes = match options.encryption_policy {
                EncryptionPolicy::Disabled => {
                    // Plain streaming path: zero in-memory accumulation, 64 KiB buffer directly to staging
                    let mut staging_writer = std::fs::OpenOptions::new()
                        .write(true)
                        .open(staging_file.path())
                        .map_err(BackupError::from)?;

                    while chunk_bytes_read < expected_chunk_bytes {
                        let to_read = (expected_chunk_bytes - chunk_bytes_read)
                            .min(STREAM_CHUNK_BUFFER_SIZE as u64)
                            as usize;
                        let n = reader
                            .read(&mut buffer[..to_read])
                            .map_err(BackupError::from)?;
                        if n == 0 {
                            break;
                        }

                        whole_file_hasher.update(&buffer[..n]);
                        chunk_plaintext_hasher.update(&buffer[..n]);
                        chunk_stored_hasher.update(&buffer[..n]);

                        staging_writer
                            .write_all(&buffer[..n])
                            .map_err(BackupError::from)?;

                        chunk_bytes_read += n as u64;
                        total_bytes_read += n as u64;
                    }

                    staging_writer.flush().map_err(BackupError::from)?;
                    chunk_bytes_read
                }
                EncryptionPolicy::Enabled(ref key) => {
                    // Encrypted path: read chunk data, apply AES-256-GCM with ChunkAad
                    let mut plaintext_bytes = Vec::new();
                    while chunk_bytes_read < expected_chunk_bytes {
                        let to_read = (expected_chunk_bytes - chunk_bytes_read)
                            .min(STREAM_CHUNK_BUFFER_SIZE as u64)
                            as usize;
                        let n = reader
                            .read(&mut buffer[..to_read])
                            .map_err(BackupError::from)?;
                        if n == 0 {
                            break;
                        }

                        whole_file_hasher.update(&buffer[..n]);
                        chunk_plaintext_hasher.update(&buffer[..n]);
                        plaintext_bytes.extend_from_slice(&buffer[..n]);

                        chunk_bytes_read += n as u64;
                        total_bytes_read += n as u64;
                    }

                    let aad = ChunkAad::new(file_id.clone(), chunk_idx, total_chunks);
                    let encrypted_payload = encrypt_chunk(key, &plaintext_bytes, &aad, None)
                        .map_err(|e| BackupError::CryptoError(e.to_string()))?;

                    let serialized_payload = serde_json::to_vec(&encrypted_payload)
                        .map_err(|e| BackupError::CryptoError(e.to_string()))?;
                    chunk_stored_hasher.update(&serialized_payload);

                    let mut staging_writer = std::fs::OpenOptions::new()
                        .write(true)
                        .open(staging_file.path())
                        .map_err(BackupError::from)?;
                    staging_writer
                        .write_all(&serialized_payload)
                        .map_err(BackupError::from)?;
                    staging_writer.flush().map_err(BackupError::from)?;

                    serialized_payload.len() as u64
                }
            };

            let plaintext_sha256 = format!("{:x}", chunk_plaintext_hasher.finalize());
            let stored_sha256 = format!("{:x}", chunk_stored_hasher.finalize());

            // Build ChunkManifest entry
            let chunk_manifest = ChunkManifest {
                chunk_id: chunk_id.clone(),
                index: chunk_idx,
                plaintext_size: chunk_bytes_read,
                stored_size: chunk_stored_bytes,
                integrity: IntegrityMetadata::sha256(&stored_sha256),
                storage_reference: StorageReference::Pending,
            };
            chunk_manifests.push(chunk_manifest);

            processed_chunks.push(ProcessedChunk {
                chunk_id,
                chunk_index: chunk_idx,
                total_chunks,
                plaintext_size: chunk_bytes_read,
                stored_size: chunk_stored_bytes,
                plaintext_sha256,
                stored_sha256,
                staging_file,
            });
        }

        let logical_file_hash = format!("{:x}", whole_file_hasher.finalize());

        let file_name = std::path::Path::new(relative_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| relative_path.to_string());

        // Construct LogicalFileMetadata
        let logical_file = televault_manifest::LogicalFileMetadata {
            file_id: file_id.clone(),
            file_name,
            relative_path: relative_path.into(),
            original_size: total_bytes_read,
            created_at: None,
            modified_at: None,
            mime_type: None,
        };

        // Construct ManifestV1
        let encryption_metadata = match options.encryption_policy {
            EncryptionPolicy::Enabled(_) => Some(EncryptionMetadata::aes256_gcm(None)),
            EncryptionPolicy::Disabled => None,
        };

        let compression_metadata = match options.compression_algorithm {
            CompressionAlgorithm::None => CompressionMetadata::none(),
            CompressionAlgorithm::Zstd => CompressionMetadata::zstd(),
        };

        let manifest = ManifestV1 {
            manifest_version: ManifestVersion::V1,
            manifest_id: format!("man-{}", file_id),
            logical_file,
            encryption: encryption_metadata,
            compression: compression_metadata,
            integrity: IntegrityMetadata::sha256(&logical_file_hash),
            chunks: chunk_manifests,
        };

        Ok(ProcessedFile {
            file_id: file_id.clone(),
            total_size: total_bytes_read,
            chunks: processed_chunks,
            logical_file_hash,
            manifest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use televault_crypto::key::SecretKey;

    #[test]
    fn test_pipeline_unencrypted_single_chunk() {
        let temp_dir = std::env::temp_dir().join("televault_test_pipeline_unenc");
        let manager = TempPayloadManager::new(&temp_dir);

        let data = b"Hello, unencrypted payload pipeline stream!";
        let file_id = FileId::new("file-pipe-1").unwrap();

        let options = ProcessingOptions::default();
        let processed = PayloadPipeline::process_file(
            &file_id,
            "test.txt",
            &data[..],
            data.len() as u64,
            &manager,
            &options,
        )
        .unwrap();

        assert_eq!(processed.chunks.len(), 1);
        assert_eq!(processed.total_size, data.len() as u64);
        assert_eq!(processed.chunks[0].plaintext_size, data.len() as u64);
        assert_eq!(processed.chunks[0].stored_size, data.len() as u64);
        assert_eq!(
            processed.chunks[0].plaintext_sha256,
            processed.chunks[0].stored_sha256
        );

        let staged_content = fs::read(processed.chunks[0].staging_file.path()).unwrap();
        assert_eq!(staged_content, data);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pipeline_encrypted_single_chunk() {
        let temp_dir = std::env::temp_dir().join("televault_test_pipeline_enc");
        let manager = TempPayloadManager::new(&temp_dir);

        let data = b"Top secret classified content destined for AES-256-GCM";
        let file_id = FileId::new("file-pipe-enc").unwrap();
        let secret_key = SecretKey::generate();

        let options = ProcessingOptions {
            encryption_policy: EncryptionPolicy::Enabled(secret_key),
            compression_algorithm: CompressionAlgorithm::None,
        };

        let processed = PayloadPipeline::process_file(
            &file_id,
            "secret.pdf",
            &data[..],
            data.len() as u64,
            &manager,
            &options,
        )
        .unwrap();

        assert_eq!(processed.chunks.len(), 1);
        assert_eq!(processed.chunks[0].plaintext_size, data.len() as u64);
        // Stored size must include encryption header and tag
        assert!(processed.chunks[0].stored_size > data.len() as u64);
        assert_ne!(
            processed.chunks[0].plaintext_sha256,
            processed.chunks[0].stored_sha256
        );

        let staged_content = fs::read(processed.chunks[0].staging_file.path()).unwrap();
        assert_ne!(staged_content, data);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
