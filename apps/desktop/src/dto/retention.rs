//! Data transfer objects for retention policy and snapshot pruning IPC commands.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Parameters for configuring or updating a profile's retention policy.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetRetentionPolicyRequest {
    /// Target backup profile identifier.
    pub profile_id: String,
    /// Maximum count of recent snapshots to retain.
    pub keep_latest_n: Option<u32>,
    /// Retention window in seconds; snapshots newer than this age are kept.
    pub keep_newer_than_secs: Option<u32>,
    /// Whether to unconditionally preserve the latest successful snapshot.
    pub keep_latest_successful: Option<bool>,
    /// Whether to unconditionally preserve the latest snapshot regardless of age or status.
    pub keep_latest_always: Option<bool>,
    /// Whether failed or incomplete snapshots should be pruned.
    pub prune_failed: Option<bool>,
    /// Whether completed snapshots with zero files should be pruned.
    pub prune_empty: Option<bool>,
    /// Whether the retention policy is actively enabled.
    pub enabled: Option<bool>,
}

/// User-facing representation of a backup profile's retention policy.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RetentionPolicyDto {
    /// Associated backup profile identifier.
    pub profile_id: String,
    /// Maximum count of recent snapshots to retain.
    pub keep_latest_n: Option<u32>,
    /// Retention window in seconds.
    pub keep_newer_than_secs: Option<u32>,
    /// Whether the latest successful snapshot is unconditionally preserved.
    pub keep_latest_successful: bool,
    /// Whether the latest snapshot is unconditionally preserved.
    pub keep_latest_always: bool,
    /// Whether failed or incomplete snapshots are pruned.
    pub prune_failed: bool,
    /// Whether completed snapshots with zero files are pruned.
    pub prune_empty: bool,
    /// Whether this retention policy is actively enabled.
    pub enabled: bool,
}

/// Retention decision for an individual snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RetentionDecisionDto {
    /// Target snapshot identifier.
    pub snapshot_id: String,
    /// Action: 'KEEP' or 'PRUNE'.
    pub action: String,
    /// Explicit machine-readable reason (e.g. 'KEEP_LATEST', 'PRUNE_EXCESS_SNAPSHOT').
    pub reason: String,
    /// Human-readable explanation.
    pub description: String,
    /// Snapshot execution status.
    pub snapshot_status: String,
    /// Snapshot creation timestamp (ISO-8601).
    pub snapshot_created_at: String,
    /// Calculated snapshot age in seconds.
    pub age_secs: u32,
    /// Associated file versions count.
    pub version_count: u32,
}

/// Deterministic evaluation report for a profile.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RetentionEvaluationDto {
    /// Target backup profile identifier.
    pub profile_id: String,
    /// Timestamp when evaluation was computed.
    pub evaluated_at: String,
    /// Effective retention policy applied.
    pub policy: RetentionPolicyDto,
    /// Itemized decisions for each snapshot evaluated.
    pub decisions: Vec<RetentionDecisionDto>,
    /// Total snapshots evaluated.
    pub snapshots_evaluated: u32,
    /// Total snapshots determined to KEEP.
    pub snapshots_kept: u32,
    /// Total snapshots determined to PRUNE.
    pub snapshots_pruned: u32,
}

/// Comprehensive outcome of a retention operation (dry-run or real execution).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RetentionResultDto {
    /// Target backup profile identifier.
    pub profile_id: String,
    /// Timestamp of execution.
    pub executed_at: String,
    /// Whether this was a dry-run evaluation with zero changes applied.
    pub dry_run: bool,
    /// Detailed retention evaluation report.
    pub evaluation: RetentionEvaluationDto,
    /// List of snapshot IDs that were pruned.
    pub pruned_snapshots: Vec<String>,
    /// Total file version records pruned.
    pub pruned_versions_count: u32,
    /// Whether the operation succeeded.
    pub success: bool,
    /// Error message if the operation failed.
    pub error_message: Option<String>,
}

/// Execution history audit entry for a retention pruning run.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RetentionHistoryDto {
    /// Unique history identifier.
    pub history_id: String,
    /// Associated backup profile identifier.
    pub profile_id: String,
    /// Timestamp of execution.
    pub executed_at: String,
    /// Whether this was a dry-run execution.
    pub dry_run: bool,
    /// Total snapshots evaluated.
    pub snapshots_evaluated: u32,
    /// Total snapshots kept.
    pub snapshots_kept: u32,
    /// Total snapshots pruned.
    pub snapshots_pruned: u32,
    /// List of snapshot IDs pruned during this run.
    pub pruned_snapshot_ids: Vec<String>,
    /// Execution status ("completed" or "failed").
    pub status: String,
    /// Error message if execution failed.
    pub error_message: Option<String>,
}
