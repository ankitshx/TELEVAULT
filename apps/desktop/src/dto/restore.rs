//! Restore domain request and response DTOs.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Policy for handling existing target files at destination during restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicyDto {
    /// Replace the existing file with the restored payload.
    Overwrite,
    /// Skip restoring without modifying the existing file.
    Skip,
    /// Keep both by generating a deterministic alternate filename (e.g. "doc (1).txt").
    KeepBoth,
}

impl From<CollisionPolicyDto> for televault_backup::restore::CollisionPolicy {
    fn from(dto: CollisionPolicyDto) -> Self {
        match dto {
            CollisionPolicyDto::Overwrite => televault_backup::restore::CollisionPolicy::Overwrite,
            CollisionPolicyDto::Skip => televault_backup::restore::CollisionPolicy::Skip,
            CollisionPolicyDto::KeepBoth => televault_backup::restore::CollisionPolicy::KeepBoth,
        }
    }
}

/// Resulting outcome of a file restore operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FileRestoreOutcomeDto {
    Restored,
    Overwritten,
    Skipped,
    KeptBoth,
    Cancelled,
    Failed,
}

impl From<televault_backup::restore::FileRestoreOutcome> for FileRestoreOutcomeDto {
    fn from(outcome: televault_backup::restore::FileRestoreOutcome) -> Self {
        match outcome {
            televault_backup::restore::FileRestoreOutcome::Restored => {
                FileRestoreOutcomeDto::Restored
            }
            televault_backup::restore::FileRestoreOutcome::Overwritten => {
                FileRestoreOutcomeDto::Overwritten
            }
            televault_backup::restore::FileRestoreOutcome::Skipped => {
                FileRestoreOutcomeDto::Skipped
            }
            televault_backup::restore::FileRestoreOutcome::KeptBoth => {
                FileRestoreOutcomeDto::KeptBoth
            }
            televault_backup::restore::FileRestoreOutcome::Cancelled => {
                FileRestoreOutcomeDto::Cancelled
            }
            televault_backup::restore::FileRestoreOutcome::Failed => FileRestoreOutcomeDto::Failed,
        }
    }
}

/// Request to restore a specific logical file by FileId.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RestoreFileRequest {
    /// File identifier to restore.
    pub file_id: String,
    /// Target local filesystem destination path.
    pub destination_path: String,
    /// Collision policy if target already exists.
    pub collision_policy: CollisionPolicyDto,
    /// Optional decryption passphrase if file was encrypted.
    pub passphrase: Option<String>,
}

/// Request to restore from an arbitrary standalone Manifest JSON string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RestoreManifestRequest {
    /// Raw Manifest V1 JSON string.
    pub manifest_json: String,
    /// Target local filesystem destination path.
    pub destination_path: String,
    /// Collision policy if target already exists.
    pub collision_policy: CollisionPolicyDto,
    /// Optional decryption passphrase if file was encrypted.
    pub passphrase: Option<String>,
}

/// Request to restore an entire historical snapshot folder tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RestoreSnapshotRequest {
    /// Snapshot identifier to restore.
    pub snapshot_id: String,
    /// Root local directory where snapshot files will be reassembled.
    pub destination_directory: String,
    /// Collision policy if destination files already exist.
    pub collision_policy: CollisionPolicyDto,
    /// Optional decryption passphrase if snapshot contains encrypted files.
    pub passphrase: Option<String>,
}

use specta_typescript::Number;

/// Result details of an individual file restore operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RestoreResultDto {
    pub file_id: String,
    pub target_path: String,
    #[specta(type = Number)]
    pub bytes_restored: u64,
    #[specta(type = Number)]
    pub duration_ms: u64,
    pub outcome: FileRestoreOutcomeDto,
}

/// Result details of a full snapshot directory restore operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnapshotRestoreResultDto {
    pub snapshot_id: String,
    pub target_directory: String,
    #[specta(type = Number)]
    pub total_files: usize,
    #[specta(type = Number)]
    pub restored_files: usize,
    #[specta(type = Number)]
    pub skipped_files: usize,
    #[specta(type = Number)]
    pub failed_files: usize,
    #[specta(type = Number)]
    pub total_bytes: u64,
    #[specta(type = Number)]
    pub duration_ms: u64,
}
