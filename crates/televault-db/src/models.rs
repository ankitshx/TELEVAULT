//! Strongly-typed database entity records.

use serde::{Deserialize, Serialize};
use televault_core::ids::{ChunkId, FileId, JobId, ProfileId, SnapshotId, VersionId};
use televault_core::models::{BackupStatus, TransferDirection, TransferStatus};

/// Stored backup profile record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileRecord {
    /// Unique profile identifier.
    pub profile_id: ProfileId,
    /// Human-readable profile name (unique).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Root path on the local filesystem configured for backup.
    pub source_path: String,
    /// Whether the profile is currently active.
    pub enabled: bool,
    /// Creation timestamp (ISO-8601 / RFC-3339).
    pub created_at: String,
    /// Last update timestamp (ISO-8601 / RFC-3339).
    pub updated_at: String,
}

/// Stored logical file record.
///
/// Represents the user-visible logical file regardless of how many internal
/// physical chunks it is split into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    /// Unique logical file identifier.
    pub file_id: FileId,
    /// Associated profile ID, or `None` if unassociated/ad-hoc.
    pub profile_id: Option<ProfileId>,
    /// File base name.
    pub file_name: String,
    /// Relative path within the backup profile or root.
    pub relative_path: String,
    /// Total original logical file size in bytes.
    pub original_size: u64,
    /// Inferred or detected MIME type.
    pub mime_type: Option<String>,
    /// Tracking status (e.g., "tracked", "backed_up", "modified").
    pub status: String,
    /// Cryptographic hash of the complete original logical file (e.g., SHA-256 hex).
    pub logical_file_hash: Option<String>,
    /// File system created timestamp.
    pub created_at: String,
    /// File system modified timestamp.
    pub modified_at: Option<String>,
    /// Record created timestamp.
    pub created_timestamp: String,
    /// Record updated timestamp.
    pub updated_timestamp: String,
}

/// Stored manifest record referencing the authoritative serialized manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRecord {
    /// Unique manifest identifier.
    pub manifest_id: String,
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Manifest schema format version (e.g. "v1").
    pub manifest_version: String,
    /// Raw serialized manifest content (JSON).
    pub serialized_manifest: String,
    /// Record creation timestamp.
    pub created_at: String,
    /// Record update timestamp.
    pub updated_at: String,
}

/// Stored physical chunk record representing an internal storage transfer unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRecord {
    /// Unique chunk identifier.
    pub chunk_id: ChunkId,
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated manifest identifier.
    pub manifest_id: String,
    /// Zero-based chunk sequence index within the logical file (0..N-1).
    pub chunk_index: u32,
    /// Logical plaintext size of this chunk before compression/encryption.
    pub plaintext_size: u64,
    /// Actual stored size of this chunk as transferred to remote storage.
    pub stored_size: u64,
    /// Cryptographic integrity hash of the chunk payload.
    pub integrity_hash: String,
    /// Serialized storage reference or provider locator.
    pub storage_reference: String,
    /// Transfer/availability status of the chunk (e.g., "pending", "uploaded", "verified").
    pub status: String,
    /// Record creation timestamp.
    pub created_at: String,
    /// Record update timestamp.
    pub updated_at: String,
}

/// Stored backup snapshot record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    /// Unique snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Associated profile identifier.
    pub profile_id: ProfileId,
    /// Status of the snapshot backup execution.
    pub status: BackupStatus,
    /// Optional metadata payload (e.g., summary stats JSON).
    pub metadata: Option<String>,
    /// Snapshot creation timestamp.
    pub created_at: String,
}

/// Stored file version record binding a snapshot to a specific file manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRecord {
    /// Unique version identifier.
    pub version_id: VersionId,
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Authoritative manifest identifier for this version.
    pub manifest_id: String,
    /// Version status (e.g., "active", "archived").
    pub status: String,
    /// Version timestamp.
    pub created_at: String,
}

/// Stored transfer job record tracking upload or download work units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferJobRecord {
    /// Unique transfer job identifier.
    pub job_id: JobId,
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated chunk identifier, or `None` if full-file job.
    pub chunk_id: Option<ChunkId>,
    /// Direction of transfer (upload or download).
    pub direction: TransferDirection,
    /// Current transfer progress state.
    pub status: TransferStatus,
    /// Progress counter in bytes or units.
    pub progress: u64,
    /// Number of retries attempted.
    pub retry_count: u32,
    /// Last error message encountered, if any.
    pub error_message: Option<String>,
    /// Job creation timestamp.
    pub created_at: String,
    /// Job update timestamp.
    pub updated_at: String,
}

/// Search result item returned from FTS5 queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Logical file identifier.
    pub file_id: FileId,
    /// File base name.
    pub file_name: String,
    /// Relative path.
    pub relative_path: String,
}

/// Database integrity and diagnostics status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Whether the database passed integrity and health checks.
    pub is_healthy: bool,
    /// Number of applied schema migrations.
    pub applied_migrations: usize,
    /// Total profiles stored.
    pub total_profiles: usize,
    /// Total logical files stored.
    pub total_files: usize,
    /// Total physical chunks stored.
    pub total_chunks: usize,
    /// Result of SQLite `PRAGMA quick_check`.
    pub integrity_check: String,
    /// Whether foreign keys are actively enforced.
    pub foreign_keys_enabled: bool,
}

/// Stored recurring backup schedule record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleRecord {
    /// Unique schedule identifier.
    pub schedule_id: televault_core::ids::ScheduleId,
    /// Associated backup profile identifier.
    pub profile_id: ProfileId,
    /// Type of schedule ('interval', 'daily', 'weekly', 'cron').
    pub schedule_type: String,
    /// Schedule recurrence expression.
    pub expression: String,
    /// Timezone strategy ('local' or 'utc').
    pub timezone: String,
    /// Whether the schedule is active.
    pub enabled: bool,
    /// ISO-8601 UTC timestamp of next scheduled execution.
    pub next_run_at: Option<String>,
    /// ISO-8601 UTC timestamp of last executed run.
    pub last_run_at: Option<String>,
    /// Last execution status.
    pub last_status: Option<String>,
    /// Last error code encountered, if any.
    pub last_error_code: Option<String>,
    /// Record creation timestamp.
    pub created_at: String,
    /// Record update timestamp.
    pub updated_at: String,
}

/// Stored history execution record for a schedule run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleHistoryRecord {
    /// Unique execution history record identifier.
    pub history_id: String,
    /// Associated schedule identifier.
    pub schedule_id: televault_core::ids::ScheduleId,
    /// Associated profile identifier.
    pub profile_id: ProfileId,
    /// Execution start timestamp.
    pub started_at: String,
    /// Execution completion timestamp, if finished.
    pub completed_at: Option<String>,
    /// Execution status ('completed', 'failed', 'cancelled', 'skipped').
    pub status: String,
    /// Snapshot identifier if backup completed.
    pub snapshot_id: Option<SnapshotId>,
    /// Total files processed.
    pub files_processed: u64,
    /// Total payload bytes transferred.
    pub bytes_transferred: u64,
    /// Error code if execution failed.
    pub error_code: Option<String>,
    /// Safe human-readable error message.
    pub error_message: Option<String>,
}
