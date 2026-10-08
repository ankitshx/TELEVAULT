//! Domain types for remote backup repair, chunk recovery, and preview analysis.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use televault_core::ids::{ChunkId, FileId, ProfileId, SnapshotId};
use televault_integrity::types::{VerificationFinding, VerificationIssueCode};

/// An individual chunk candidate identified for repair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairCandidate {
    /// Associated profile identifier.
    pub profile_id: ProfileId,
    /// Associated snapshot identifier if part of snapshot repair.
    pub snapshot_id: Option<SnapshotId>,
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated manifest identifier.
    pub manifest_id: String,
    /// Physical chunk identifier needing repair.
    pub chunk_id: ChunkId,
    /// Zero-based sequential chunk index.
    pub chunk_index: u32,
    /// Total count of chunks in the logical file.
    pub total_chunks: u32,
    /// Verification issue code that triggered repair eligibility.
    pub finding_code: VerificationIssueCode,
    /// Existing (damaged or missing) storage reference string.
    pub old_storage_reference: String,
    /// Expected plaintext byte size of this chunk.
    pub plaintext_size: u64,
    /// Expected stored byte size before failure.
    pub stored_size: u64,
    /// Resolved absolute path to legitimate local source file.
    pub source_path: PathBuf,
}

/// Dry-run preview analysis of repair operations before any execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairPreview {
    /// Scoped profile identifier.
    pub profile_id: ProfileId,
    /// Target type ("file", "snapshot", "manifest").
    pub target_type: String,
    /// Target identifier string.
    pub target_id: String,
    /// List of eligible chunks ready for reconstruction and replacement.
    pub eligible_candidates: Vec<RepairCandidate>,
    /// Findings that were rejected/ineligible for automatic repair.
    pub ineligible_findings: Vec<VerificationFinding>,
    /// Total number of chunks that will be repaired.
    pub total_affected_chunks: u32,
    /// Estimated payload bytes to be reconstructed and uploaded.
    pub total_estimated_bytes: u64,
    /// Whether at least one eligible repair candidate was found and no fatal barriers exist.
    pub is_repairable: bool,
}

/// Execution outcome of repairing an individual physical chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairChunkResult {
    /// Physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Zero-based sequential chunk index.
    pub chunk_index: u32,
    /// Status ("success", "skipped_healthy", "failed", "cancelled").
    pub status: String,
    /// Previous storage reference string.
    pub old_storage_reference: String,
    /// New replacement storage reference string if upload succeeded.
    pub new_storage_reference: Option<String>,
    /// Plaintext/stored bytes processed.
    pub bytes_processed: u64,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Error message if repair failed.
    pub error: Option<String>,
}

/// Aggregate result of a file or snapshot repair operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairExecutionResult {
    /// Scoped profile identifier.
    pub profile_id: ProfileId,
    /// Target type ("file", "snapshot", "manifest").
    pub target_type: String,
    /// Target identifier string.
    pub target_id: String,
    /// Total chunks evaluated.
    pub total_chunks_evaluated: u32,
    /// Chunks successfully repaired and verified.
    pub repaired_chunks: u32,
    /// Chunks already healthy (skipped to avoid unnecessary duplicate Telegram uploads).
    pub skipped_healthy_chunks: u32,
    /// Chunks that failed repair.
    pub failed_chunks: u32,
    /// Total payload bytes uploaded and verified.
    pub total_bytes_transferred: u64,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Itemized results per chunk.
    pub chunk_results: Vec<RepairChunkResult>,
}
