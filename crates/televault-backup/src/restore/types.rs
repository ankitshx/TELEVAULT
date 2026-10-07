//! Domain types and requests for restore operations and backup verification.

use super::collision::CollisionPolicy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use televault_core::ids::{FileId, SnapshotId, VersionId};
use televault_crypto::policy::EncryptionPolicy;

/// Concrete outcome of an individual file restore operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRestoreOutcome {
    /// File was successfully restored to the target path (destination did not previously exist).
    Restored,
    /// Restore was skipped because destination file existed and policy was [`CollisionPolicy::Skip`].
    Skipped,
    /// Destination file existed and was replaced according to [`CollisionPolicy::Overwrite`].
    Overwritten,
    /// Destination file existed and content was restored to an alternate path via [`CollisionPolicy::KeepBoth`].
    KeptBoth,
    /// Restore operation was cancelled before finalization.
    Cancelled,
    /// Restore failed with an unrecoverable error.
    Failed,
}

/// Request to restore a single logical file or version to local disk.
#[derive(Debug, Clone)]
pub struct RestoreRequest {
    /// Logical file identifier.
    pub file_id: FileId,
    /// Specific version to restore. If `None`, the latest active version is resolved from the database.
    pub version_id: Option<VersionId>,
    /// Specific manifest ID to restore. If `None`, resolved from version or file catalog record.
    pub manifest_id: Option<String>,
    /// User-selected target destination (either destination directory or destination file path).
    pub destination_path: PathBuf,
    /// Policy governing destination file collision resolution.
    pub collision_policy: CollisionPolicy,
    /// Whether to enforce full cryptographic hash verification against the manifest.
    pub verify_integrity: bool,
    /// Optional encryption policy with secret key for decryption if file is encrypted.
    pub encryption_policy: EncryptionPolicy,
}

impl RestoreRequest {
    /// Constructs a basic restore request for a file to a destination path with default policies.
    pub fn new(file_id: FileId, destination_path: PathBuf) -> Self {
        Self {
            file_id,
            version_id: None,
            manifest_id: None,
            destination_path,
            collision_policy: CollisionPolicy::default(),
            verify_integrity: true,
            encryption_policy: EncryptionPolicy::Disabled,
        }
    }

    /// Sets the specific version ID to restore.
    pub fn with_version(mut self, version_id: VersionId) -> Self {
        self.version_id = Some(version_id);
        self
    }

    /// Sets the collision policy.
    pub fn with_collision_policy(mut self, policy: CollisionPolicy) -> Self {
        self.collision_policy = policy;
        self
    }

    /// Sets the encryption policy with decryption key.
    pub fn with_encryption_policy(mut self, policy: EncryptionPolicy) -> Self {
        self.encryption_policy = policy;
        self
    }
}

/// Result summary of an individual file restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// Logical file identifier.
    pub file_id: FileId,
    /// Relative path of the restored file within its backup profile.
    pub relative_path: String,
    /// Final absolute filesystem path where the file was written (or skipped).
    pub target_path: PathBuf,
    /// Outcome of the restore operation.
    pub outcome: FileRestoreOutcome,
    /// Number of plaintext bytes restored and written to disk.
    pub bytes_restored: u64,
    /// Verified whole-file SHA-256 hash.
    pub verified_sha256: Option<String>,
    /// Time elapsed during restore in milliseconds.
    pub elapsed_ms: u64,
    /// Error message if the restore failed.
    pub error: Option<String>,
}

/// Request to restore an entire point-in-time snapshot to a local directory.
#[derive(Debug, Clone)]
pub struct SnapshotRestoreRequest {
    /// Snapshot identifier to restore.
    pub snapshot_id: SnapshotId,
    /// Destination root directory where snapshot files will be reconstructed.
    pub destination_dir: PathBuf,
    /// Policy governing destination file collision resolution.
    pub collision_policy: CollisionPolicy,
    /// Whether to enforce full cryptographic hash verification.
    pub verify_integrity: bool,
    /// Optional encryption policy with secret key for decryption.
    pub encryption_policy: EncryptionPolicy,
}

impl SnapshotRestoreRequest {
    /// Constructs a basic snapshot restore request.
    pub fn new(snapshot_id: SnapshotId, destination_dir: PathBuf) -> Self {
        Self {
            snapshot_id,
            destination_dir,
            collision_policy: CollisionPolicy::default(),
            verify_integrity: true,
            encryption_policy: EncryptionPolicy::Disabled,
        }
    }

    /// Sets the collision policy.
    pub fn with_collision_policy(mut self, policy: CollisionPolicy) -> Self {
        self.collision_policy = policy;
        self
    }

    /// Sets the encryption policy with decryption key.
    pub fn with_encryption_policy(mut self, policy: EncryptionPolicy) -> Self {
        self.encryption_policy = policy;
        self
    }
}

/// Result summary of a complete snapshot restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRestoreResult {
    /// Snapshot identifier that was restored.
    pub snapshot_id: SnapshotId,
    /// Total number of files evaluated in the snapshot.
    pub total_files: usize,
    /// Number of files successfully restored.
    pub restored_files: usize,
    /// Number of files skipped due to [`CollisionPolicy::Skip`].
    pub skipped_files: usize,
    /// Number of existing files overwritten.
    pub overwritten_files: usize,
    /// Number of files kept alongside existing files using alternate filenames.
    pub kept_both_files: usize,
    /// Number of file restore failures.
    pub failed_files: usize,
    /// Total aggregate plaintext bytes restored across all files.
    pub total_bytes_restored: u64,
    /// Total elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Individual file restore results.
    pub file_results: Vec<RestoreResult>,
}

/// Fast metadata verification report assessing whether a manifest is complete and restorable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestVerificationReport {
    /// Manifest identifier checked.
    pub manifest_id: String,
    /// Logical file identifier.
    pub file_id: FileId,
    /// File relative path.
    pub relative_path: String,
    /// Whether all checks passed and the manifest is completely restorable.
    pub is_restorable: bool,
    /// Total chunks in the manifest.
    pub total_chunks: usize,
    /// Original uncompressed plaintext size.
    pub original_size: u64,
    /// List of detected validation or integrity issues.
    pub issues: Vec<String>,
    /// Whether remote objects were confirmed via storage provider query.
    pub remote_objects_verified: bool,
}

/// Complete end-to-end trial restore verification report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullVerificationReport {
    /// Manifest identifier tested.
    pub manifest_id: String,
    /// Logical file identifier.
    pub file_id: FileId,
    /// Whether the full trial restore succeeded and hash matched.
    pub is_valid: bool,
    /// Total chunks downloaded and verified.
    pub total_chunks: usize,
    /// Original uncompressed plaintext size.
    pub original_size: u64,
    /// Calculated whole-file SHA-256 hash.
    pub calculated_sha256: Option<String>,
    /// Expected whole-file SHA-256 hash from manifest.
    pub expected_sha256: String,
    /// Total verification time in milliseconds.
    pub elapsed_ms: u64,
    /// Error message if trial restore failed.
    pub error: Option<String>,
}
