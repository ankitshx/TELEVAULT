//! Backup status checker exposing file and profile status queries for future UI consumption.

use crate::change_detector::ChangeDetector;
use crate::error::{BackupError, Result};
use crate::profile::BackupProfile;
use crate::scanner::FileScanner;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use televault_core::ids::{FileId, ProfileId, SnapshotId, VersionId};
use televault_db::{Database, VersionRecord};

/// Comprehensive status of a specific file within a backup profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBackupStatus {
    /// Whether the file is tracked in the local backup database.
    pub is_tracked: bool,
    /// Associated logical file identifier if tracked.
    pub file_id: Option<FileId>,
    /// Latest backed-up version identifier.
    pub latest_version_id: Option<VersionId>,
    /// Latest snapshot identifier containing this file.
    pub latest_snapshot_id: Option<SnapshotId>,
    /// Whether the file has at least one completed remote backup.
    pub is_backed_up: bool,
    /// Whether the file has local uncommitted modifications.
    pub is_modified_locally: bool,
    /// Whether the authoritative remote manifest is present in SQLite catalog.
    pub manifest_available: bool,
    /// Whether all associated physical chunks are remotely verified.
    pub all_chunks_verified: bool,
    /// Total chunks comprising the file.
    pub total_chunks: usize,
    /// Number of verified chunks.
    pub verified_chunks: usize,
}

/// Backup checker service for querying backup health, file states, and incremental needs.
pub struct BackupChecker {
    db: Arc<Database>,
}

impl BackupChecker {
    /// Creates a new `BackupChecker` instance backed by the database.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Queries the detailed backup state for a specific relative file path in a profile.
    pub fn check_file_status(
        &self,
        profile_id: &ProfileId,
        relative_path: &str,
    ) -> Result<FileBackupStatus> {
        let file_record = self
            .db
            .get_file_by_relative_path(profile_id, relative_path)
            .map_err(BackupError::from)?;

        let file = match file_record {
            Some(f) => f,
            None => {
                return Ok(FileBackupStatus {
                    is_tracked: false,
                    file_id: None,
                    latest_version_id: None,
                    latest_snapshot_id: None,
                    is_backed_up: false,
                    is_modified_locally: true,
                    manifest_available: false,
                    all_chunks_verified: false,
                    total_chunks: 0,
                    verified_chunks: 0,
                });
            }
        };

        // Query versions of this file
        let versions = self
            .db
            .list_versions_by_file(&file.file_id)
            .map_err(BackupError::from)?;

        let latest_ver = versions.last();
        let latest_version_id = latest_ver.map(|v| v.version_id.clone());
        let latest_snapshot_id = latest_ver.map(|v| v.snapshot_id.clone());

        // Check manifest availability
        let manifest_available = match latest_ver {
            Some(v) => self
                .db
                .get_manifest(&v.manifest_id)
                .map_err(BackupError::from)?
                .is_some(),
            None => false,
        };

        // Check chunk verification status
        let chunks = self
            .db
            .list_chunks_by_file(&file.file_id)
            .map_err(BackupError::from)?;

        let total_chunks = chunks.len();
        let verified_chunks = chunks.iter().filter(|c| c.status == "verified").count();
        let all_chunks_verified = total_chunks > 0 && verified_chunks == total_chunks;

        let is_backed_up = manifest_available && all_chunks_verified;

        // Check if local file currently differs from DB record
        let full_path = std::path::Path::new(&relative_path);
        let is_modified_locally = if full_path.exists() {
            match std::fs::metadata(full_path) {
                Ok(m) => m.len() != file.original_size,
                Err(_) => false,
            }
        } else {
            false
        };

        Ok(FileBackupStatus {
            is_tracked: true,
            file_id: Some(file.file_id),
            latest_version_id,
            latest_snapshot_id,
            is_backed_up,
            is_modified_locally,
            manifest_available,
            all_chunks_verified,
            total_chunks,
            verified_chunks,
        })
    }

    /// Determines whether an incremental backup is needed for a profile.
    pub fn is_incremental_backup_needed(&self, profile: &BackupProfile) -> Result<bool> {
        let scan_result = FileScanner::scan_directory(&profile.source_path, profile)?;

        let latest_snap = self
            .db
            .get_latest_snapshot(&profile.profile_id)
            .map_err(BackupError::from)?;

        let previous_records = match latest_snap {
            Some(snap) => self
                .db
                .list_files_by_snapshot(&snap.snapshot_id)
                .map_err(BackupError::from)?,
            None => return Ok(!scan_result.files.is_empty()),
        };

        let change_set = ChangeDetector::detect_changes(&scan_result.files, &previous_records);
        Ok(change_set.has_mutations())
    }

    /// Retrieves all historical versions for a logical file.
    pub fn get_file_versions(&self, file_id: &FileId) -> Result<Vec<VersionRecord>> {
        self.db
            .list_versions_by_file(file_id)
            .map_err(BackupError::from)
    }

    /// Fast metadata verification: verifies manifest invariants, completeness, and non-pending references.
    ///
    /// If `storage_provider` is provided, also queries remote metadata for each chunk
    /// without downloading payload data.
    pub fn verify_manifest_metadata(
        &self,
        manifest: &televault_manifest::ManifestV1,
        storage_provider: Option<&dyn televault_storage::StorageProvider>,
    ) -> crate::restore::ManifestVerificationReport {
        let mut issues = Vec::new();
        let mut remote_objects_verified = false;

        // 1. Schema and structural validation
        if let Err(e) = manifest.validate() {
            issues.push(format!("Manifest validation error: {e}"));
        }

        // 2. Format version check
        if manifest.manifest_version != televault_manifest::ManifestVersion::V1 {
            issues.push(format!(
                "Unsupported manifest version: {}",
                manifest.manifest_version
            ));
        }

        // 3. Expected chunk count validation
        let expected_chunks = televault_manifest::chunk::calculate_expected_chunk_count(
            manifest.logical_file.original_size,
        ) as usize;
        if manifest.chunks.len() != expected_chunks {
            issues.push(format!(
                "Chunk count mismatch: expected {expected_chunks}, found {}",
                manifest.chunks.len()
            ));
        }

        // 4. Inspect each chunk reference
        for chunk in &manifest.chunks {
            if matches!(
                chunk.storage_reference,
                televault_manifest::StorageReference::Pending
            ) {
                issues.push(format!(
                    "Chunk {} (index {}) has pending storage allocation",
                    chunk.chunk_id, chunk.index
                ));
            } else if let Some(provider) = storage_provider {
                match provider.get_metadata(&chunk.storage_reference) {
                    Ok(meta) => {
                        remote_objects_verified = true;
                        if meta.size_bytes != chunk.stored_size {
                            issues.push(format!(
                                "Chunk {} size mismatch on remote: expected {} bytes, remote reports {} bytes",
                                chunk.chunk_id, chunk.stored_size, meta.size_bytes
                            ));
                        }
                    }
                    Err(e) => {
                        issues.push(format!(
                            "Remote object unavailable for chunk {}: {e}",
                            chunk.chunk_id
                        ));
                    }
                }
            }
        }

        let is_restorable = issues.is_empty();

        crate::restore::ManifestVerificationReport {
            manifest_id: manifest.manifest_id.clone(),
            file_id: manifest.logical_file.file_id.clone(),
            relative_path: manifest.logical_file.relative_path.clone(),
            is_restorable,
            total_chunks: manifest.chunks.len(),
            original_size: manifest.logical_file.original_size,
            issues,
            remote_objects_verified,
        }
    }

    /// Full restore verification: downloads and decrypts all chunks in managed staging,
    /// computing and matching whole-file SHA-256 hash without writing to a user destination.
    pub fn verify_full_restore(
        &self,
        manifest: &televault_manifest::ManifestV1,
        encryption_policy: &televault_crypto::policy::EncryptionPolicy,
        transfer_engine: &televault_transfer::engine::TransferEngine,
        temp_manager: &televault_storage::temp::TempPayloadManager,
        cancellation: &televault_transfer::cancellation::CancellationToken,
    ) -> crate::restore::FullVerificationReport {
        let start = std::time::Instant::now();
        let expected_sha256 = manifest.integrity.digest.clone();

        // Trial restore into a temporary destination file that is cleaned up immediately
        let trial_temp = match temp_manager.create_staging_file("full_verify_trial") {
            Ok(f) => f,
            Err(e) => {
                return crate::restore::FullVerificationReport {
                    manifest_id: manifest.manifest_id.clone(),
                    file_id: manifest.logical_file.file_id.clone(),
                    is_valid: false,
                    total_chunks: manifest.chunks.len(),
                    original_size: manifest.logical_file.original_size,
                    calculated_sha256: None,
                    expected_sha256,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    error: Some(format!("Failed to create temporary staging file: {e}")),
                };
            }
        };

        let res = crate::restore::RestorePipeline::restore_file(
            manifest,
            trial_temp.path(),
            crate::restore::CollisionPolicy::Overwrite,
            encryption_policy,
            transfer_engine,
            temp_manager,
            cancellation,
        );

        let _ = trial_temp.cleanup();

        match res {
            Ok(restore_result) => crate::restore::FullVerificationReport {
                manifest_id: manifest.manifest_id.clone(),
                file_id: manifest.logical_file.file_id.clone(),
                is_valid: true,
                total_chunks: manifest.chunks.len(),
                original_size: manifest.logical_file.original_size,
                calculated_sha256: restore_result.verified_sha256,
                expected_sha256,
                elapsed_ms: start.elapsed().as_millis() as u64,
                error: None,
            },
            Err(err) => crate::restore::FullVerificationReport {
                manifest_id: manifest.manifest_id.clone(),
                file_id: manifest.logical_file.file_id.clone(),
                is_valid: false,
                total_chunks: manifest.chunks.len(),
                original_size: manifest.logical_file.original_size,
                calculated_sha256: None,
                expected_sha256,
                elapsed_ms: start.elapsed().as_millis() as u64,
                error: Some(err.to_string()),
            },
        }
    }
}
