//! Production-grade remote backup verification, integrity audit, and ownership isolation engine.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use televault_core::ids::{FileId, ProfileId, SnapshotId};
use televault_crypto::policy::EncryptionPolicy;
use televault_db::{Database, VerificationHistoryRecord};
use televault_integrity::hasher::NullHashWriter;
use televault_integrity::types::{
    RestoreImpact, VerificationFinding, VerificationIssueCode, VerificationLevel,
    VerificationOptions, VerificationResult, VerificationSeverity, VerificationStatus,
    VerificationSummary,
};
use televault_manifest::chunk::{calculate_expected_chunk_count, TARGET_CHUNK_SIZE_BYTES};
use televault_manifest::{ManifestV1, StorageReference};
use televault_storage::error::StorageError;
use televault_storage::provider::StorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_storage::types::DownloadRequest;
use televault_transfer::cancellation::{CancellationToken, CancellingWriter};

use super::ownership::OwnershipValidator;
use crate::error::{BackupError, Result};

/// Core orchestration engine for remote backup verification, integrity audits, and profile isolation.
pub struct VerificationEngine {
    db: Arc<Database>,
    storage_provider: Arc<dyn StorageProvider + Send + Sync>,
    temp_manager: Arc<TempPayloadManager>,
    encryption_policy: Arc<EncryptionPolicy>,
}

impl VerificationEngine {
    /// Creates a new [`VerificationEngine`].
    pub fn new(
        db: Arc<Database>,
        storage_provider: Arc<dyn StorageProvider + Send + Sync>,
        temp_manager: Arc<TempPayloadManager>,
    ) -> Self {
        Self {
            db,
            storage_provider,
            temp_manager,
            encryption_policy: Arc::new(EncryptionPolicy::Disabled),
        }
    }

    /// Sets the encryption policy used for trial decryption verification.
    pub fn with_encryption_policy(mut self, policy: Arc<EncryptionPolicy>) -> Self {
        self.encryption_policy = policy;
        self
    }

    /// Returns a reference to the underlying database.
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    /// Returns a reference to the storage provider.
    pub fn storage_provider(&self) -> &Arc<dyn StorageProvider + Send + Sync> {
        &self.storage_provider
    }

    /// Returns a reference to the staging temp manager.
    pub fn temp_manager(&self) -> &Arc<TempPayloadManager> {
        &self.temp_manager
    }

    /// Returns a reference to the active encryption policy.
    pub fn encryption_policy(&self) -> &Arc<EncryptionPolicy> {
        &self.encryption_policy
    }

    /// Verifies a specific backup manifest against the expected profile.
    pub fn verify_manifest(
        &self,
        profile_id: &ProfileId,
        manifest: &ManifestV1,
        options: &VerificationOptions,
        cancellation: &CancellationToken,
    ) -> Result<VerificationResult> {
        let start = Instant::now();
        let mut findings = Vec::new();
        let target_id = manifest.manifest_id.clone();
        let target_type = "manifest".to_string();

        let ownership_validator = OwnershipValidator::new(&self.db, profile_id);

        // Profile existence check
        if let Err(f) = ownership_validator.validate_profile() {
            findings.push(f);
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                findings,
                1,
                1,
                manifest.chunks.len() as u32,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        if cancellation.is_cancelled() {
            findings.push(self.create_cancelled_finding(profile_id, options.level));
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                findings,
                1,
                1,
                manifest.chunks.len() as u32,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        // =====================================================================
        // LEVEL 1 — LOCAL METADATA & OWNERSHIP VERIFICATION
        // =====================================================================
        let ownership_findings =
            ownership_validator.validate_manifest_ownership_chain(manifest, None, None);
        findings.extend(ownership_findings);

        // Reuse manifest structure validation
        if let Err(manifest_err) = manifest.validate() {
            findings.push(
                VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::ManifestInvalid,
                    format!("Manifest validation error: {manifest_err}"),
                    RestoreImpact::Fatal,
                )
                .with_profile(profile_id.clone())
                .with_file(manifest.logical_file.file_id.clone()),
            );
        }

        // Validate chunk count formula
        let expected_chunks = calculate_expected_chunk_count(manifest.logical_file.original_size);
        if manifest.chunks.len() as u32 != expected_chunks {
            findings.push(
                VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::ChunkCountMismatch,
                    format!(
                        "Manifest has {} chunks, but original size {} bytes requires {} chunks",
                        manifest.chunks.len(),
                        manifest.logical_file.original_size,
                        expected_chunks
                    ),
                    RestoreImpact::Fatal,
                )
                .with_profile(profile_id.clone())
                .with_file(manifest.logical_file.file_id.clone()),
            );
        }

        // Validate contiguous chunk indexes, ordering, and size boundaries
        let mut total_declared_plaintext: u64 = 0;
        let chunk_count = manifest.chunks.len();
        for (i, chunk) in manifest.chunks.iter().enumerate() {
            if chunk.index != i as u32 {
                findings.push(
                    VerificationFinding::new(
                        VerificationLevel::MetadataOnly,
                        VerificationSeverity::Critical,
                        VerificationIssueCode::ChunkIndexDiscontinuous,
                        format!(
                            "Chunk index mismatch at position {i}: expected index {i}, declared {}",
                            chunk.index
                        ),
                        RestoreImpact::Fatal,
                    )
                    .with_profile(profile_id.clone())
                    .with_file(manifest.logical_file.file_id.clone())
                    .with_chunk(chunk.chunk_id.clone(), chunk.index),
                );
            }

            // Chunk size boundary check
            let expected_chunk_size = if chunk_count == 1 {
                manifest.logical_file.original_size
            } else if i < chunk_count - 1 {
                TARGET_CHUNK_SIZE_BYTES
            } else {
                manifest
                    .logical_file
                    .original_size
                    .saturating_sub(i as u64 * TARGET_CHUNK_SIZE_BYTES)
            };

            if chunk.plaintext_size != expected_chunk_size {
                findings.push(
                    VerificationFinding::new(
                        VerificationLevel::MetadataOnly,
                        VerificationSeverity::Critical,
                        VerificationIssueCode::ChunkSizeMismatch,
                        format!(
                            "Chunk {} (index {i}) declared plaintext size {} bytes, expected {} bytes",
                            chunk.chunk_id, chunk.plaintext_size, expected_chunk_size
                        ),
                        RestoreImpact::Fatal,
                    )
                    .with_profile(profile_id.clone())
                    .with_file(manifest.logical_file.file_id.clone())
                    .with_chunk(chunk.chunk_id.clone(), chunk.index),
                );
            }

            total_declared_plaintext += chunk.plaintext_size;
        }

        if total_declared_plaintext != manifest.logical_file.original_size {
            findings.push(
                VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::InvalidLogicalFileSize,
                    format!(
                        "Sum of chunk plaintext sizes ({total_declared_plaintext} bytes) does not equal logical file original size ({})",
                        manifest.logical_file.original_size
                    ),
                    RestoreImpact::Fatal,
                )
                .with_profile(profile_id.clone())
                .with_file(manifest.logical_file.file_id.clone()),
            );
        }

        if options.level == VerificationLevel::MetadataOnly || cancellation.is_cancelled() {
            if cancellation.is_cancelled() {
                findings.push(self.create_cancelled_finding(profile_id, options.level));
            }
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                findings,
                1,
                1,
                chunk_count as u32,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        // =====================================================================
        // LEVEL 2 — REMOTE AVAILABILITY VERIFICATION
        // =====================================================================
        for chunk in &manifest.chunks {
            if cancellation.is_cancelled() {
                findings.push(self.create_cancelled_finding(profile_id, options.level));
                break;
            }

            match &chunk.storage_reference {
                StorageReference::Pending => {
                    // Already flagged at Level 1
                }
                StorageReference::Telegram { .. } | StorageReference::LocalStaging { .. } => {
                    match self.storage_provider.get_metadata(&chunk.storage_reference) {
                        Ok(meta) => {
                            if chunk.stored_size > 0 && meta.size_bytes != chunk.stored_size {
                                findings.push(
                                    VerificationFinding::new(
                                        VerificationLevel::RemoteAvailability,
                                        VerificationSeverity::Error,
                                        VerificationIssueCode::RemoteMetadataMismatch,
                                        format!(
                                            "Remote size mismatch for chunk '{}': remote reports {} bytes, local catalog expects {} bytes",
                                            chunk.chunk_id, meta.size_bytes, chunk.stored_size
                                        ),
                                        RestoreImpact::Fatal,
                                    )
                                    .with_profile(profile_id.clone())
                                    .with_file(manifest.logical_file.file_id.clone())
                                    .with_chunk(chunk.chunk_id.clone(), chunk.index),
                                );
                            }
                        }
                        Err(StorageError::NotFound(err_msg)) => {
                            findings.push(
                                VerificationFinding::new(
                                    VerificationLevel::RemoteAvailability,
                                    VerificationSeverity::Critical,
                                    VerificationIssueCode::ConfirmedMissing,
                                    format!(
                                        "Chunk '{}' (index {}) confirmed missing on remote storage: {err_msg}",
                                        chunk.chunk_id, chunk.index
                                    ),
                                    RestoreImpact::Fatal,
                                )
                                .with_profile(profile_id.clone())
                                .with_file(manifest.logical_file.file_id.clone())
                                .with_chunk(chunk.chunk_id.clone(), chunk.index),
                            );
                        }
                        Err(StorageError::InvalidReference(err_msg)) => {
                            findings.push(
                                VerificationFinding::new(
                                    VerificationLevel::RemoteAvailability,
                                    VerificationSeverity::Critical,
                                    VerificationIssueCode::InvalidTelegramReference,
                                    format!(
                                        "Chunk '{}' storage reference is invalid: {err_msg}",
                                        chunk.chunk_id
                                    ),
                                    RestoreImpact::Fatal,
                                )
                                .with_profile(profile_id.clone())
                                .with_file(manifest.logical_file.file_id.clone())
                                .with_chunk(chunk.chunk_id.clone(), chunk.index),
                            );
                        }
                        Err(other_err) => {
                            // Transient/connection failure distinguished from permanent missing
                            findings.push(
                                VerificationFinding::new(
                                    VerificationLevel::RemoteAvailability,
                                    VerificationSeverity::Warning,
                                    VerificationIssueCode::RemoteUnavailable,
                                    format!(
                                        "Remote storage query failed for chunk '{}' (transient): {other_err}",
                                        chunk.chunk_id
                                    ),
                                    RestoreImpact::Degraded,
                                )
                                .with_profile(profile_id.clone())
                                .with_file(manifest.logical_file.file_id.clone())
                                .with_chunk(chunk.chunk_id.clone(), chunk.index),
                            );
                        }
                    }
                }
            }
        }

        if options.level == VerificationLevel::RemoteAvailability || cancellation.is_cancelled() {
            if cancellation.is_cancelled() {
                findings.push(self.create_cancelled_finding(profile_id, options.level));
            }
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                findings,
                1,
                1,
                chunk_count as u32,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        // =====================================================================
        // LEVEL 3 — REMOTE INTEGRITY VERIFICATION (STREAMING)
        // =====================================================================
        if options.level >= VerificationLevel::RemoteIntegrity || options.full_hash_check {
            for chunk in &manifest.chunks {
                if cancellation.is_cancelled() {
                    findings.push(self.create_cancelled_finding(profile_id, options.level));
                    break;
                }

                if matches!(chunk.storage_reference, StorageReference::Pending) {
                    continue;
                }

                let mut null_writer = NullHashWriter::new();
                let mut cancelling_writer = CancellingWriter::new(&mut null_writer, cancellation);

                let req = DownloadRequest {
                    file_id: manifest.logical_file.file_id.clone(),
                    chunk_id: chunk.chunk_id.clone(),
                    storage_reference: chunk.storage_reference.clone(),
                    expected_size_bytes: chunk.stored_size,
                    expected_sha256: None,
                };

                // Stream remote data directly to NullHashWriter using bounded 64 KiB buffer
                match self.storage_provider.download(&req, &mut cancelling_writer) {
                    Ok(_dl_res) => {
                        let (calculated_hash, bytes_streamed) = null_writer.finalize();

                        if chunk.stored_size > 0 && bytes_streamed != chunk.stored_size {
                            findings.push(
                                VerificationFinding::new(
                                    VerificationLevel::RemoteIntegrity,
                                    VerificationSeverity::Critical,
                                    VerificationIssueCode::ChunkSizeMismatch,
                                    format!(
                                        "Chunk '{}' streamed {} bytes, expected {} stored bytes",
                                        chunk.chunk_id, bytes_streamed, chunk.stored_size
                                    ),
                                    RestoreImpact::Fatal,
                                )
                                .with_profile(profile_id.clone())
                                .with_file(manifest.logical_file.file_id.clone())
                                .with_chunk(chunk.chunk_id.clone(), chunk.index),
                            );
                        }

                        // Validate payload hash against chunk integrity digest
                        if !calculated_hash.eq_ignore_ascii_case(&chunk.integrity.digest) {
                            findings.push(
                                VerificationFinding::new(
                                    VerificationLevel::RemoteIntegrity,
                                    VerificationSeverity::Critical,
                                    VerificationIssueCode::ChunkHashMismatch,
                                    format!(
                                        "Payload hash mismatch for chunk '{}': calculated '{calculated_hash}', expected '{}'",
                                        chunk.chunk_id, chunk.integrity.digest
                                    ),
                                    RestoreImpact::Fatal,
                                )
                                .with_profile(profile_id.clone())
                                .with_file(manifest.logical_file.file_id.clone())
                                .with_chunk(chunk.chunk_id.clone(), chunk.index),
                            );
                        }
                    }
                    Err(StorageError::NotFound(msg)) => {
                        findings.push(
                            VerificationFinding::new(
                                VerificationLevel::RemoteIntegrity,
                                VerificationSeverity::Critical,
                                VerificationIssueCode::ConfirmedMissing,
                                format!(
                                    "Chunk '{}' not found during stream verification: {msg}",
                                    chunk.chunk_id
                                ),
                                RestoreImpact::Fatal,
                            )
                            .with_profile(profile_id.clone())
                            .with_file(manifest.logical_file.file_id.clone())
                            .with_chunk(chunk.chunk_id.clone(), chunk.index),
                        );
                    }
                    Err(StorageError::Io(e))
                        if e.kind() == std::io::ErrorKind::Interrupted
                            || cancellation.is_cancelled() =>
                    {
                        findings.push(self.create_cancelled_finding(profile_id, options.level));
                        break;
                    }
                    Err(StorageError::VerificationFailed { reason }) => {
                        findings.push(
                            VerificationFinding::new(
                                VerificationLevel::RemoteIntegrity,
                                VerificationSeverity::Critical,
                                VerificationIssueCode::PayloadCorrupted,
                                format!(
                                    "Remote verification failed for chunk '{}': {reason}",
                                    chunk.chunk_id
                                ),
                                RestoreImpact::Fatal,
                            )
                            .with_profile(profile_id.clone())
                            .with_file(manifest.logical_file.file_id.clone())
                            .with_chunk(chunk.chunk_id.clone(), chunk.index),
                        );
                    }
                    Err(err) => {
                        findings.push(
                            VerificationFinding::new(
                                VerificationLevel::RemoteIntegrity,
                                VerificationSeverity::Warning,
                                VerificationIssueCode::RemoteUnavailable,
                                format!(
                                    "Remote download failed for chunk '{}': {err}",
                                    chunk.chunk_id
                                ),
                                RestoreImpact::Degraded,
                            )
                            .with_profile(profile_id.clone())
                            .with_file(manifest.logical_file.file_id.clone())
                            .with_chunk(chunk.chunk_id.clone(), chunk.index),
                        );
                    }
                }
            }
        }

        // =====================================================================
        // LEVEL 4 — RESTORE-READINESS SYNTHESIS
        // =====================================================================
        let result = self.synthesize_result(
            target_type,
            target_id,
            profile_id.clone(),
            options.level,
            findings,
            1,
            1,
            chunk_count as u32,
            start.elapsed().as_millis() as u64,
        );

        // Persist verification history in SQLite database
        self.persist_history(&result);

        Ok(result)
    }

    /// Verifies a tracked file in a profile by retrieving its latest manifest and auditing it.
    pub fn verify_file(
        &self,
        profile_id: &ProfileId,
        file_id: &FileId,
        options: &VerificationOptions,
        cancellation: &CancellationToken,
    ) -> Result<VerificationResult> {
        let start = Instant::now();
        let target_type = "file".to_string();
        let target_id = file_id.to_string();

        let ownership_validator = OwnershipValidator::new(&self.db, profile_id);

        if let Err(f) = ownership_validator.validate_profile() {
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                vec![f],
                1,
                0,
                0,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        // Validate file ownership
        if let Err(f) = ownership_validator.validate_file(file_id) {
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                vec![f],
                1,
                0,
                0,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        // Query versions of file to find latest manifest
        let versions = self
            .db
            .list_versions_by_file(file_id)
            .map_err(BackupError::from)?;

        let latest_ver = match versions.last() {
            Some(v) => v,
            None => {
                let f = VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Error,
                    VerificationIssueCode::ManifestNotFound,
                    format!("File '{file_id}' has no backed up versions in catalog"),
                    RestoreImpact::Fatal,
                )
                .with_profile(profile_id.clone())
                .with_file(file_id.clone());

                let res = self.synthesize_result(
                    target_type,
                    target_id,
                    profile_id.clone(),
                    options.level,
                    vec![f],
                    1,
                    0,
                    0,
                    start.elapsed().as_millis() as u64,
                );
                self.persist_history(&res);
                return Ok(res);
            }
        };

        // Query manifest from DB
        let manifest = match self
            .db
            .get_manifest(&latest_ver.manifest_id)
            .map_err(BackupError::from)?
        {
            Some(m) => m,
            None => {
                let f = VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::ManifestNotFound,
                    format!("Manifest '{}' not found in catalog", latest_ver.manifest_id),
                    RestoreImpact::Fatal,
                )
                .with_profile(profile_id.clone())
                .with_file(file_id.clone());

                let res = self.synthesize_result(
                    target_type,
                    target_id,
                    profile_id.clone(),
                    options.level,
                    vec![f],
                    1,
                    0,
                    0,
                    start.elapsed().as_millis() as u64,
                );
                self.persist_history(&res);
                return Ok(res);
            }
        };

        let mut manifest_result =
            self.verify_manifest(profile_id, &manifest, options, cancellation)?;
        manifest_result.target_type = "file".to_string();
        manifest_result.target_id = file_id.to_string();
        self.persist_history(&manifest_result);

        Ok(manifest_result)
    }

    /// Verifies all file manifests comprising a specific snapshot.
    pub fn verify_snapshot(
        &self,
        profile_id: &ProfileId,
        snapshot_id: &SnapshotId,
        options: &VerificationOptions,
        cancellation: &CancellationToken,
    ) -> Result<VerificationResult> {
        let start = Instant::now();
        let target_type = "snapshot".to_string();
        let target_id = snapshot_id.to_string();

        let ownership_validator = OwnershipValidator::new(&self.db, profile_id);

        if let Err(f) = ownership_validator.validate_profile() {
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                vec![f],
                0,
                0,
                0,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        // Validate snapshot ownership
        if let Err(f) = ownership_validator.validate_snapshot(snapshot_id) {
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                vec![f],
                0,
                0,
                0,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        let versions = self
            .db
            .list_versions_by_snapshot(snapshot_id)
            .map_err(BackupError::from)?;

        let mut findings = Vec::new();
        let mut total_chunks = 0;
        let mut total_manifests = 0;

        for ver in &versions {
            if cancellation.is_cancelled() {
                findings.push(self.create_cancelled_finding(profile_id, options.level));
                break;
            }

            match self
                .db
                .get_manifest(&ver.manifest_id)
                .map_err(BackupError::from)?
            {
                Some(manifest) => {
                    total_manifests += 1;
                    total_chunks += manifest.chunks.len() as u32;
                    let m_res =
                        self.verify_manifest(profile_id, &manifest, options, cancellation)?;
                    findings.extend(m_res.findings);
                }
                None => {
                    findings.push(
                        VerificationFinding::new(
                            VerificationLevel::MetadataOnly,
                            VerificationSeverity::Critical,
                            VerificationIssueCode::ManifestNotFound,
                            format!(
                                "Manifest '{}' referenced by version '{}' not found in catalog",
                                ver.manifest_id, ver.version_id
                            ),
                            RestoreImpact::Fatal,
                        )
                        .with_profile(profile_id.clone())
                        .with_snapshot(snapshot_id.clone())
                        .with_file(ver.file_id.clone()),
                    );
                }
            }
        }

        let res = self.synthesize_result(
            target_type,
            target_id,
            profile_id.clone(),
            options.level,
            findings,
            versions.len() as u32,
            total_manifests,
            total_chunks,
            start.elapsed().as_millis() as u64,
        );
        self.persist_history(&res);
        Ok(res)
    }

    /// Verifies all snapshots belonging to a profile.
    pub fn verify_profile(
        &self,
        profile_id: &ProfileId,
        options: &VerificationOptions,
        cancellation: &CancellationToken,
    ) -> Result<VerificationResult> {
        let start = Instant::now();
        let target_type = "profile".to_string();
        let target_id = profile_id.to_string();

        let ownership_validator = OwnershipValidator::new(&self.db, profile_id);

        if let Err(f) = ownership_validator.validate_profile() {
            let res = self.synthesize_result(
                target_type,
                target_id,
                profile_id.clone(),
                options.level,
                vec![f],
                0,
                0,
                0,
                start.elapsed().as_millis() as u64,
            );
            self.persist_history(&res);
            return Ok(res);
        }

        let snapshots = self
            .db
            .list_snapshots_by_profile(profile_id)
            .map_err(BackupError::from)?;

        let mut findings = Vec::new();
        let mut total_files = 0;
        let mut total_manifests = 0;
        let mut total_chunks = 0;

        for snap in &snapshots {
            if cancellation.is_cancelled() {
                findings.push(self.create_cancelled_finding(profile_id, options.level));
                break;
            }

            let snap_res =
                self.verify_snapshot(profile_id, &snap.snapshot_id, options, cancellation)?;
            total_files += snap_res.summary.total_files;
            total_manifests += snap_res.summary.total_manifests;
            total_chunks += snap_res.summary.total_chunks;
            findings.extend(snap_res.findings);
        }

        let res = self.synthesize_result(
            target_type,
            target_id,
            profile_id.clone(),
            options.level,
            findings,
            total_files,
            total_manifests,
            total_chunks,
            start.elapsed().as_millis() as u64,
        );
        self.persist_history(&res);
        Ok(res)
    }

    /// Synthesizes itemized findings into final aggregate metrics, status, and restore readiness.
    #[allow(clippy::too_many_arguments)]
    fn synthesize_result(
        &self,
        target_type: String,
        target_id: String,
        profile_id: ProfileId,
        level: VerificationLevel,
        findings: Vec<VerificationFinding>,
        total_files: u32,
        total_manifests: u32,
        total_chunks: u32,
        duration_ms: u64,
    ) -> VerificationResult {
        let mut corrupted_chunks = 0;
        let mut missing_chunks = 0;
        let mut ownership_violations = 0;
        let mut has_cancelled = false;
        let mut has_fatal = false;
        let mut has_error = false;
        let mut has_warning = false;

        for f in &findings {
            match f.restore_impact {
                RestoreImpact::Fatal => has_fatal = true,
                RestoreImpact::Degraded => has_warning = true,
                RestoreImpact::None => {}
            }

            match f.severity {
                VerificationSeverity::Critical => has_fatal = true,
                VerificationSeverity::Error => has_error = true,
                VerificationSeverity::Warning => has_warning = true,
                VerificationSeverity::Info => {}
            }

            match f.code {
                VerificationIssueCode::ChunkHashMismatch
                | VerificationIssueCode::StoredHashMismatch
                | VerificationIssueCode::PayloadCorrupted
                | VerificationIssueCode::AuthenticationFailed
                | VerificationIssueCode::DecryptionFailed
                | VerificationIssueCode::WholeFileHashMismatch => corrupted_chunks += 1,

                VerificationIssueCode::ConfirmedMissing => missing_chunks += 1,

                VerificationIssueCode::ProfileMismatch
                | VerificationIssueCode::SnapshotOwnershipViolation
                | VerificationIssueCode::FileOwnershipViolation
                | VerificationIssueCode::ManifestOwnershipViolation
                | VerificationIssueCode::ChunkOwnershipViolation
                | VerificationIssueCode::RemoteReferenceOwnershipViolation
                | VerificationIssueCode::CrossProfileReference => ownership_violations += 1,

                VerificationIssueCode::Cancelled => has_cancelled = true,
                _ => {}
            }
        }

        let is_restore_ready = !has_fatal
            && !has_cancelled
            && corrupted_chunks == 0
            && missing_chunks == 0
            && ownership_violations == 0;

        let status = if has_cancelled {
            VerificationStatus::Failed
        } else if corrupted_chunks > 0 || missing_chunks > 0 {
            VerificationStatus::Corrupted
        } else if has_fatal || has_error {
            VerificationStatus::Failed
        } else if has_warning {
            VerificationStatus::Warning
        } else {
            VerificationStatus::Healthy
        };

        let healthy_chunks = total_chunks
            .saturating_sub(corrupted_chunks)
            .saturating_sub(missing_chunks);

        let summary = VerificationSummary {
            total_files,
            total_manifests,
            total_chunks,
            healthy_chunks,
            corrupted_chunks,
            missing_chunks,
            ownership_violations,
            is_restore_ready,
            status,
            duration_ms,
        };

        VerificationResult {
            target_type,
            target_id,
            profile_id,
            level,
            status,
            is_restore_ready,
            findings,
            summary,
            verified_at: Utc::now().to_rfc3339(),
        }
    }

    /// Persists a completed verification audit record in the database.
    fn persist_history(&self, result: &VerificationResult) {
        let findings_json = serde_json::to_string(&result.findings).unwrap_or_else(|_| "[]".into());
        let record = VerificationHistoryRecord {
            history_id: format!("vh-{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)),
            profile_id: result.profile_id.clone(),
            target_type: result.target_type.clone(),
            target_id: result.target_id.clone(),
            level: result.level as u32,
            status: result.status.to_string(),
            is_restore_ready: result.is_restore_ready,
            total_files: result.summary.total_files,
            total_manifests: result.summary.total_manifests,
            total_chunks: result.summary.total_chunks,
            healthy_chunks: result.summary.healthy_chunks,
            corrupted_chunks: result.summary.corrupted_chunks,
            missing_chunks: result.summary.missing_chunks,
            ownership_violations: result.summary.ownership_violations,
            duration_ms: result.summary.duration_ms,
            findings_json,
            verified_at: result.verified_at.clone(),
        };

        let _ = self.db.record_verification_history(&record);
    }

    fn create_cancelled_finding(
        &self,
        profile_id: &ProfileId,
        level: VerificationLevel,
    ) -> VerificationFinding {
        VerificationFinding::new(
            level,
            VerificationSeverity::Warning,
            VerificationIssueCode::Cancelled,
            "Verification was cancelled cooperatively by user request",
            RestoreImpact::Fatal,
        )
        .with_profile(profile_id.clone())
    }
}
