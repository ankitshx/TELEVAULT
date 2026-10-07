//! Strongly-typed domain models for retention evaluation and pruning decisions.

use serde::{Deserialize, Serialize};
use televault_core::ids::{ProfileId, SnapshotId};
use televault_core::models::BackupStatus;

/// Categorical retention decision action indicating whether to keep or prune a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetentionAction {
    /// The snapshot must be preserved locally.
    Keep,
    /// The snapshot metadata is eligible for local pruning.
    Prune,
}

impl std::fmt::Display for RetentionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Keep => write!(f, "KEEP"),
            Self::Prune => write!(f, "PRUNE"),
        }
    }
}

/// Explicit, auditable reason for a retention decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetentionReason {
    /// Preserved because it is the most recent snapshot for the profile.
    KeepLatest,
    /// Preserved because its age falls within the configured retention window.
    KeepWithinRetentionWindow,
    /// Preserved because it is the most recent successful (Completed) recovery point.
    KeepLatestSuccessful,
    /// Preserved because it falls within the configured latest N count limit.
    KeepCountLimit,
    /// Protected because the snapshot is actively being created, scanned, or restored.
    ProtectedActive,
    /// Protected because it is the sole snapshot for the profile.
    ProtectedSoleSnapshot,
    /// Pruned because its creation age exceeds the configured retention window.
    PruneExpired,
    /// Pruned because it exceeds the maximum count of snapshots to retain.
    PruneExcessSnapshot,
    /// Pruned because the snapshot is in a failed state and policy permits pruning.
    PruneFailedSnapshot,
    /// Pruned because the snapshot is cancelled or incomplete and policy permits pruning.
    PruneIncompleteSnapshot,
    /// Pruned because the snapshot is empty (0 associated file versions).
    PruneEmpty,
}

impl std::fmt::Display for RetentionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeepLatest => write!(f, "KEEP_LATEST"),
            Self::KeepWithinRetentionWindow => write!(f, "KEEP_WITHIN_RETENTION_WINDOW"),
            Self::KeepLatestSuccessful => write!(f, "KEEP_LATEST_SUCCESSFUL"),
            Self::KeepCountLimit => write!(f, "KEEP_COUNT_LIMIT"),
            Self::ProtectedActive => write!(f, "PROTECTED_ACTIVE"),
            Self::ProtectedSoleSnapshot => write!(f, "PROTECTED_SOLE_SNAPSHOT"),
            Self::PruneExpired => write!(f, "PRUNE_EXPIRED"),
            Self::PruneExcessSnapshot => write!(f, "PRUNE_EXCESS_SNAPSHOT"),
            Self::PruneFailedSnapshot => write!(f, "PRUNE_FAILED_SNAPSHOT"),
            Self::PruneIncompleteSnapshot => write!(f, "PRUNE_INCOMPLETE_SNAPSHOT"),
            Self::PruneEmpty => write!(f, "PRUNE_EMPTY"),
        }
    }
}

/// Candidate snapshot evaluated by the retention policy engine.
#[derive(Debug, Clone)]
pub struct RetentionCandidate {
    /// Snapshot record from the database.
    pub snapshot_id: SnapshotId,
    /// Profile identifier.
    pub profile_id: ProfileId,
    /// Creation timestamp (ISO-8601).
    pub created_at: String,
    /// Backup execution status.
    pub status: BackupStatus,
    /// Number of active file versions bound to this snapshot.
    pub version_count: usize,
    /// Whether this snapshot is actively being created or restored.
    pub is_active: bool,
    /// Calculated age in seconds relative to evaluation reference time.
    pub age_secs: u64,
}

/// Individual retention decision for a candidate snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionDecision {
    /// Target snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Final action (KEEP or PRUNE).
    pub action: RetentionAction,
    /// Auditable reason for the decision.
    pub reason: RetentionReason,
    /// Human-readable explanation.
    pub description: String,
    /// Snapshot status at time of evaluation.
    pub snapshot_status: BackupStatus,
    /// Snapshot creation timestamp.
    pub snapshot_created_at: String,
    /// Calculated age in seconds.
    pub age_secs: u64,
    /// Associated file versions count.
    pub version_count: usize,
}

/// Deterministic evaluation report produced by the retention policy engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionEvaluation {
    /// Associated backup profile.
    pub profile_id: ProfileId,
    /// Timestamp of evaluation (ISO-8601 UTC).
    pub evaluated_at: String,
    /// Effective retention policy applied.
    pub policy: crate::retention::policy::RetentionPolicy,
    /// Decisions for all evaluated candidate snapshots.
    pub decisions: Vec<RetentionDecision>,
    /// Total candidate snapshots evaluated.
    pub snapshots_evaluated: usize,
    /// Total snapshots determined to KEEP.
    pub snapshots_kept: usize,
    /// Total snapshots determined to PRUNE.
    pub snapshots_pruned: usize,
}

/// Comprehensive outcome of a retention operation (dry-run or real execution).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionResult {
    /// Associated backup profile.
    pub profile_id: ProfileId,
    /// Timestamp of execution (ISO-8601 UTC).
    pub executed_at: String,
    /// Whether this was a dry-run evaluation with zero changes applied.
    pub dry_run: bool,
    /// Detailed retention evaluation report.
    pub evaluation: RetentionEvaluation,
    /// List of snapshot identifiers that were actually pruned (empty if dry-run).
    pub pruned_snapshots: Vec<SnapshotId>,
    /// Total version records pruned from database (0 if dry-run).
    pub pruned_versions_count: usize,
    /// Whether execution succeeded completely.
    pub success: bool,
    /// Error message if execution encountered an unrecoverable failure.
    pub error_message: Option<String>,
}
