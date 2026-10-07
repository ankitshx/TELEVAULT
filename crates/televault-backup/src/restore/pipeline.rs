//! Reverse processing pipeline: downloads, authenticates, decrypts, and verifies logical files.

use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use televault_core::ids::JobId;
use televault_core::validation::validate_safe_relative_path;
use televault_crypto::aad::ChunkAad;
use televault_crypto::cipher::decrypt_chunk;
use televault_crypto::payload::EncryptedPayload;
use televault_crypto::policy::EncryptionPolicy;
use televault_manifest::chunk::{calculate_expected_chunk_count, CHUNK_THRESHOLD_BYTES};
use televault_manifest::{ManifestV1, ManifestVersion, StorageReference};
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::TransferEngine;
use televault_transfer::job::{DownloadJobParams, TransferJob};

use super::collision::{resolve_collision, CollisionPolicy, CollisionResolution};
use super::types::{FileRestoreOutcome, RestoreResult};
use crate::error::{RestoreError, RestoreOpResult};

/// Buffer size for bounded streaming operations (64 KiB).
pub const RESTORE_STREAM_BUFFER_SIZE: usize = 64 * 1024;

/// Pipeline orchestrating the reverse reconstruction of logical files from remote chunks.
pub struct RestorePipeline;

impl RestorePipeline {
    /// Strictly pre-validates a manifest before any download commences.
    ///
    /// Rejects incomplete backups, pending chunk allocations, mismatched chunk counts,
    /// duplicate indices, or corrupted metadata.
    pub fn validate_manifest_for_restore(manifest: &ManifestV1) -> RestoreOpResult<()> {
        // 1. Basic schema and structural invariant validation
        manifest.validate()?;

        // 2. Format version support
        if manifest.manifest_version != ManifestVersion::V1 {
            return Err(RestoreError::IncompleteBackup {
                reason: format!(
                    "Unsupported manifest version: {}",
                    manifest.manifest_version
                ),
            });
        }

        // 3. Path traversal safety
        validate_safe_relative_path(Path::new(&manifest.logical_file.relative_path)).map_err(
            |e| RestoreError::UnsafePath {
                path: manifest.logical_file.relative_path.clone(),
                reason: e.to_string(),
            },
        )?;

        // 4. Expected chunk count matches logical file size (strictly enforced for files >= 2GB)
        let expected_chunks =
            calculate_expected_chunk_count(manifest.logical_file.original_size) as usize;
        if manifest.logical_file.original_size >= CHUNK_THRESHOLD_BYTES
            && manifest.chunks.len() != expected_chunks
        {
            return Err(RestoreError::IncompleteBackup {
                reason: format!(
                    "Chunk count mismatch: manifest has {} chunks, but original size {} requires {} chunks",
                    manifest.chunks.len(),
                    manifest.logical_file.original_size,
                    expected_chunks
                ),
            });
        }

        // 5. Incomplete backup detection: NO chunk may possess a Pending storage reference
        for (pos, chunk) in manifest.chunks.iter().enumerate() {
            if matches!(chunk.storage_reference, StorageReference::Pending) {
                return Err(RestoreError::PendingChunk {
                    chunk_id: chunk.chunk_id.to_string(),
                    index: chunk.index,
                });
            }

            if chunk.index != pos as u32 {
                return Err(RestoreError::MissingChunk {
                    chunk_id: chunk.chunk_id.to_string(),
                    index: pos as u32,
                });
            }
        }

        Ok(())
    }

    /// Executes the complete restore workflow for a validated manifest to `target_path`.
    ///
    /// 1. Resolves destination collision policy.
    /// 2. Streams each physical chunk via [`TransferEngine`].
    /// 3. Authenticates and decrypts payloads if encryption is enabled.
    /// 4. Reconstructs the logical file in managed temporary staging.
    /// 5. Validates end-to-end whole-file size and SHA-256 hash.
    /// 6. Safely moves the verified file to the final destination path.
    pub fn restore_file(
        manifest: &ManifestV1,
        target_path: &Path,
        collision_policy: CollisionPolicy,
        encryption_policy: &EncryptionPolicy,
        transfer_engine: &TransferEngine,
        temp_manager: &TempPayloadManager,
        cancellation: &CancellationToken,
    ) -> RestoreOpResult<RestoreResult> {
        let start_time = std::time::Instant::now();

        // 1. Pre-validate manifest invariants
        Self::validate_manifest_for_restore(manifest)?;

        // 2. Encryption configuration validation
        if manifest.encryption.is_some() && !encryption_policy.is_enabled() {
            return Err(RestoreError::EncryptionMismatch {
                reason: "Manifest is encrypted but encryption policy is disabled (missing decryption key)".into(),
            });
        }

        // 3. Collision resolution against destination filesystem
        let destination_existed = target_path.exists();
        let final_target = match resolve_collision(target_path, collision_policy) {
            CollisionResolution::Skip => {
                return Ok(RestoreResult {
                    file_id: manifest.logical_file.file_id.clone(),
                    relative_path: manifest.logical_file.relative_path.clone(),
                    target_path: target_path.to_path_buf(),
                    outcome: FileRestoreOutcome::Skipped,
                    bytes_restored: 0,
                    verified_sha256: None,
                    elapsed_ms: start_time.elapsed().as_millis() as u64,
                    error: None,
                });
            }
            CollisionResolution::Apply(path) => path,
        };

        // 4. Create managed temporary staging file for reconstructed logical file
        let staging_file = temp_manager
            .create_staging_file("restore_logical_staging")
            .map_err(RestoreError::Storage)?;

        let mut staging_writer = OpenOptions::new()
            .write(true)
            .open(staging_file.path())
            .map_err(|e| RestoreError::Io(e.to_string()))?;

        let mut whole_file_hasher = Sha256::new();
        let mut total_restored_bytes: u64 = 0;
        let total_chunks = manifest.chunks.len() as u32;

        // 5. Ensure chunks are ordered strictly by chunk index 0..N-1
        let mut sorted_chunks = manifest.chunks.clone();
        sorted_chunks.sort_by_key(|c| c.index);

        // 6. Download and process chunks sequentially
        for chunk in sorted_chunks {
            if cancellation.is_cancelled() {
                return Err(RestoreError::Cancelled);
            }

            // Create temporary staging file for downloaded chunk payload
            let temp_chunk = temp_manager
                .create_staging_file(&format!("chunk_dl_{}", chunk.index))
                .map_err(RestoreError::Storage)?;

            let job_id = JobId::new(format!("restore-dl-{}", chunk.chunk_id)).map_err(|e| {
                RestoreError::Transfer(televault_transfer::TransferError::InvalidJob(e.to_string()))
            })?;

            let dl_params = DownloadJobParams::new(
                job_id,
                manifest.logical_file.file_id.clone(),
                chunk.chunk_id.clone(),
                chunk.storage_reference.clone(),
                chunk.stored_size,
            )
            .with_chunk(chunk.index, total_chunks)
            .with_sha256(&chunk.integrity.digest);

            let mut transfer_job =
                TransferJob::new_download(dl_params).map_err(RestoreError::Transfer)?;

            // Stream downloaded chunk from cloud storage to temporary chunk file
            {
                let mut chunk_writer = OpenOptions::new()
                    .write(true)
                    .open(temp_chunk.path())
                    .map_err(|e| RestoreError::Io(e.to_string()))?;

                let downloaded_bytes = transfer_engine
                    .download_stream(&mut transfer_job, &mut chunk_writer, cancellation)
                    .map_err(RestoreError::Transfer)?;

                chunk_writer
                    .flush()
                    .map_err(|e| RestoreError::Io(e.to_string()))?;

                if downloaded_bytes != chunk.stored_size {
                    return Err(RestoreError::SizeMismatch {
                        file_id: manifest.logical_file.file_id.to_string(),
                        expected: chunk.stored_size,
                        actual: downloaded_bytes,
                    });
                }
            }

            // Verify stored chunk integrity hash
            let mut chunk_file =
                File::open(temp_chunk.path()).map_err(|e| RestoreError::Io(e.to_string()))?;
            let mut chunk_hasher = Sha256::new();
            let mut buf = [0u8; RESTORE_STREAM_BUFFER_SIZE];
            loop {
                let n = chunk_file
                    .read(&mut buf)
                    .map_err(|e| RestoreError::Io(e.to_string()))?;
                if n == 0 {
                    break;
                }
                chunk_hasher.update(&buf[..n]);
            }
            let actual_stored_hash = format!("{:x}", chunk_hasher.finalize());
            if !actual_stored_hash.eq_ignore_ascii_case(&chunk.integrity.digest) {
                return Err(RestoreError::ChunkIntegrityMismatch {
                    chunk_id: chunk.chunk_id.to_string(),
                    expected: chunk.integrity.digest.clone(),
                    actual: actual_stored_hash,
                });
            }

            // Decode chunk payload into logical plaintext stream
            match encryption_policy {
                EncryptionPolicy::Enabled(ref key) => {
                    // Encrypted path: deserialize EncryptedPayload, verify AAD, and decrypt
                    let chunk_payload_file = File::open(temp_chunk.path())
                        .map_err(|e| RestoreError::Io(e.to_string()))?;

                    let encrypted_payload: EncryptedPayload =
                        serde_json::from_reader(chunk_payload_file).map_err(|e| {
                            RestoreError::DecryptionFailed {
                                chunk_id: chunk.chunk_id.to_string(),
                                reason: format!(
                                    "Failed to parse encrypted chunk payload JSON: {e}"
                                ),
                            }
                        })?;

                    let aad = ChunkAad::new(
                        manifest.logical_file.file_id.clone(),
                        chunk.index,
                        total_chunks,
                    );
                    let decrypted_bytes =
                        decrypt_chunk(key, &encrypted_payload, &aad).map_err(|e| {
                            RestoreError::DecryptionFailed {
                                chunk_id: chunk.chunk_id.to_string(),
                                reason: e.to_string(),
                            }
                        })?;

                    if decrypted_bytes.len() as u64 != chunk.plaintext_size {
                        return Err(RestoreError::SizeMismatch {
                            file_id: manifest.logical_file.file_id.to_string(),
                            expected: chunk.plaintext_size,
                            actual: decrypted_bytes.len() as u64,
                        });
                    }

                    whole_file_hasher.update(&decrypted_bytes);
                    staging_writer
                        .write_all(&decrypted_bytes)
                        .map_err(|e| RestoreError::Io(e.to_string()))?;

                    total_restored_bytes += decrypted_bytes.len() as u64;
                }
                EncryptionPolicy::Disabled => {
                    // Unencrypted path: stream directly from temp chunk file to staging file
                    let mut chunk_reader = File::open(temp_chunk.path())
                        .map_err(|e| RestoreError::Io(e.to_string()))?;

                    let mut chunk_bytes_read: u64 = 0;
                    loop {
                        let n = chunk_reader
                            .read(&mut buf)
                            .map_err(|e| RestoreError::Io(e.to_string()))?;
                        if n == 0 {
                            break;
                        }
                        whole_file_hasher.update(&buf[..n]);
                        staging_writer
                            .write_all(&buf[..n])
                            .map_err(|e| RestoreError::Io(e.to_string()))?;

                        chunk_bytes_read += n as u64;
                        total_restored_bytes += n as u64;
                    }

                    if chunk_bytes_read != chunk.plaintext_size {
                        return Err(RestoreError::SizeMismatch {
                            file_id: manifest.logical_file.file_id.to_string(),
                            expected: chunk.plaintext_size,
                            actual: chunk_bytes_read,
                        });
                    }
                }
            }

            // Immediately clean up temporary chunk file
            temp_chunk.cleanup().map_err(RestoreError::Storage)?;
        }

        staging_writer
            .flush()
            .map_err(|e| RestoreError::Io(e.to_string()))?;

        // 7. Whole-file size verification
        if total_restored_bytes != manifest.logical_file.original_size {
            return Err(RestoreError::SizeMismatch {
                file_id: manifest.logical_file.file_id.to_string(),
                expected: manifest.logical_file.original_size,
                actual: total_restored_bytes,
            });
        }

        // 8. Whole-file cryptographic hash verification
        let calculated_hash = format!("{:x}", whole_file_hasher.finalize());
        let expected_hash = &manifest.integrity.digest;

        if !expected_hash.is_empty() && !calculated_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(RestoreError::IntegrityMismatch {
                file_id: manifest.logical_file.file_id.to_string(),
                expected: expected_hash.to_string(),
                actual: calculated_hash,
            });
        }

        if cancellation.is_cancelled() {
            return Err(RestoreError::Cancelled);
        }

        // 9. Atomic finalization to destination path
        if let Some(parent) = final_target.parent() {
            fs::create_dir_all(parent).map_err(|e| RestoreError::Io(e.to_string()))?;
        }

        // Move verified temporary staging file to final target
        if fs::rename(staging_file.path(), &final_target).is_err() {
            // Fallback for cross-device links (EXDEV): stream copy and unlink temp file
            let mut src =
                File::open(staging_file.path()).map_err(|e| RestoreError::Io(e.to_string()))?;
            let mut dst =
                File::create(&final_target).map_err(|e| RestoreError::Io(e.to_string()))?;
            let mut copy_buf = [0u8; RESTORE_STREAM_BUFFER_SIZE];
            loop {
                let n = src
                    .read(&mut copy_buf)
                    .map_err(|e| RestoreError::Io(e.to_string()))?;
                if n == 0 {
                    break;
                }
                dst.write_all(&copy_buf[..n])
                    .map_err(|e| RestoreError::Io(e.to_string()))?;
            }
            dst.flush().map_err(|e| RestoreError::Io(e.to_string()))?;
            let _ = staging_file.cleanup();
        } else {
            // Renamed successfully: disown so RAII Drop does not delete final_target
            let _ = staging_file.disown();
        }

        let outcome = if !destination_existed {
            FileRestoreOutcome::Restored
        } else if collision_policy == CollisionPolicy::Overwrite {
            FileRestoreOutcome::Overwritten
        } else {
            FileRestoreOutcome::KeptBoth
        };

        Ok(RestoreResult {
            file_id: manifest.logical_file.file_id.clone(),
            relative_path: manifest.logical_file.relative_path.clone(),
            target_path: final_target,
            outcome,
            bytes_restored: total_restored_bytes,
            verified_sha256: Some(calculated_hash),
            elapsed_ms: start_time.elapsed().as_millis() as u64,
            error: None,
        })
    }
}
