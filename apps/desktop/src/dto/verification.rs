//! Data transfer objects for remote backup verification and integrity audit IPC commands.

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;
use televault_db::VerificationHistoryRecord;
use televault_integrity::types::{VerificationFinding, VerificationResult, VerificationSummary};

/// Request parameters for scoped remote backup verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct VerifyTargetRequest {
    /// Associated profile identifier enforcing ownership isolation.
    pub profile_id: String,
    /// Target identifier (file_id, manifest_id, or snapshot_id depending on command).
    pub target_id: Option<String>,
    /// Verification scope level: 1 (MetadataOnly), 2 (RemoteAvailability), 3 (RemoteIntegrity), 4 (RestoreReadiness).
    pub level: Option<u32>,
    /// Whether to force streaming byte integrity hash calculation.
    pub full_hash_check: Option<bool>,
    /// Whether to attempt decryption and authentication verification.
    pub decrypt_check: Option<bool>,
    /// Optional operation tracking identifier for cooperative cancellation.
    pub operation_id: Option<String>,
}

/// Structured finding identifying a specific issue discovered during verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct VerificationFindingDto {
    pub level: String,
    pub severity: String,
    pub code: String,
    pub description: String,
    pub restore_impact: String,
    pub profile_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub file_id: Option<String>,
    pub chunk_id: Option<String>,
    pub chunk_index: Option<u32>,
}

impl From<VerificationFinding> for VerificationFindingDto {
    fn from(f: VerificationFinding) -> Self {
        Self {
            level: f.level.to_string(),
            severity: f.severity.to_string(),
            code: f.code.to_string(),
            description: f.description,
            restore_impact: f.restore_impact.to_string(),
            profile_id: f.profile_id.map(|p| p.to_string()),
            snapshot_id: f.snapshot_id.map(|s| s.to_string()),
            file_id: f.file_id.map(|fi| fi.to_string()),
            chunk_id: f.chunk_id.map(|c| c.to_string()),
            chunk_index: f.chunk_index,
        }
    }
}

/// Quantitative summary counts of verification results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct VerificationSummaryDto {
    pub total_files: u32,
    pub total_manifests: u32,
    pub total_chunks: u32,
    pub healthy_chunks: u32,
    pub missing_chunks: u32,
    pub corrupted_chunks: u32,
    pub ownership_violations: u32,
    pub is_restore_ready: bool,
    pub status: String,
    #[specta(type = Number)]
    pub duration_ms: u64,
}

impl From<VerificationSummary> for VerificationSummaryDto {
    fn from(s: VerificationSummary) -> Self {
        Self {
            total_files: s.total_files,
            total_manifests: s.total_manifests,
            total_chunks: s.total_chunks,
            healthy_chunks: s.healthy_chunks,
            missing_chunks: s.missing_chunks,
            corrupted_chunks: s.corrupted_chunks,
            ownership_violations: s.ownership_violations,
            is_restore_ready: s.is_restore_ready,
            status: s.status.to_string(),
            duration_ms: s.duration_ms,
        }
    }
}

/// Comprehensive verification audit result for a target object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct VerificationResultDto {
    pub target_type: String,
    pub target_id: String,
    pub profile_id: String,
    pub level: String,
    pub status: String,
    pub is_restore_ready: bool,
    pub findings: Vec<VerificationFindingDto>,
    pub summary: VerificationSummaryDto,
    pub verified_at: String,
}

impl From<VerificationResult> for VerificationResultDto {
    fn from(r: VerificationResult) -> Self {
        Self {
            target_type: r.target_type,
            target_id: r.target_id,
            profile_id: r.profile_id.to_string(),
            level: r.level.to_string(),
            status: r.status.to_string(),
            is_restore_ready: r.is_restore_ready,
            findings: r.findings.into_iter().map(Into::into).collect(),
            summary: r.summary.into(),
            verified_at: r.verified_at,
        }
    }
}

/// Persisted audit history entry from SQLite verification_history table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct VerificationHistoryRecordDto {
    pub history_id: String,
    pub profile_id: String,
    pub target_type: String,
    pub target_id: String,
    pub level: u32,
    pub status: String,
    pub is_restore_ready: bool,
    pub total_files: u32,
    pub total_manifests: u32,
    pub total_chunks: u32,
    pub healthy_chunks: u32,
    pub missing_chunks: u32,
    pub corrupted_chunks: u32,
    pub ownership_violations: u32,
    pub findings_json: String,
    #[specta(type = Number)]
    pub duration_ms: u64,
    pub verified_at: String,
}

impl From<VerificationHistoryRecord> for VerificationHistoryRecordDto {
    fn from(r: VerificationHistoryRecord) -> Self {
        Self {
            history_id: r.history_id,
            profile_id: r.profile_id.to_string(),
            target_type: r.target_type,
            target_id: r.target_id,
            level: r.level,
            status: r.status,
            is_restore_ready: r.is_restore_ready,
            total_files: r.total_files,
            total_manifests: r.total_manifests,
            total_chunks: r.total_chunks,
            healthy_chunks: r.healthy_chunks,
            missing_chunks: r.missing_chunks,
            corrupted_chunks: r.corrupted_chunks,
            ownership_violations: r.ownership_violations,
            findings_json: r.findings_json,
            duration_ms: r.duration_ms,
            verified_at: r.verified_at,
        }
    }
}
