//! Orchestration engine coordinating remote backup repair, previews, and history tracking.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use televault_core::ids::{FileId, ProfileId, SnapshotId};
use televault_crypto::policy::EncryptionPolicy;
use televault_db::{Database, RepairHistoryRecord};
use televault_integrity::types::VerificationOptions;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::TransferEngine;

use super::eligibility::RepairEligibilityChecker;
use super::pipeline::RepairPipeline;
use super::types::{RepairChunkResult, RepairExecutionResult, RepairPreview};
use crate::error::{RepairError, RepairOpResult};
use crate::verification::VerificationEngine;

/// RAII lock guard holding exclusive execution rights for a profile during repair operations.
pub struct RepairGuard {
    profile_id: ProfileId,
    active_set: Arc<Mutex<HashSet<ProfileId>>>,
}

impl Drop for RepairGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = self.active_set.lock() {
            set.remove(&self.profile_id);
        }
    }
}

/// High-level orchestration engine for remote backup repair and recovery.
#[derive(Clone)]
pub struct RepairEngine {
    db: Arc<Database>,
    transfer_engine: Arc<TransferEngine>,
    temp_manager: Arc<TempPayloadManager>,
    verification_engine: Arc<VerificationEngine>,
    active_profiles: Arc<Mutex<HashSet<ProfileId>>>,
    encryption_policy: Arc<EncryptionPolicy>,
}

impl RepairEngine {
    /// Constructs a new [`RepairEngine`].
    pub fn new(
        db: Arc<Database>,
        transfer_engine: Arc<TransferEngine>,
        temp_manager: Arc<TempPayloadManager>,
        verification_engine: Arc<VerificationEngine>,
    ) -> Self {
        Self {
            db,
            transfer_engine,
            temp_manager,
            verification_engine,
            active_profiles: Arc::new(Mutex::new(HashSet::new())),
            encryption_policy: Arc::new(EncryptionPolicy::Disabled),
        }
    }

    /// Sets the encryption policy used for encrypted chunk repair.
    pub fn with_encryption_policy(mut self, policy: Arc<EncryptionPolicy>) -> Self {
        self.encryption_policy = policy;
        self
    }

    /// Returns a reference to the embedded database catalog.
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    /// Attempts to acquire an exclusive repair lock for a profile.
    pub fn try_acquire_profile(&self, profile_id: &ProfileId) -> RepairOpResult<RepairGuard> {
        let mut set = self
            .active_profiles
            .lock()
            .map_err(|e| RepairError::Internal(format!("Lock poisoned: {e}")))?;

        if set.contains(profile_id) {
            return Err(RepairError::ProfileBusy(profile_id.to_string()));
        }

        set.insert(profile_id.clone());
        Ok(RepairGuard {
            profile_id: profile_id.clone(),
            active_set: Arc::clone(&self.active_profiles),
        })
    }

    /// Checks if a repair operation is currently active for the specified profile.
    pub fn is_profile_active(&self, profile_id: &ProfileId) -> bool {
        self.active_profiles
            .lock()
            .map(|set| set.contains(profile_id))
            .unwrap_or(false)
    }

    /// Generates a preview analysis for repairing a logical file without mutating local or remote state.
    pub fn preview_file_repair(
        &self,
        profile_id: &ProfileId,
        file_id: &FileId,
        cancellation: &CancellationToken,
    ) -> RepairOpResult<RepairPreview> {
        let eligibility =
            RepairEligibilityChecker::new(&self.db, profile_id, &self.encryption_policy);
        let profile = eligibility.validate_profile()?;

        let manifest = self
            .db
            .get_manifest_by_file_id(file_id)
            .map_err(RepairError::Database)?
            .ok_or_else(|| {
                RepairError::NotFound(format!(
                    "Manifest for file '{file_id}' not found in catalog"
                ))
            })?;

        eligibility.validate_ownership_chain(&manifest, file_id, None)?;
        let source_path = eligibility.validate_local_source(&profile, &manifest)?;
        eligibility.validate_crypto_prerequisites(&manifest)?;

        // Run full verification to uncover current findings
        let verify_result = self
            .verification_engine
            .verify_file(
                profile_id,
                file_id,
                &VerificationOptions::restore_readiness(),
                cancellation,
            )
            .map_err(|e| RepairError::Internal(format!("Pre-repair verification failed: {e}")))?;

        let (candidates, ineligible) = eligibility.resolve_candidates(
            &profile,
            &manifest,
            &verify_result.findings,
            &source_path,
            None,
        )?;

        let total_chunks = candidates.len() as u32;
        let total_bytes: u64 = candidates.iter().map(|c| c.stored_size).sum();
        let is_repairable = total_chunks > 0;

        Ok(RepairPreview {
            profile_id: profile_id.clone(),
            target_type: "file".into(),
            target_id: file_id.to_string(),
            eligible_candidates: candidates,
            ineligible_findings: ineligible,
            total_affected_chunks: total_chunks,
            total_estimated_bytes: total_bytes,
            is_repairable,
        })
    }

    /// Generates a preview analysis for repairing all damaged files in a snapshot.
    pub fn preview_snapshot_repair(
        &self,
        profile_id: &ProfileId,
        snapshot_id: &SnapshotId,
        cancellation: &CancellationToken,
    ) -> RepairOpResult<RepairPreview> {
        let eligibility =
            RepairEligibilityChecker::new(&self.db, profile_id, &self.encryption_policy);
        let profile = eligibility.validate_profile()?;

        // Verify snapshot belongs to this profile
        let snapshot = self
            .db
            .get_snapshot(snapshot_id)
            .map_err(RepairError::Database)?
            .ok_or_else(|| {
                RepairError::NotFound(format!("Snapshot '{snapshot_id}' not found in catalog"))
            })?;

        if snapshot.profile_id != *profile_id {
            return Err(RepairError::OwnershipViolation {
                reason: format!(
                    "Snapshot '{snapshot_id}' belongs to profile '{}', not '{profile_id}'",
                    snapshot.profile_id
                ),
            });
        }

        // Run snapshot verification
        let verify_result = self
            .verification_engine
            .verify_snapshot(
                profile_id,
                snapshot_id,
                &VerificationOptions::restore_readiness(),
                cancellation,
            )
            .map_err(|e| RepairError::Internal(format!("Snapshot verification failed: {e}")))?;

        let mut all_candidates = Vec::new();
        let mut all_ineligible = Vec::new();

        let versions = self
            .db
            .list_versions_by_snapshot(snapshot_id)
            .map_err(RepairError::Database)?;

        for version in versions {
            let manifest = match self
                .db
                .get_manifest(&version.manifest_id)
                .map_err(RepairError::Database)?
            {
                Some(m) => m,
                None => continue,
            };

            eligibility.validate_ownership_chain(&manifest, &version.file_id, Some(snapshot_id))?;

            let source_path = match eligibility.validate_local_source(&profile, &manifest) {
                Ok(p) => p,
                Err(e) => {
                    all_ineligible.push(
                        televault_integrity::types::VerificationFinding::new(
                            televault_integrity::types::VerificationLevel::MetadataOnly,
                            televault_integrity::types::VerificationSeverity::Critical,
                            televault_integrity::types::VerificationIssueCode::FileNotFound,
                            format!("Local source file validation failed: {e}"),
                            televault_integrity::types::RestoreImpact::Fatal,
                        )
                        .with_file(version.file_id.clone()),
                    );
                    continue;
                }
            };

            let (file_cand, file_inel) = eligibility.resolve_candidates(
                &profile,
                &manifest,
                &verify_result.findings,
                &source_path,
                Some(snapshot_id),
            )?;

            all_candidates.extend(file_cand);
            all_ineligible.extend(file_inel);
        }

        let total_chunks = all_candidates.len() as u32;
        let total_bytes: u64 = all_candidates.iter().map(|c| c.stored_size).sum();

        Ok(RepairPreview {
            profile_id: profile_id.clone(),
            target_type: "snapshot".into(),
            target_id: snapshot_id.to_string(),
            eligible_candidates: all_candidates,
            ineligible_findings: all_ineligible,
            total_affected_chunks: total_chunks,
            total_estimated_bytes: total_bytes,
            is_repairable: total_chunks > 0,
        })
    }

    /// Executes repair for a specific logical file.
    ///
    /// Mutual exclusion: acquires exclusive [`RepairGuard`] to ensure repair never races
    /// with an ongoing repair on the same profile.
    pub fn repair_file(
        &self,
        profile_id: &ProfileId,
        file_id: &FileId,
        dry_run: bool,
        cancellation: &CancellationToken,
    ) -> RepairOpResult<RepairExecutionResult> {
        let start = Instant::now();

        // 1. Acquire execution guard lock for mutual exclusion
        let _guard = self.try_acquire_profile(profile_id)?;

        // 2. Generate preview and resolve candidates
        let preview = self.preview_file_repair(profile_id, file_id, cancellation)?;

        if dry_run {
            let chunk_results = preview
                .eligible_candidates
                .iter()
                .map(|c| RepairChunkResult {
                    chunk_id: c.chunk_id.clone(),
                    chunk_index: c.chunk_index,
                    status: "dry_run".into(),
                    old_storage_reference: c.old_storage_reference.clone(),
                    new_storage_reference: None,
                    bytes_processed: 0,
                    duration_ms: 0,
                    error: None,
                })
                .collect();

            return Ok(RepairExecutionResult {
                profile_id: profile_id.clone(),
                target_type: "file".into(),
                target_id: file_id.to_string(),
                total_chunks_evaluated: preview.total_affected_chunks,
                repaired_chunks: 0,
                skipped_healthy_chunks: 0,
                failed_chunks: 0,
                total_bytes_transferred: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                chunk_results,
            });
        }

        let manifest = self
            .db
            .get_manifest_by_file_id(file_id)
            .map_err(RepairError::Database)?
            .ok_or_else(|| {
                RepairError::NotFound(format!(
                    "Manifest for file '{file_id}' not found in catalog"
                ))
            })?;

        let mut chunk_results = Vec::new();
        let mut repaired_count = 0;
        let mut skipped_count = 0;
        let mut failed_count = 0;
        let mut total_transferred = 0;

        for candidate in &preview.eligible_candidates {
            if cancellation.is_cancelled() {
                chunk_results.push(RepairChunkResult {
                    chunk_id: candidate.chunk_id.clone(),
                    chunk_index: candidate.chunk_index,
                    status: "cancelled".into(),
                    old_storage_reference: candidate.old_storage_reference.clone(),
                    new_storage_reference: None,
                    bytes_processed: 0,
                    duration_ms: 0,
                    error: Some("Operation cancelled".into()),
                });
                break;
            }

            match RepairPipeline::repair_single_chunk(
                candidate,
                &manifest,
                &self.db,
                &self.transfer_engine,
                &self.temp_manager,
                &self.encryption_policy,
                cancellation,
            ) {
                Ok(res) => {
                    if res.status == "success" {
                        repaired_count += 1;
                        total_transferred += res.bytes_processed;
                    } else if res.status == "skipped_healthy" {
                        skipped_count += 1;
                    }
                    chunk_results.push(res);
                }
                Err(e) => {
                    failed_count += 1;
                    chunk_results.push(RepairChunkResult {
                        chunk_id: candidate.chunk_id.clone(),
                        chunk_index: candidate.chunk_index,
                        status: "failed".into(),
                        old_storage_reference: candidate.old_storage_reference.clone(),
                        new_storage_reference: None,
                        bytes_processed: 0,
                        duration_ms: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(RepairExecutionResult {
            profile_id: profile_id.clone(),
            target_type: "file".into(),
            target_id: file_id.to_string(),
            total_chunks_evaluated: preview.total_affected_chunks,
            repaired_chunks: repaired_count,
            skipped_healthy_chunks: skipped_count,
            failed_chunks: failed_count,
            total_bytes_transferred: total_transferred,
            duration_ms: start.elapsed().as_millis() as u64,
            chunk_results,
        })
    }

    /// Executes repair across all files in a snapshot.
    pub fn repair_snapshot(
        &self,
        profile_id: &ProfileId,
        snapshot_id: &SnapshotId,
        dry_run: bool,
        cancellation: &CancellationToken,
    ) -> RepairOpResult<RepairExecutionResult> {
        let start = Instant::now();

        let _guard = self.try_acquire_profile(profile_id)?;

        let preview = self.preview_snapshot_repair(profile_id, snapshot_id, cancellation)?;

        if dry_run {
            let chunk_results = preview
                .eligible_candidates
                .iter()
                .map(|c| RepairChunkResult {
                    chunk_id: c.chunk_id.clone(),
                    chunk_index: c.chunk_index,
                    status: "dry_run".into(),
                    old_storage_reference: c.old_storage_reference.clone(),
                    new_storage_reference: None,
                    bytes_processed: 0,
                    duration_ms: 0,
                    error: None,
                })
                .collect();

            return Ok(RepairExecutionResult {
                profile_id: profile_id.clone(),
                target_type: "snapshot".into(),
                target_id: snapshot_id.to_string(),
                total_chunks_evaluated: preview.total_affected_chunks,
                repaired_chunks: 0,
                skipped_healthy_chunks: 0,
                failed_chunks: 0,
                total_bytes_transferred: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                chunk_results,
            });
        }

        let mut chunk_results = Vec::new();
        let mut repaired_count = 0;
        let mut skipped_count = 0;
        let mut failed_count = 0;
        let mut total_transferred = 0;

        for candidate in &preview.eligible_candidates {
            if cancellation.is_cancelled() {
                chunk_results.push(RepairChunkResult {
                    chunk_id: candidate.chunk_id.clone(),
                    chunk_index: candidate.chunk_index,
                    status: "cancelled".into(),
                    old_storage_reference: candidate.old_storage_reference.clone(),
                    new_storage_reference: None,
                    bytes_processed: 0,
                    duration_ms: 0,
                    error: Some("Operation cancelled".into()),
                });
                break;
            }

            let manifest = match self
                .db
                .get_manifest(&candidate.manifest_id)
                .map_err(RepairError::Database)?
            {
                Some(m) => m,
                None => {
                    failed_count += 1;
                    chunk_results.push(RepairChunkResult {
                        chunk_id: candidate.chunk_id.clone(),
                        chunk_index: candidate.chunk_index,
                        status: "failed".into(),
                        old_storage_reference: candidate.old_storage_reference.clone(),
                        new_storage_reference: None,
                        bytes_processed: 0,
                        duration_ms: 0,
                        error: Some(format!("Manifest '{}' not found", candidate.manifest_id)),
                    });
                    continue;
                }
            };

            match RepairPipeline::repair_single_chunk(
                candidate,
                &manifest,
                &self.db,
                &self.transfer_engine,
                &self.temp_manager,
                &self.encryption_policy,
                cancellation,
            ) {
                Ok(res) => {
                    if res.status == "success" {
                        repaired_count += 1;
                        total_transferred += res.bytes_processed;
                    } else if res.status == "skipped_healthy" {
                        skipped_count += 1;
                    }
                    chunk_results.push(res);
                }
                Err(e) => {
                    failed_count += 1;
                    chunk_results.push(RepairChunkResult {
                        chunk_id: candidate.chunk_id.clone(),
                        chunk_index: candidate.chunk_index,
                        status: "failed".into(),
                        old_storage_reference: candidate.old_storage_reference.clone(),
                        new_storage_reference: None,
                        bytes_processed: 0,
                        duration_ms: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(RepairExecutionResult {
            profile_id: profile_id.clone(),
            target_type: "snapshot".into(),
            target_id: snapshot_id.to_string(),
            total_chunks_evaluated: preview.total_affected_chunks,
            repaired_chunks: repaired_count,
            skipped_healthy_chunks: skipped_count,
            failed_chunks: failed_count,
            total_bytes_transferred: total_transferred,
            duration_ms: start.elapsed().as_millis() as u64,
            chunk_results,
        })
    }

    /// Retrieves repair audit history records for a profile.
    pub fn get_repair_history(
        &self,
        profile_id: &ProfileId,
        limit: usize,
    ) -> RepairOpResult<Vec<RepairHistoryRecord>> {
        self.db
            .list_repair_history(profile_id, limit)
            .map_err(RepairError::Database)
    }
}
