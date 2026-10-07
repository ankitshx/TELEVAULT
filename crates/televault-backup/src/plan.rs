//! Backup planning and execution summary models.

use crate::change_detector::ChangeSet;
use crate::profile::BackupProfile;
use serde::{Deserialize, Serialize};
use televault_core::ids::{ProfileId, SnapshotId};
use televault_core::models::BackupStatus;

/// Deterministic plan representing the operations required for a backup execution.
#[derive(Debug, Clone, PartialEq)]
pub struct BackupPlan {
    /// Target backup profile.
    pub profile: BackupProfile,
    /// Unique snapshot identifier allocated for this execution.
    pub snapshot_id: SnapshotId,
    /// Detailed change set identifying new, modified, unchanged, and deleted files.
    pub change_set: ChangeSet,
    /// Count of files requiring processing and transfer.
    pub files_to_transfer: usize,
    /// Total bytes requiring network transfer.
    pub bytes_to_transfer: u64,
    /// Count of unchanged files reused from previous snapshot without transfer.
    pub unchanged_files_reused: usize,
    /// Total bytes of unchanged files reused without network transfer.
    pub unchanged_bytes_reused: u64,
}

impl BackupPlan {
    /// Creates a new `BackupPlan` from a profile, snapshot ID, and change set.
    pub fn new(profile: BackupProfile, snapshot_id: SnapshotId, change_set: ChangeSet) -> Self {
        let files_to_transfer = change_set.count_transfers_needed();
        let bytes_to_transfer = change_set.total_transfer_bytes();
        let unchanged_files_reused = change_set.unchanged_files().len();
        let unchanged_bytes_reused = change_set.total_reused_bytes();

        Self {
            profile,
            snapshot_id,
            change_set,
            files_to_transfer,
            bytes_to_transfer,
            unchanged_files_reused,
            unchanged_bytes_reused,
        }
    }
}

/// Comprehensive summary of a completed or failed backup execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSummary {
    /// Associated snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Associated profile identifier.
    pub profile_id: ProfileId,
    /// Number of new files added.
    pub new_files: usize,
    /// Number of modified files updated.
    pub modified_files: usize,
    /// Number of unchanged files reused.
    pub unchanged_files: usize,
    /// Number of deleted files recorded.
    pub deleted_files: usize,
    /// Total physical chunks transferred.
    pub transferred_chunks: usize,
    /// Total bytes transferred over network.
    pub transferred_bytes: u64,
    /// Total bytes of unchanged files preserved without re-upload.
    pub reused_bytes: u64,
    /// Terminal execution status.
    pub status: BackupStatus,
    /// Total execution duration in milliseconds.
    pub elapsed_ms: u64,
    /// Optional error message if backup failed.
    pub error_message: Option<String>,
}
