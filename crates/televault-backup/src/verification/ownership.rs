//! Ownership and profile isolation validation for TELEVAULT backup chains.
//!
//! Enforces the invariant:
//! Profile -> Snapshot -> File -> Manifest -> Chunk -> Remote Storage Reference
//! must form an unbroken, exclusive ownership chain. Cross-profile object references
//! are strictly detected and reported as fatal security findings.

use televault_core::ids::{FileId, ProfileId, SnapshotId};
use televault_db::{Database, FileRecord, ProfileRecord, SnapshotRecord};
use televault_integrity::types::{
    RestoreImpact, VerificationFinding, VerificationIssueCode, VerificationLevel,
    VerificationSeverity,
};
use televault_manifest::{ManifestV1, StorageReference};

/// Validator responsible for enforcing ownership chain invariants across all backup entities.
pub struct OwnershipValidator<'a> {
    db: &'a Database,
    expected_profile_id: &'a ProfileId,
}

#[allow(clippy::result_large_err)]
impl<'a> OwnershipValidator<'a> {
    /// Creates a new ownership validator scoped to the specified profile.
    pub fn new(db: &'a Database, expected_profile_id: &'a ProfileId) -> Self {
        Self {
            db,
            expected_profile_id,
        }
    }

    /// Validates the profile entity itself exists in the database.
    pub fn validate_profile(&self) -> Result<ProfileRecord, VerificationFinding> {
        match self.db.get_profile(self.expected_profile_id) {
            Ok(Some(profile)) => Ok(profile),
            Ok(None) => Err(VerificationFinding::new(
                VerificationLevel::MetadataOnly,
                VerificationSeverity::Critical,
                VerificationIssueCode::ProfileNotFound,
                format!(
                    "Scoped profile '{}' does not exist in local catalog",
                    self.expected_profile_id
                ),
                RestoreImpact::Fatal,
            )
            .with_profile(self.expected_profile_id.clone())),
            Err(e) => Err(VerificationFinding::new(
                VerificationLevel::MetadataOnly,
                VerificationSeverity::Critical,
                VerificationIssueCode::InternalError,
                format!("Database error querying profile: {e}"),
                RestoreImpact::Fatal,
            )
            .with_profile(self.expected_profile_id.clone())),
        }
    }

    /// Validates snapshot ownership: snapshot must exist and belong directly to `expected_profile_id`.
    pub fn validate_snapshot(
        &self,
        snapshot_id: &SnapshotId,
    ) -> Result<SnapshotRecord, VerificationFinding> {
        let snap = match self.db.get_snapshot(snapshot_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                return Err(VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::SnapshotNotFound,
                    format!("Snapshot '{snapshot_id}' not found in catalog"),
                    RestoreImpact::Fatal,
                )
                .with_profile(self.expected_profile_id.clone())
                .with_snapshot(snapshot_id.clone()));
            }
            Err(e) => {
                return Err(VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::InternalError,
                    format!("Database error querying snapshot: {e}"),
                    RestoreImpact::Fatal,
                )
                .with_profile(self.expected_profile_id.clone())
                .with_snapshot(snapshot_id.clone()));
            }
        };

        if snap.profile_id != *self.expected_profile_id {
            return Err(VerificationFinding::new(
                VerificationLevel::MetadataOnly,
                VerificationSeverity::Critical,
                VerificationIssueCode::SnapshotOwnershipViolation,
                format!(
                    "Snapshot '{snapshot_id}' belongs to profile '{}', not expected profile '{}'",
                    snap.profile_id, self.expected_profile_id
                ),
                RestoreImpact::Fatal,
            )
            .with_profile(self.expected_profile_id.clone())
            .with_snapshot(snapshot_id.clone()));
        }

        Ok(snap)
    }

    /// Validates logical file ownership: file must exist and belong to `expected_profile_id`.
    pub fn validate_file(&self, file_id: &FileId) -> Result<FileRecord, VerificationFinding> {
        let file = match self.db.get_file(file_id) {
            Ok(Some(f)) => f,
            Ok(None) => {
                return Err(VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::FileNotFound,
                    format!("Logical file '{file_id}' not found in catalog"),
                    RestoreImpact::Fatal,
                )
                .with_profile(self.expected_profile_id.clone())
                .with_file(file_id.clone()));
            }
            Err(e) => {
                return Err(VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::InternalError,
                    format!("Database error querying file: {e}"),
                    RestoreImpact::Fatal,
                )
                .with_profile(self.expected_profile_id.clone())
                .with_file(file_id.clone()));
            }
        };

        if let Some(ref pid) = file.profile_id {
            if pid != self.expected_profile_id {
                return Err(VerificationFinding::new(
                    VerificationLevel::MetadataOnly,
                    VerificationSeverity::Critical,
                    VerificationIssueCode::FileOwnershipViolation,
                    format!(
                        "File '{file_id}' belongs to profile '{pid}', not expected profile '{}'",
                        self.expected_profile_id
                    ),
                    RestoreImpact::Fatal,
                )
                .with_profile(self.expected_profile_id.clone())
                .with_file(file_id.clone()));
            }
        }

        Ok(file)
    }

    /// Validates manifest, chunk, and storage reference ownership against expected profile and file.
    pub fn validate_manifest_ownership_chain(
        &self,
        manifest: &ManifestV1,
        expected_file_id: Option<&FileId>,
        expected_snapshot_id: Option<&SnapshotId>,
    ) -> Vec<VerificationFinding> {
        let mut findings = Vec::new();
        let file_id = &manifest.logical_file.file_id;

        // 1. If expected_file_id is specified, must match manifest's file_id
        if let Some(expected_fid) = expected_file_id {
            if expected_fid != file_id {
                findings.push(
                    VerificationFinding::new(
                        VerificationLevel::MetadataOnly,
                        VerificationSeverity::Critical,
                        VerificationIssueCode::ManifestOwnershipViolation,
                        format!(
                            "Manifest '{}' declares file_id '{file_id}', but expected file_id '{expected_fid}'",
                            manifest.manifest_id
                        ),
                        RestoreImpact::Fatal,
                    )
                    .with_profile(self.expected_profile_id.clone())
                    .with_file(file_id.clone()),
                );
            }
        }

        // 2. Validate file record ownership in DB
        match self.validate_file(file_id) {
            Ok(file_rec) => {
                if let Some(ref f_pid) = file_rec.profile_id {
                    if f_pid != self.expected_profile_id {
                        findings.push(
                            VerificationFinding::new(
                                VerificationLevel::MetadataOnly,
                                VerificationSeverity::Critical,
                                VerificationIssueCode::CrossProfileReference,
                                format!(
                                    "File '{file_id}' is owned by profile '{f_pid}', cross-profile access prohibited"
                                ),
                                RestoreImpact::Fatal,
                            )
                            .with_profile(self.expected_profile_id.clone())
                            .with_file(file_id.clone()),
                        );
                    }
                }
            }
            Err(f) => {
                findings.push(f);
            }
        }

        // 3. If expected_snapshot_id is provided, verify snapshot ownership
        if let Some(snap_id) = expected_snapshot_id {
            if let Err(f) = self.validate_snapshot(snap_id) {
                findings.push(f);
            }
        }

        // 4. Validate chunks and storage references
        for chunk in &manifest.chunks {
            // Check storage reference syntax and ownership
            match &chunk.storage_reference {
                StorageReference::Pending => {
                    findings.push(
                        VerificationFinding::new(
                            VerificationLevel::MetadataOnly,
                            VerificationSeverity::Error,
                            VerificationIssueCode::PendingStorageAllocation,
                            format!(
                                "Chunk '{}' (index {}) is in Pending storage state",
                                chunk.chunk_id, chunk.index
                            ),
                            RestoreImpact::Fatal,
                        )
                        .with_profile(self.expected_profile_id.clone())
                        .with_file(file_id.clone())
                        .with_chunk(chunk.chunk_id.clone(), chunk.index),
                    );
                }
                StorageReference::Telegram {
                    chat_id: _chat_id,
                    message_id,
                    file_id: tg_file_id,
                } => {
                    if *message_id <= 0 {
                        findings.push(
                            VerificationFinding::new(
                                VerificationLevel::RemoteAvailability,
                                VerificationSeverity::Critical,
                                VerificationIssueCode::WrongMessageId,
                                format!(
                                    "Chunk '{}' has invalid non-positive Telegram message ID {message_id}",
                                    chunk.chunk_id
                                ),
                                RestoreImpact::Fatal,
                            )
                            .with_profile(self.expected_profile_id.clone())
                            .with_file(file_id.clone())
                            .with_chunk(chunk.chunk_id.clone(), chunk.index),
                        );
                    }

                    if tg_file_id.trim().is_empty() {
                        findings.push(
                            VerificationFinding::new(
                                VerificationLevel::RemoteAvailability,
                                VerificationSeverity::Critical,
                                VerificationIssueCode::WrongFileId,
                                format!("Chunk '{}' has empty Telegram file ID", chunk.chunk_id),
                                RestoreImpact::Fatal,
                            )
                            .with_profile(self.expected_profile_id.clone())
                            .with_file(file_id.clone())
                            .with_chunk(chunk.chunk_id.clone(), chunk.index),
                        );
                    }

                    // Check if reference is associated with a different chunk in SQLite
                    let ref_str =
                        serde_json::to_string(&chunk.storage_reference).unwrap_or_default();
                    if let Ok(other_chunks) = self.db.find_chunks_by_storage_reference(&ref_str) {
                        for other in other_chunks {
                            if other.chunk_id != chunk.chunk_id {
                                findings.push(
                                    VerificationFinding::new(
                                        VerificationLevel::MetadataOnly,
                                        VerificationSeverity::Critical,
                                        VerificationIssueCode::RemoteReferenceOwnershipViolation,
                                        format!(
                                            "Remote reference '{ref_str}' is already assigned to chunk '{}' (file '{}'), illegal reuse detected",
                                            other.chunk_id, other.file_id
                                        ),
                                        RestoreImpact::Fatal,
                                    )
                                    .with_profile(self.expected_profile_id.clone())
                                    .with_file(file_id.clone())
                                    .with_chunk(chunk.chunk_id.clone(), chunk.index),
                                );
                            }
                        }
                    }
                }
                StorageReference::LocalStaging { relative_path } => {
                    if relative_path.contains("..") || relative_path.starts_with('/') {
                        findings.push(
                            VerificationFinding::new(
                                VerificationLevel::MetadataOnly,
                                VerificationSeverity::Critical,
                                VerificationIssueCode::InvalidRelativePath,
                                format!(
                                    "Chunk '{}' has unsafe local staging path '{relative_path}'",
                                    chunk.chunk_id
                                ),
                                RestoreImpact::Fatal,
                            )
                            .with_profile(self.expected_profile_id.clone())
                            .with_file(file_id.clone())
                            .with_chunk(chunk.chunk_id.clone(), chunk.index),
                        );
                    }
                }
            }

            // Check if chunk record in DB matches manifest
            if let Ok(Some(chunk_rec)) = self.db.get_chunk(&chunk.chunk_id) {
                if chunk_rec.file_id != *file_id {
                    findings.push(
                        VerificationFinding::new(
                            VerificationLevel::MetadataOnly,
                            VerificationSeverity::Critical,
                            VerificationIssueCode::ChunkOwnershipViolation,
                            format!(
                                "Chunk '{}' belongs to file '{}' in DB, but manifest declares file '{file_id}'",
                                chunk.chunk_id, chunk_rec.file_id
                            ),
                            RestoreImpact::Fatal,
                        )
                        .with_profile(self.expected_profile_id.clone())
                        .with_file(file_id.clone())
                        .with_chunk(chunk.chunk_id.clone(), chunk.index),
                    );
                }
            }
        }

        findings
    }
}
