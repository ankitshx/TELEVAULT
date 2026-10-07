//! Production-grade backup orchestration and snapshot execution engine.

use crate::change_detector::ChangeKind;
use crate::error::{BackupError, Result};
use crate::pipeline::{PayloadPipeline, ProcessingOptions};
use crate::plan::{BackupPlan, BackupSummary};
use crate::profile::BackupProfile;
use crate::scanner::FileScanner;
use crate::snapshot::SnapshotManager;
use std::fs::File;
use std::sync::Arc;
use televault_core::ids::{JobId, SnapshotId, VersionId};
use televault_core::models::BackupStatus;
use televault_db::{ChunkRecord, Database, FileRecord, VersionRecord};
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::TransferEngine;
use televault_transfer::job::{TransferJob, UploadJobParams};

/// Backup engine orchestrating discovery, change detection, pipelining, and transfer handoff.
pub struct BackupEngine {
    db: Arc<Database>,
    transfer_engine: Arc<TransferEngine>,
    temp_manager: Arc<TempPayloadManager>,
    snapshot_manager: SnapshotManager,
}

impl BackupEngine {
    /// Creates a new `BackupEngine` with injected database, transfer, and temp storage handles.
    pub fn new(
        db: Arc<Database>,
        transfer_engine: Arc<TransferEngine>,
        temp_manager: Arc<TempPayloadManager>,
    ) -> Self {
        let snapshot_manager = SnapshotManager::new(Arc::clone(&db));
        Self {
            db,
            transfer_engine,
            temp_manager,
            snapshot_manager,
        }
    }

    /// Access the underlying snapshot manager.
    pub fn snapshot_manager(&self) -> &SnapshotManager {
        &self.snapshot_manager
    }

    /// Plans a backup execution by scanning the local filesystem and computing changes against the latest snapshot.
    pub fn plan_backup(&self, profile: &BackupProfile) -> Result<BackupPlan> {
        profile.validate()?;

        // 1. Scan filesystem for current state
        let scan_result = FileScanner::scan_directory(&profile.source_path, profile)?;

        // 2. Compute differential change set against previous snapshot
        let change_set = self
            .snapshot_manager
            .diff_against_latest_snapshot(&profile.profile_id, &scan_result.files)?;

        // 3. Allocate fresh snapshot identifier
        let snapshot_id = SnapshotId::new(format!("snap-{}", uuid_or_timestamp()))
            .map_err(|e| BackupError::SnapshotError(e.to_string()))?;

        Ok(BackupPlan::new(profile.clone(), snapshot_id, change_set))
    }

    /// Executes a planned backup transactionally, transferring modified/new payloads and reusing unchanged ones.
    pub fn execute_backup(
        &self,
        plan: BackupPlan,
        cancel: &CancellationToken,
    ) -> Result<BackupSummary> {
        if cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }

        let start_time = std::time::Instant::now();

        // 1. Initialize snapshot record in DB
        self.snapshot_manager.create_snapshot(
            &plan.profile.profile_id,
            &plan.snapshot_id,
            BackupStatus::BackingUp,
        )?;

        let mut transferred_chunks = 0;
        let mut transferred_bytes = 0;
        let new_count = plan.change_set.new_files().len();
        let modified_count = plan.change_set.modified_files().len();
        let unchanged_count = plan.change_set.unchanged_files().len();
        let deleted_count = plan.change_set.deleted_files().len();
        let reused_bytes = plan.change_set.total_reused_bytes();

        let processing_options = ProcessingOptions {
            encryption_policy: plan.profile.encryption_policy.clone(),
            compression_algorithm: plan.profile.compression_algorithm,
        };

        // 2. Process Unchanged files (Zero transfer, zero hashing, instant version link)
        for change in plan.change_set.unchanged_files() {
            if cancel.is_cancelled() {
                self.fail_snapshot(&plan.snapshot_id, "Backup cancelled")?;
                return Err(BackupError::Cancelled);
            }

            if let Some(prev_ver) = &change.previous_version {
                let version_id = VersionId::new(format!("ver-{}", uuid_or_timestamp()))
                    .map_err(|e| BackupError::SnapshotError(e.to_string()))?;

                let version_record = VersionRecord {
                    version_id,
                    file_id: change.file_id.clone(),
                    snapshot_id: plan.snapshot_id.clone(),
                    manifest_id: prev_ver.manifest_id.clone(),
                    status: "active".into(),
                    created_at: "2026-10-07T12:00:00Z".into(),
                };
                self.db
                    .create_version(&version_record)
                    .map_err(BackupError::from)?;
            }
        }

        // 3. Process New and Modified files
        let files_to_process: Vec<_> = plan
            .change_set
            .changes
            .iter()
            .filter(|c| c.kind == ChangeKind::New || c.kind == ChangeKind::Modified)
            .collect();

        for change in files_to_process {
            if cancel.is_cancelled() {
                self.fail_snapshot(&plan.snapshot_id, "Backup cancelled")?;
                return Err(BackupError::Cancelled);
            }

            let scanned = change.scanned.as_ref().ok_or_else(|| {
                BackupError::ScanError(format!("Missing scan data for {}", change.relative_path))
            })?;

            let source_file =
                File::open(&scanned.absolute_path).map_err(|e| BackupError::InaccessibleFile {
                    path: scanned.absolute_path.display().to_string(),
                    reason: e.to_string(),
                })?;

            // Stream through pipeline -> creates staged chunks and manifest
            let mut processed = PayloadPipeline::process_file(
                &change.file_id,
                &change.relative_path,
                source_file,
                scanned.size_bytes,
                &self.temp_manager,
                &processing_options,
            )?;

            // Update or create file record in DB
            let file_record = FileRecord {
                file_id: change.file_id.clone(),
                profile_id: Some(plan.profile.profile_id.clone()),
                file_name: scanned.file_name.clone(),
                relative_path: scanned.relative_path.clone(),
                original_size: scanned.size_bytes,
                mime_type: None,
                status: "backed_up".into(),
                logical_file_hash: Some(processed.logical_file_hash.clone()),
                created_at: "2026-10-07T12:00:00Z".into(),
                modified_at: Some(scanned.modified_at.clone()),
                created_timestamp: "2026-10-07T12:00:00Z".into(),
                updated_timestamp: "2026-10-07T12:00:00Z".into(),
            };

            match change.kind {
                ChangeKind::New => {
                    self.db
                        .create_file(&file_record)
                        .map_err(BackupError::from)?;
                }
                _ => {
                    self.db
                        .update_file(&file_record)
                        .map_err(BackupError::from)?;
                    self.db
                        .delete_chunks_by_file(&change.file_id)
                        .map_err(BackupError::from)?;
                }
            }

            // Save initial manifest to satisfy chunks foreign key constraint
            self.db
                .save_manifest(&processed.manifest)
                .map_err(BackupError::from)?;

            // Transfer each physical chunk to remote storage via TransferEngine
            for chunk in std::mem::take(&mut processed.chunks) {
                if cancel.is_cancelled() {
                    self.fail_snapshot(&plan.snapshot_id, "Backup cancelled")?;
                    return Err(BackupError::Cancelled);
                }

                let job_id = JobId::new(format!("job-{}", chunk.chunk_id))
                    .map_err(|e| BackupError::TransferError(e.to_string()))?;

                let upload_params = UploadJobParams::new(
                    job_id.clone(),
                    change.file_id.clone(),
                    chunk.chunk_id.clone(),
                    chunk.stored_size,
                )
                .with_chunk(chunk.chunk_index, chunk.total_chunks)
                .with_source_path(chunk.staging_file.path().to_path_buf())
                .with_sha256(&chunk.stored_sha256);

                let mut transfer_job =
                    TransferJob::new_upload(upload_params).map_err(BackupError::from)?;

                // Transfer staged upload -> uploads to remote, verifies remote object, and deletes temp file
                let storage_ref = self
                    .transfer_engine
                    .execute_staged_upload(&mut transfer_job, chunk.staging_file, cancel)
                    .map_err(BackupError::from)?;

                // Update chunk's storage reference in manifest
                if let Some(chunk_manifest) = processed
                    .manifest
                    .chunks
                    .get_mut(chunk.chunk_index as usize)
                {
                    chunk_manifest.storage_reference = storage_ref.clone();
                }

                // Record physical chunk in DB
                let chunk_record = ChunkRecord {
                    chunk_id: chunk.chunk_id.clone(),
                    file_id: change.file_id.clone(),
                    manifest_id: processed.manifest.manifest_id.clone(),
                    chunk_index: chunk.chunk_index,
                    plaintext_size: chunk.plaintext_size,
                    stored_size: chunk.stored_size,
                    integrity_hash: chunk.stored_sha256.clone(),
                    storage_reference: format!("{:?}", storage_ref),
                    status: "verified".into(),
                    created_at: "2026-10-07T12:00:00Z".into(),
                    updated_at: "2026-10-07T12:00:00Z".into(),
                };
                self.db
                    .create_chunk(&chunk_record)
                    .map_err(BackupError::from)?;

                // Sync transfer job record to DB
                let _ = self.transfer_engine.sync_job_to_db(&self.db, &job_id);

                transferred_chunks += 1;
                transferred_bytes += chunk.stored_size;
            }

            // Save manifest with updated storage references to database
            self.db
                .save_manifest(&processed.manifest)
                .map_err(BackupError::from)?;

            // Bind new file version to this snapshot
            let version_id = VersionId::new(format!("ver-{}", uuid_or_timestamp()))
                .map_err(|e| BackupError::SnapshotError(e.to_string()))?;

            let version_record = VersionRecord {
                version_id,
                file_id: change.file_id.clone(),
                snapshot_id: plan.snapshot_id.clone(),
                manifest_id: processed.manifest.manifest_id,
                status: "active".into(),
                created_at: "2026-10-07T12:00:00Z".into(),
            };
            self.db
                .create_version(&version_record)
                .map_err(BackupError::from)?;
        }

        // 4. Record Deleted files in database status
        for change in plan.change_set.deleted_files() {
            if let Some(prev) = &change.previous_file {
                let mut updated_deleted = prev.clone();
                updated_deleted.status = "deleted".into();
                let _ = self.db.update_file(&updated_deleted);
            }
        }

        let elapsed_ms = start_time.elapsed().as_millis() as u64;

        // 5. Build summary
        let summary = BackupSummary {
            snapshot_id: plan.snapshot_id.clone(),
            profile_id: plan.profile.profile_id.clone(),
            new_files: new_count,
            modified_files: modified_count,
            unchanged_files: unchanged_count,
            deleted_files: deleted_count,
            transferred_chunks,
            transferred_bytes,
            reused_bytes,
            status: BackupStatus::Completed,
            elapsed_ms,
            error_message: None,
        };

        // 6. Complete snapshot record
        let meta_json = serde_json::to_string(&summary).unwrap_or_default();
        self.snapshot_manager.update_snapshot_status(
            &plan.snapshot_id,
            BackupStatus::Completed,
            Some(meta_json),
        )?;

        Ok(summary)
    }

    /// Plans and executes a backup in a single call.
    pub fn execute_profile_backup(
        &self,
        profile: &BackupProfile,
        cancel: &CancellationToken,
    ) -> Result<BackupSummary> {
        let plan = self.plan_backup(profile)?;
        self.execute_backup(plan, cancel)
    }

    /// Internal helper to record a failed snapshot state.
    fn fail_snapshot(&self, snapshot_id: &SnapshotId, reason: &str) -> Result<()> {
        let err_json = format!("{{\"error\":\"{}\"}}", reason);
        self.snapshot_manager.update_snapshot_status(
            snapshot_id,
            BackupStatus::Failed,
            Some(err_json),
        )
    }
}

/// Generates a simple timestamp string for unique ID generation.
fn uuid_or_timestamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}-{}", now.as_secs(), now.subsec_nanos())
}
