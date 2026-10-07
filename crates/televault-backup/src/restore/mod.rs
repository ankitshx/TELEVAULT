//! Production-grade restore engine and backup verification services.

pub mod collision;
pub mod pipeline;
pub mod types;

pub use collision::{
    generate_alternate_path, resolve_collision, CollisionPolicy, CollisionResolution,
};
pub use pipeline::RestorePipeline;
pub use types::{
    FileRestoreOutcome, FullVerificationReport, ManifestVerificationReport, RestoreRequest,
    RestoreResult, SnapshotRestoreRequest, SnapshotRestoreResult,
};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::Database;
use televault_manifest::ManifestV1;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::TransferEngine;

use crate::error::{RestoreError, RestoreOpResult};

/// High-level orchestration engine for restoring backed-up files and snapshots.
pub struct RestoreEngine {
    db: Arc<Database>,
    transfer_engine: Arc<TransferEngine>,
    temp_manager: Arc<TempPayloadManager>,
}

impl RestoreEngine {
    /// Constructs a new [`RestoreEngine`] using shared application singletons.
    pub fn new(
        db: Arc<Database>,
        transfer_engine: Arc<TransferEngine>,
        temp_manager: Arc<TempPayloadManager>,
    ) -> Self {
        Self {
            db,
            transfer_engine,
            temp_manager,
        }
    }

    /// Restores a single logical file or version according to the provided [`RestoreRequest`].
    pub fn restore_file(
        &self,
        request: &RestoreRequest,
        cancellation: &CancellationToken,
    ) -> RestoreOpResult<RestoreResult> {
        if cancellation.is_cancelled() {
            return Err(RestoreError::Cancelled);
        }

        // 1. Resolve authoritative ManifestV1 from database
        let manifest = if let Some(ref mid) = request.manifest_id {
            self.db
                .get_manifest(mid)?
                .ok_or_else(|| RestoreError::IncompleteBackup {
                    reason: format!("Manifest ID '{mid}' not found in catalog"),
                })?
        } else if let Some(ref vid) = request.version_id {
            let version = self
                .db
                .get_version(vid)?
                .ok_or_else(|| RestoreError::VersionNotFound(vid.to_string()))?;
            self.db.get_manifest(&version.manifest_id)?.ok_or_else(|| {
                RestoreError::IncompleteBackup {
                    reason: format!("Manifest '{}' for version not found", version.manifest_id),
                }
            })?
        } else {
            // Find latest active version for the file
            let versions = self.db.list_versions_by_file(&request.file_id)?;
            let latest_version = versions
                .first()
                .ok_or_else(|| RestoreError::FileNotFound(request.file_id.to_string()))?;
            self.db
                .get_manifest(&latest_version.manifest_id)?
                .ok_or_else(|| RestoreError::IncompleteBackup {
                    reason: format!(
                        "Manifest '{}' for latest version not found",
                        latest_version.manifest_id
                    ),
                })?
        };

        // 2. Resolve destination file path
        let target_path = self.resolve_target_path(&request.destination_path, &manifest);

        // 3. Delegate to streaming restore pipeline
        RestorePipeline::restore_file(
            &manifest,
            &target_path,
            request.collision_policy,
            &request.encryption_policy,
            &self.transfer_engine,
            &self.temp_manager,
            cancellation,
        )
    }

    /// Directly restores a provided [`ManifestV1`] to a target path without requiring database lookup.
    pub fn restore_manifest(
        &self,
        manifest: &ManifestV1,
        destination_path: &Path,
        collision_policy: CollisionPolicy,
        encryption_policy: &EncryptionPolicy,
        cancellation: &CancellationToken,
    ) -> RestoreOpResult<RestoreResult> {
        let target_path = self.resolve_target_path(destination_path, manifest);
        RestorePipeline::restore_file(
            manifest,
            &target_path,
            collision_policy,
            encryption_policy,
            &self.transfer_engine,
            &self.temp_manager,
            cancellation,
        )
    }

    /// Restores all files present in a point-in-time backup snapshot to `destination_dir`.
    pub fn restore_snapshot(
        &self,
        request: &SnapshotRestoreRequest,
        cancellation: &CancellationToken,
    ) -> RestoreOpResult<SnapshotRestoreResult> {
        if cancellation.is_cancelled() {
            return Err(RestoreError::Cancelled);
        }

        let start_time = std::time::Instant::now();

        // 1. Fetch all files and bound version records for this snapshot
        let file_versions = self.db.list_files_by_snapshot(&request.snapshot_id)?;
        if file_versions.is_empty() {
            // Verify if snapshot actually exists
            if self.db.get_snapshot(&request.snapshot_id)?.is_none() {
                return Err(RestoreError::SnapshotNotFound(
                    request.snapshot_id.to_string(),
                ));
            }
        }

        let total_files = file_versions.len();
        let mut restored_files = 0;
        let mut skipped_files = 0;
        let mut overwritten_files = 0;
        let mut kept_both_files = 0;
        let mut failed_files = 0;
        let mut total_bytes_restored = 0;
        let mut file_results = Vec::with_capacity(total_files);

        // 2. Process each file version in the snapshot
        for (file_record, version_record) in file_versions {
            if cancellation.is_cancelled() {
                return Err(RestoreError::Cancelled);
            }

            let manifest_opt = self.db.get_manifest(&version_record.manifest_id)?;
            let manifest = match manifest_opt {
                Some(m) => m,
                None => {
                    failed_files += 1;
                    file_results.push(RestoreResult {
                        file_id: file_record.file_id,
                        relative_path: file_record.relative_path,
                        target_path: PathBuf::new(),
                        outcome: FileRestoreOutcome::Failed,
                        bytes_restored: 0,
                        verified_sha256: None,
                        elapsed_ms: 0,
                        error: Some(format!(
                            "Manifest '{}' not found in database",
                            version_record.manifest_id
                        )),
                    });
                    continue;
                }
            };

            // Reconstruct relative file hierarchy under destination directory
            let target_file_path = request
                .destination_dir
                .join(&manifest.logical_file.relative_path);

            let res = RestorePipeline::restore_file(
                &manifest,
                &target_file_path,
                request.collision_policy,
                &request.encryption_policy,
                &self.transfer_engine,
                &self.temp_manager,
                cancellation,
            );

            match res {
                Ok(result) => {
                    match result.outcome {
                        FileRestoreOutcome::Restored => restored_files += 1,
                        FileRestoreOutcome::Skipped => skipped_files += 1,
                        FileRestoreOutcome::Overwritten => overwritten_files += 1,
                        FileRestoreOutcome::KeptBoth => kept_both_files += 1,
                        FileRestoreOutcome::Cancelled => {
                            return Err(RestoreError::Cancelled);
                        }
                        FileRestoreOutcome::Failed => failed_files += 1,
                    }
                    total_bytes_restored += result.bytes_restored;
                    file_results.push(result);
                }
                Err(err) => {
                    failed_files += 1;
                    file_results.push(RestoreResult {
                        file_id: manifest.logical_file.file_id.clone(),
                        relative_path: manifest.logical_file.relative_path.clone(),
                        target_path: target_file_path,
                        outcome: FileRestoreOutcome::Failed,
                        bytes_restored: 0,
                        verified_sha256: None,
                        elapsed_ms: 0,
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        Ok(SnapshotRestoreResult {
            snapshot_id: request.snapshot_id.clone(),
            total_files,
            restored_files,
            skipped_files,
            overwritten_files,
            kept_both_files,
            failed_files,
            total_bytes_restored,
            elapsed_ms: start_time.elapsed().as_millis() as u64,
            file_results,
        })
    }

    /// Resolves destination path: if `destination_path` is a directory or points to a folder,
    /// appends the logical file name to prevent accidental file clobbering.
    fn resolve_target_path(&self, destination_path: &Path, manifest: &ManifestV1) -> PathBuf {
        if destination_path.is_dir()
            || destination_path
                .to_string_lossy()
                .ends_with(std::path::MAIN_SEPARATOR)
            || destination_path.to_string_lossy().ends_with('/')
        {
            destination_path.join(&manifest.logical_file.file_name)
        } else {
            destination_path.to_path_buf()
        }
    }
}
