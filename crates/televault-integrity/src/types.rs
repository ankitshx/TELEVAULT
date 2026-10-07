//! Domain types for remote backup verification, integrity audit, and ownership isolation.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use televault_core::ids::{ChunkId, FileId, ManifestId, ProfileId, SnapshotId};

/// Verification depth level.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum VerificationLevel {
    /// Level 1 — Local SQLite metadata and manifest structure only.
    MetadataOnly = 1,
    /// Level 2 — Remote object availability verified via StorageProvider without payload download.
    #[default]
    RemoteAvailability = 2,
    /// Level 3 — Streaming remote payload verification with running cryptographic hash computation.
    RemoteIntegrity = 3,
    /// Level 4 — Comprehensive restore-readiness synthesis including trial decryption and whole-file checks.
    RestoreReadiness = 4,
}

impl fmt::Display for VerificationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MetadataOnly => write!(f, "metadata_only"),
            Self::RemoteAvailability => write!(f, "remote_availability"),
            Self::RemoteIntegrity => write!(f, "remote_integrity"),
            Self::RestoreReadiness => write!(f, "restore_readiness"),
        }
    }
}

/// Overall outcome status of a verification run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// All metadata, ownership relationships, and remote objects are healthy.
    Healthy,
    /// Minor non-critical anomalies detected; backup remains restorable.
    Warning,
    /// Integrity corruption, hash mismatch, or missing required chunks detected.
    Corrupted,
    /// Verification failed due to operational, network, or validation errors.
    Failed,
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Warning => write!(f, "warning"),
            Self::Corrupted => write!(f, "corrupted"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Severity classification of an individual verification finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSeverity {
    /// Informational note (e.g. file modified locally since snapshot).
    Info = 1,
    /// Non-fatal warning (e.g. transient remote unavailability with retry possible).
    Warning = 2,
    /// Definite error impacting integrity or metadata consistency.
    Error = 3,
    /// Critical fatal issue preventing file or snapshot restoration.
    Critical = 4,
}

impl fmt::Display for VerificationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Direct impact of a finding on restoration capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreImpact {
    /// No impact on restore readiness.
    None,
    /// Degraded restore possible (e.g. retryable transient remote unavailability).
    Degraded,
    /// Fatal impact; file cannot be restored from this backup.
    Fatal,
}

impl fmt::Display for RestoreImpact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Degraded => write!(f, "degraded"),
            Self::Fatal => write!(f, "fatal"),
        }
    }
}

/// Strongly typed machine-readable issue code for verification findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationIssueCode {
    // Level 1: Metadata & Structure
    /// Manifest record not found in database.
    ManifestNotFound,
    /// Manifest JSON or structure failed schema validation.
    ManifestInvalid,
    /// Manifest version is not supported by current engine.
    UnsupportedVersion,
    /// Referenced snapshot does not exist.
    SnapshotNotFound,
    /// Referenced profile does not exist.
    ProfileNotFound,
    /// Referenced file does not exist.
    FileNotFound,
    /// Chunk count does not match expected formula.
    ChunkCountMismatch,
    /// Chunk index sequence has gaps or is not contiguous.
    ChunkIndexDiscontinuous,
    /// Duplicate chunk index detected within the manifest.
    DuplicateChunkIndex,
    /// Chunk size does not match expected chunking boundary.
    ChunkSizeMismatch,
    /// Chunk storage reference is still in Pending state.
    PendingStorageAllocation,
    /// Logical file size is negative or inconsistent with chunks.
    InvalidLogicalFileSize,
    /// File path contains directory traversal or forbidden characters.
    InvalidRelativePath,

    // Ownership & Profile Isolation (Mandatory Phase 13)
    /// Target does not belong to expected profile.
    ProfileMismatch,
    /// Snapshot does not belong to expected profile.
    SnapshotOwnershipViolation,
    /// File does not belong to expected profile.
    FileOwnershipViolation,
    /// Manifest does not belong to expected file or profile.
    ManifestOwnershipViolation,
    /// Chunk does not belong to expected manifest.
    ChunkOwnershipViolation,
    /// Remote storage reference is associated with a different chunk.
    RemoteReferenceOwnershipViolation,
    /// Reference points to an object owned by another profile.
    CrossProfileReference,

    // Level 2: Remote Availability
    /// Remote storage provider confirmed object is missing (HTTP 404 / TG invalid).
    ConfirmedMissing,
    /// Remote storage provider returned a transient or connection error.
    RemoteUnavailable,
    /// Remote object size or metadata does not match local record.
    RemoteMetadataMismatch,
    /// Telegram storage reference is malformed.
    InvalidTelegramReference,
    /// Telegram reference specifies unexpected chat ID.
    WrongChatId,
    /// Telegram reference specifies unexpected message ID.
    WrongMessageId,
    /// Telegram reference specifies unexpected file ID.
    WrongFileId,

    // Level 3: Remote Integrity
    /// Plaintext chunk SHA-256 hash mismatch.
    ChunkHashMismatch,
    /// Reconstructed logical file SHA-256 hash mismatch.
    WholeFileHashMismatch,
    /// Stored/ciphertext chunk SHA-256 hash mismatch.
    StoredHashMismatch,
    /// Downloaded payload corrupted or truncated.
    PayloadCorrupted,

    // Crypto & Compression
    /// AEAD authentication tag verification failed.
    AuthenticationFailed,
    /// Additional authenticated data (AAD) mismatch.
    InvalidAad,
    /// AES-256-GCM decryption failed.
    DecryptionFailed,
    /// Zstandard decompression failed.
    DecompressionFailed,

    // Operational
    /// Verification cancelled cooperatively.
    Cancelled,
    /// Verification timed out.
    Timeout,
    /// Internal unexpected error during verification.
    InternalError,
}

impl fmt::Display for VerificationIssueCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// An itemized finding discovered during verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationFinding {
    /// Unique identifier for this finding.
    pub finding_id: String,
    /// Associated profile identifier if known.
    pub profile_id: Option<ProfileId>,
    /// Associated snapshot identifier if known.
    pub snapshot_id: Option<SnapshotId>,
    /// Associated logical file identifier if known.
    pub file_id: Option<FileId>,
    /// Associated manifest identifier if known.
    pub manifest_id: Option<ManifestId>,
    /// Associated chunk identifier if known.
    pub chunk_id: Option<ChunkId>,
    /// Zero-based chunk index if applicable.
    pub chunk_index: Option<u32>,
    /// Verification depth level at which this finding was detected.
    pub level: VerificationLevel,
    /// Severity classification.
    pub severity: VerificationSeverity,
    /// Structured issue code.
    pub code: VerificationIssueCode,
    /// Human-readable explanation of the finding.
    pub description: String,
    /// Impact on future restore operations.
    pub restore_impact: RestoreImpact,
    /// ISO-8601 UTC timestamp when finding was identified.
    pub timestamp: String,
}

impl VerificationFinding {
    /// Constructs a new verification finding with current timestamp and generated finding ID.
    pub fn new(
        level: VerificationLevel,
        severity: VerificationSeverity,
        code: VerificationIssueCode,
        description: impl Into<String>,
        restore_impact: RestoreImpact,
    ) -> Self {
        let nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Self {
            finding_id: format!("find-{nanos}"),
            profile_id: None,
            snapshot_id: None,
            file_id: None,
            manifest_id: None,
            chunk_id: None,
            chunk_index: None,
            level,
            severity,
            code,
            description: description.into(),
            restore_impact,
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    /// Builder method attaching profile context.
    pub fn with_profile(mut self, profile_id: ProfileId) -> Self {
        self.profile_id = Some(profile_id);
        self
    }

    /// Builder method attaching snapshot context.
    pub fn with_snapshot(mut self, snapshot_id: SnapshotId) -> Self {
        self.snapshot_id = Some(snapshot_id);
        self
    }

    /// Builder method attaching file context.
    pub fn with_file(mut self, file_id: FileId) -> Self {
        self.file_id = Some(file_id);
        self
    }

    /// Builder method attaching manifest context.
    pub fn with_manifest(mut self, manifest_id: ManifestId) -> Self {
        self.manifest_id = Some(manifest_id);
        self
    }

    /// Builder method attaching chunk context.
    pub fn with_chunk(mut self, chunk_id: ChunkId, index: u32) -> Self {
        self.chunk_id = Some(chunk_id);
        self.chunk_index = Some(index);
        self
    }
}

/// Aggregate metrics and counts for a verification operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationSummary {
    /// Total logical files evaluated.
    pub total_files: u32,
    /// Total manifests evaluated.
    pub total_manifests: u32,
    /// Total physical chunks evaluated.
    pub total_chunks: u32,
    /// Total chunks verified completely healthy.
    pub healthy_chunks: u32,
    /// Total chunks with confirmed corruption or hash mismatches.
    pub corrupted_chunks: u32,
    /// Total chunks confirmed missing on remote storage.
    pub missing_chunks: u32,
    /// Total cross-profile or ownership violations detected.
    pub ownership_violations: u32,
    /// Whether the backup is confirmed ready for full restore.
    pub is_restore_ready: bool,
    /// Overall verification status.
    pub status: VerificationStatus,
    /// Elapsed duration of verification in milliseconds.
    pub duration_ms: u64,
}

impl Default for VerificationSummary {
    fn default() -> Self {
        Self {
            total_files: 0,
            total_manifests: 0,
            total_chunks: 0,
            healthy_chunks: 0,
            corrupted_chunks: 0,
            missing_chunks: 0,
            ownership_violations: 0,
            is_restore_ready: true,
            status: VerificationStatus::Healthy,
            duration_ms: 0,
        }
    }
}

/// Complete report returned from a verification run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Target entity type ("file", "manifest", "snapshot", "profile").
    pub target_type: String,
    /// Target entity identifier string.
    pub target_id: String,
    /// Scoped backup profile identifier.
    pub profile_id: ProfileId,
    /// Verification depth level applied.
    pub level: VerificationLevel,
    /// Overall verification status.
    pub status: VerificationStatus,
    /// Whether the backup is confirmed restore-ready.
    pub is_restore_ready: bool,
    /// List of all itemized findings discovered.
    pub findings: Vec<VerificationFinding>,
    /// Summary metrics.
    pub summary: VerificationSummary,
    /// Execution timestamp (ISO-8601 UTC).
    pub verified_at: String,
}

/// Configurable options passed to a verification operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOptions {
    /// Requested verification depth level.
    pub level: VerificationLevel,
    /// Whether to calculate running cryptographic hash on remote payload streams.
    pub full_hash_check: bool,
    /// Whether to perform trial decryption to verify authentication tags.
    pub decrypt_check: bool,
    /// Optional timeout in seconds.
    pub timeout_secs: Option<u64>,
}

impl Default for VerificationOptions {
    fn default() -> Self {
        Self {
            level: VerificationLevel::RemoteAvailability,
            full_hash_check: false,
            decrypt_check: false,
            timeout_secs: Some(300),
        }
    }
}

impl VerificationOptions {
    /// Creates options for fast Level 1 metadata-only verification.
    pub fn metadata_only() -> Self {
        Self {
            level: VerificationLevel::MetadataOnly,
            full_hash_check: false,
            decrypt_check: false,
            timeout_secs: Some(30),
        }
    }

    /// Creates options for Level 2 remote availability verification.
    pub fn remote_availability() -> Self {
        Self {
            level: VerificationLevel::RemoteAvailability,
            full_hash_check: false,
            decrypt_check: false,
            timeout_secs: Some(120),
        }
    }

    /// Creates options for Level 3 full streaming integrity verification.
    pub fn full_integrity() -> Self {
        Self {
            level: VerificationLevel::RemoteIntegrity,
            full_hash_check: true,
            decrypt_check: true,
            timeout_secs: Some(600),
        }
    }

    /// Creates options for Level 4 full restore-readiness verification.
    pub fn restore_readiness() -> Self {
        Self {
            level: VerificationLevel::RestoreReadiness,
            full_hash_check: true,
            decrypt_check: true,
            timeout_secs: Some(600),
        }
    }
}
