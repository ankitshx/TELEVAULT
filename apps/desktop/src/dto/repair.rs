//! Data transfer objects for remote backup repair and recovery IPC commands.

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;
use televault_backup::repair::{
    RepairCandidate, RepairChunkResult, RepairExecutionResult, RepairPreview,
};
use televault_db::RepairHistoryRecord;

/// Request parameters for generating a repair preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PreviewRepairRequest {
    /// Associated profile identifier enforcing ownership isolation.
    pub profile_id: String,
    /// Target type ("file" or "snapshot").
    pub target_type: String,
    /// Target identifier (file_id or snapshot_id).
    pub target_id: String,
    /// Optional passphrase for encrypted backups.
    pub passphrase: Option<String>,
    /// Optional operation tracking identifier for cooperative cancellation.
    pub operation_id: Option<String>,
}

/// Request parameters for repairing an individual logical file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepairFileRequest {
    /// Associated profile identifier enforcing ownership isolation.
    pub profile_id: String,
    /// Logical file identifier to repair.
    pub file_id: String,
    /// Whether to perform dry-run evaluation without remote upload.
    pub dry_run: Option<bool>,
    /// Optional passphrase for encrypted backups.
    pub passphrase: Option<String>,
    /// Optional operation tracking identifier for cooperative cancellation.
    pub operation_id: Option<String>,
}

/// Request parameters for repairing an entire backup snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepairSnapshotRequest {
    /// Associated profile identifier enforcing ownership isolation.
    pub profile_id: String,
    /// Snapshot identifier to repair.
    pub snapshot_id: String,
    /// Whether to perform dry-run evaluation without remote upload.
    pub dry_run: Option<bool>,
    /// Optional passphrase for encrypted backups.
    pub passphrase: Option<String>,
    /// Optional operation tracking identifier for cooperative cancellation.
    pub operation_id: Option<String>,
}

/// Data transfer object representing an individual chunk candidate for repair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepairCandidateDto {
    pub chunk_id: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub finding_code: String,
    pub old_storage_reference: String,
    #[specta(type = Number)]
    pub plaintext_size: u64,
    #[specta(type = Number)]
    pub stored_size: u64,
    pub source_path: String,
}

impl From<RepairCandidate> for RepairCandidateDto {
    fn from(c: RepairCandidate) -> Self {
        Self {
            chunk_id: c.chunk_id.to_string(),
            chunk_index: c.chunk_index,
            total_chunks: c.total_chunks,
            finding_code: format!("{:?}", c.finding_code),
            old_storage_reference: c.old_storage_reference,
            plaintext_size: c.plaintext_size,
            stored_size: c.stored_size,
            source_path: c.source_path.to_string_lossy().to_string(),
        }
    }
}

/// Data transfer object representing preview analysis prior to repair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepairPreviewDto {
    pub profile_id: String,
    pub target_type: String,
    pub target_id: String,
    pub eligible_candidates: Vec<RepairCandidateDto>,
    pub ineligible_findings_count: u32,
    pub total_affected_chunks: u32,
    #[specta(type = Number)]
    pub total_estimated_bytes: u64,
    pub is_repairable: bool,
}

impl From<RepairPreview> for RepairPreviewDto {
    fn from(p: RepairPreview) -> Self {
        Self {
            profile_id: p.profile_id.to_string(),
            target_type: p.target_type,
            target_id: p.target_id,
            eligible_candidates: p.eligible_candidates.into_iter().map(Into::into).collect(),
            ineligible_findings_count: p.ineligible_findings.len() as u32,
            total_affected_chunks: p.total_affected_chunks,
            total_estimated_bytes: p.total_estimated_bytes,
            is_repairable: p.is_repairable,
        }
    }
}

/// Data transfer object representing the repair result of an individual chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepairChunkResultDto {
    pub chunk_id: String,
    pub chunk_index: u32,
    pub status: String,
    pub old_storage_reference: String,
    pub new_storage_reference: Option<String>,
    #[specta(type = Number)]
    pub bytes_processed: u64,
    #[specta(type = Number)]
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl From<RepairChunkResult> for RepairChunkResultDto {
    fn from(r: RepairChunkResult) -> Self {
        Self {
            chunk_id: r.chunk_id.to_string(),
            chunk_index: r.chunk_index,
            status: r.status,
            old_storage_reference: r.old_storage_reference,
            new_storage_reference: r.new_storage_reference,
            bytes_processed: r.bytes_processed,
            duration_ms: r.duration_ms,
            error: r.error,
        }
    }
}

/// Data transfer object representing the overall repair execution outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepairExecutionResultDto {
    pub profile_id: String,
    pub target_type: String,
    pub target_id: String,
    pub total_chunks_evaluated: u32,
    pub repaired_chunks: u32,
    pub skipped_healthy_chunks: u32,
    pub failed_chunks: u32,
    #[specta(type = Number)]
    pub total_bytes_transferred: u64,
    #[specta(type = Number)]
    pub duration_ms: u64,
    pub chunk_results: Vec<RepairChunkResultDto>,
}

impl From<RepairExecutionResult> for RepairExecutionResultDto {
    fn from(r: RepairExecutionResult) -> Self {
        Self {
            profile_id: r.profile_id.to_string(),
            target_type: r.target_type,
            target_id: r.target_id,
            total_chunks_evaluated: r.total_chunks_evaluated,
            repaired_chunks: r.repaired_chunks,
            skipped_healthy_chunks: r.skipped_healthy_chunks,
            failed_chunks: r.failed_chunks,
            total_bytes_transferred: r.total_bytes_transferred,
            duration_ms: r.duration_ms,
            chunk_results: r.chunk_results.into_iter().map(Into::into).collect(),
        }
    }
}

/// Data transfer object representing an auditable repair history record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepairHistoryRecordDto {
    pub repair_id: String,
    pub profile_id: String,
    pub snapshot_id: Option<String>,
    pub file_id: String,
    pub manifest_id: String,
    pub chunk_id: String,
    pub chunk_index: u32,
    pub repair_type: String,
    pub finding_code: String,
    pub old_storage_reference: String,
    pub new_storage_reference: String,
    pub status: String,
    #[specta(type = Number)]
    pub bytes_processed: u64,
    #[specta(type = Number)]
    pub duration_ms: u64,
    pub error_message: Option<String>,
    pub repaired_at: String,
}

impl From<RepairHistoryRecord> for RepairHistoryRecordDto {
    fn from(r: RepairHistoryRecord) -> Self {
        Self {
            repair_id: r.repair_id,
            profile_id: r.profile_id.to_string(),
            snapshot_id: r.snapshot_id.map(|s| s.to_string()),
            file_id: r.file_id.to_string(),
            manifest_id: r.manifest_id,
            chunk_id: r.chunk_id.to_string(),
            chunk_index: r.chunk_index,
            repair_type: r.repair_type,
            finding_code: r.finding_code,
            old_storage_reference: r.old_storage_reference,
            new_storage_reference: r.new_storage_reference,
            status: r.status,
            bytes_processed: r.bytes_processed,
            duration_ms: r.duration_ms,
            error_message: r.error_message,
            repaired_at: r.repaired_at,
        }
    }
}
