//! Eligibility rules, source validation, and strict ownership gating for repair operations.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use televault_core::ids::{FileId, ProfileId, SnapshotId};
use televault_core::validation::validate_safe_relative_path;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::{Database, ProfileRecord};
use televault_integrity::types::{VerificationFinding, VerificationIssueCode};
use televault_manifest::ManifestV1;

use super::types::RepairCandidate;
use crate::error::{RepairError, RepairOpResult};
use crate::verification::OwnershipValidator;

/// Evaluates whether an itemized verification issue code represents a recoverable remote failure.
pub fn is_issue_repairable(code: &VerificationIssueCode) -> bool {
    matches!(
        code,
        VerificationIssueCode::ConfirmedMissing
            | VerificationIssueCode::ChunkHashMismatch
            | VerificationIssueCode::StoredHashMismatch
            | VerificationIssueCode::PayloadCorrupted
            | VerificationIssueCode::WrongMessageId
            | VerificationIssueCode::WrongFileId
            | VerificationIssueCode::InvalidTelegramReference
            | VerificationIssueCode::RemoteMetadataMismatch
            | VerificationIssueCode::PendingStorageAllocation
    )
}

/// Evaluates whether an issue code represents a strict ownership or structure violation
/// that must NEVER be automatically repaired.
pub fn is_strict_barrier(code: &VerificationIssueCode) -> bool {
    matches!(
        code,
        VerificationIssueCode::ProfileMismatch
            | VerificationIssueCode::SnapshotOwnershipViolation
            | VerificationIssueCode::FileOwnershipViolation
            | VerificationIssueCode::ManifestOwnershipViolation
            | VerificationIssueCode::ChunkOwnershipViolation
            | VerificationIssueCode::RemoteReferenceOwnershipViolation
            | VerificationIssueCode::CrossProfileReference
            | VerificationIssueCode::ManifestInvalid
            | VerificationIssueCode::UnsupportedVersion
            | VerificationIssueCode::ChunkCountMismatch
            | VerificationIssueCode::ChunkIndexDiscontinuous
            | VerificationIssueCode::DuplicateChunkIndex
            | VerificationIssueCode::InvalidLogicalFileSize
            | VerificationIssueCode::InvalidRelativePath
            | VerificationIssueCode::ProfileNotFound
            | VerificationIssueCode::SnapshotNotFound
            | VerificationIssueCode::FileNotFound
            | VerificationIssueCode::ManifestNotFound
    )
}

/// Evaluator enforcing repair eligibility, local source validity, and cryptographic prerequisites.
pub struct RepairEligibilityChecker<'a> {
    db: &'a Database,
    profile_id: &'a ProfileId,
    encryption_policy: &'a EncryptionPolicy,
}

impl<'a> RepairEligibilityChecker<'a> {
    /// Creates a new eligibility checker.
    pub fn new(
        db: &'a Database,
        profile_id: &'a ProfileId,
        encryption_policy: &'a EncryptionPolicy,
    ) -> Self {
        Self {
            db,
            profile_id,
            encryption_policy,
        }
    }

    /// Strictly validates the backup profile existence and ownership.
    pub fn validate_profile(&self) -> RepairOpResult<ProfileRecord> {
        let profile = self
            .db
            .get_profile(self.profile_id)
            .map_err(RepairError::Database)?
            .ok_or_else(|| RepairError::Ineligible {
                reason: format!("Profile '{}' not found in local catalog", self.profile_id),
            })?;
        Ok(profile)
    }

    /// Validates the unbroken ownership chain for a file and manifest.
    pub fn validate_ownership_chain(
        &self,
        manifest: &ManifestV1,
        expected_file_id: &FileId,
        expected_snapshot_id: Option<&SnapshotId>,
    ) -> RepairOpResult<()> {
        let validator = OwnershipValidator::new(self.db, self.profile_id);

        if manifest.logical_file.file_id != *expected_file_id {
            return Err(RepairError::OwnershipViolation {
                reason: format!(
                    "Manifest file_id '{}' does not match target file_id '{}'",
                    manifest.logical_file.file_id, expected_file_id
                ),
            });
        }

        let file_rec = validator.validate_file(expected_file_id).map_err(|f| {
            RepairError::OwnershipViolation {
                reason: f.description,
            }
        })?;

        if let Some(ref pid) = file_rec.profile_id {
            if pid != self.profile_id {
                return Err(RepairError::OwnershipViolation {
                    reason: format!(
                        "File '{expected_file_id}' belongs to profile '{pid}', not expected profile '{}'",
                        self.profile_id
                    ),
                });
            }
        }

        if let Some(snap_id) = expected_snapshot_id {
            validator
                .validate_snapshot(snap_id)
                .map_err(|f| RepairError::OwnershipViolation {
                    reason: f.description,
                })?;
        }

        let findings = validator.validate_manifest_ownership_chain(
            manifest,
            Some(expected_file_id),
            expected_snapshot_id,
        );

        if let Some(barrier) = findings.into_iter().find(|f| is_strict_barrier(&f.code)) {
            return Err(RepairError::OwnershipViolation {
                reason: format!("{}: {}", barrier.code, barrier.description),
            });
        }

        Ok(())
    }

    /// Validates the local filesystem source file required to rebuild damaged chunks.
    pub fn validate_local_source(
        &self,
        profile: &ProfileRecord,
        manifest: &ManifestV1,
    ) -> RepairOpResult<PathBuf> {
        let rel_path_str = &manifest.logical_file.relative_path;
        validate_safe_relative_path(Path::new(rel_path_str)).map_err(|e| {
            RepairError::Ineligible {
                reason: format!("Unsafe relative path '{rel_path_str}': {e}"),
            }
        })?;

        let source_base = PathBuf::from(&profile.source_path);
        let full_path = source_base.join(rel_path_str);

        if !full_path.exists() {
            return Err(RepairError::SourceNotFound {
                path: full_path.to_string_lossy().to_string(),
            });
        }

        let metadata = fs::metadata(&full_path).map_err(|e| RepairError::SourceUnreadable {
            path: full_path.to_string_lossy().to_string(),
            reason: e.to_string(),
        })?;

        if !metadata.is_file() {
            return Err(RepairError::Ineligible {
                reason: format!(
                    "Source path '{}' is not a regular file",
                    full_path.display()
                ),
            });
        }

        if metadata.len() != manifest.logical_file.original_size {
            return Err(RepairError::SourceSizeMismatch {
                path: full_path.to_string_lossy().to_string(),
                expected: manifest.logical_file.original_size,
                actual: metadata.len(),
            });
        }

        // Verify readable
        File::open(&full_path).map_err(|e| RepairError::SourceUnreadable {
            path: full_path.to_string_lossy().to_string(),
            reason: e.to_string(),
        })?;

        Ok(full_path)
    }

    /// Validates that encryption keys are available if the backup manifest requires encryption.
    pub fn validate_crypto_prerequisites(&self, manifest: &ManifestV1) -> RepairOpResult<()> {
        if manifest.encryption.is_some() && !self.encryption_policy.is_enabled() {
            return Err(RepairError::EncryptionKeyMissing {
                reason:
                    "Manifest requires AES-256-GCM encryption but no encryption key is available"
                        .into(),
            });
        }
        Ok(())
    }

    /// Filters and resolves eligible repair candidates from a list of verification findings.
    pub fn resolve_candidates(
        &self,
        _profile: &ProfileRecord,
        manifest: &ManifestV1,
        findings: &[VerificationFinding],
        source_path: &Path,
        snapshot_id: Option<&SnapshotId>,
    ) -> RepairOpResult<(Vec<RepairCandidate>, Vec<VerificationFinding>)> {
        let mut candidates = Vec::new();
        let mut ineligible = Vec::new();

        let total_chunks = manifest.chunks.len() as u32;

        for finding in findings {
            if is_strict_barrier(&finding.code) {
                return Err(RepairError::OwnershipViolation {
                    reason: format!(
                        "Strict invariant violation detected ({:?}): {}",
                        finding.code, finding.description
                    ),
                });
            }

            if !is_issue_repairable(&finding.code) {
                ineligible.push(finding.clone());
                continue;
            }

            // Find matching chunk from manifest
            let chunk_opt = if let Some(ref cid) = finding.chunk_id {
                manifest.chunks.iter().find(|c| c.chunk_id == *cid)
            } else if let Some(idx) = finding.chunk_index {
                manifest.chunks.get(idx as usize)
            } else {
                None
            };

            if let Some(chunk) = chunk_opt {
                // Avoid adding duplicate candidates for the same chunk
                if candidates
                    .iter()
                    .any(|c: &RepairCandidate| c.chunk_id == chunk.chunk_id)
                {
                    continue;
                }

                let old_ref = serde_json::to_string(&chunk.storage_reference).unwrap_or_default();

                candidates.push(RepairCandidate {
                    profile_id: self.profile_id.clone(),
                    snapshot_id: snapshot_id.cloned(),
                    file_id: manifest.logical_file.file_id.clone(),
                    manifest_id: manifest.manifest_id.clone(),
                    chunk_id: chunk.chunk_id.clone(),
                    chunk_index: chunk.index,
                    total_chunks,
                    finding_code: finding.code,
                    old_storage_reference: old_ref,
                    plaintext_size: chunk.plaintext_size,
                    stored_size: chunk.stored_size,
                    source_path: source_path.to_path_buf(),
                });
            } else {
                ineligible.push(finding.clone());
            }
        }

        Ok((candidates, ineligible))
    }
}
