//! Domain contracts for storage requests, results, status lifecycle, and objects.

use serde::{Deserialize, Serialize};
use std::fmt;
use televault_core::ids::{ChunkId, FileId};
use televault_manifest::StorageReference;

/// Default streaming buffer size for bounded zero-allocation I/O transfers (64 KiB).
pub const STREAM_CHUNK_BUFFER_SIZE: usize = 64 * 1024;

/// Lifecycle state of a remote storage object.
///
/// Implements the remote-first completion contract: a backup object is NEVER
/// considered remotely complete until it reaches the [`StorageStatus::Verified`] state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageStatus {
    /// Initial queued state before upload dispatch.
    Pending,
    /// Actively transmitting payload bytes to the remote provider.
    Uploading,
    /// Remote payload transmission completed; pending server-side confirmation/hash verification.
    Uploaded,
    /// Integrity and availability verification in progress.
    Verifying,
    /// Remote storage confirmed, verified, and finalized in the cloud.
    Verified,
    /// Upload or verification encountered an unrecoverable or retryable error.
    Failed,
    /// Scheduled for exponential backoff or retry attempt.
    Retrying,
    /// Deleted from remote cloud storage.
    Deleted,
}

impl StorageStatus {
    /// Returns true ONLY if the object has been remotely stored and cryptographically verified.
    ///
    /// Neither `Uploading` nor `Uploaded` states satisfy remote completion.
    pub fn is_remotely_complete(&self) -> bool {
        matches!(self, Self::Verified)
    }

    /// Returns true if the state indicates an active in-flight transfer.
    pub fn is_in_flight(&self) -> bool {
        matches!(self, Self::Uploading | Self::Verifying)
    }
}

impl fmt::Display for StorageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Uploading => write!(f, "uploading"),
            Self::Uploaded => write!(f, "uploaded"),
            Self::Verifying => write!(f, "verifying"),
            Self::Verified => write!(f, "verified"),
            Self::Failed => write!(f, "failed"),
            Self::Retrying => write!(f, "retrying"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

/// Metadata describing an object stored with a remote provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteObjectMetadata {
    /// Remote provider-specific identifier (e.g. Telegram message ID string).
    pub remote_id: String,
    /// Exact byte size confirmed on the remote store.
    pub size_bytes: u64,
    /// Verified SHA-256 hex digest if available from the provider or manifest.
    pub sha256_hash: Option<String>,
    /// Authoritative storage reference.
    pub storage_reference: StorageReference,
    /// Creation or upload timestamp (RFC-3339 / ISO-8601).
    pub created_at: Option<String>,
}

/// Request to upload a stream payload to a remote storage provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadRequest {
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Zero-based sequential index (0, 1, ..., N-1) within the logical file.
    pub chunk_index: u32,
    /// Total chunks comprising the parent logical file.
    pub total_chunks: u32,
    /// Expected byte size of the payload.
    pub expected_size_bytes: u64,
    /// Expected SHA-256 hex digest of the payload for post-upload verification.
    pub expected_sha256: Option<String>,
    /// Whether this upload request is allowed to resume a partially transferred object.
    pub resumable: bool,
}

/// Result returned after uploading a payload to a remote storage provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadResult {
    /// Authoritative storage reference for retrieving or verifying the uploaded object.
    pub storage_reference: StorageReference,
    /// Total bytes confirmed transmitted to the remote provider.
    pub bytes_uploaded: u64,
    /// Cryptographic SHA-256 hex digest calculated during the streaming upload.
    pub verified_sha256: Option<String>,
    /// Final verified storage status.
    pub remote_status: StorageStatus,
}

/// Request to download a remote storage object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadRequest {
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Authoritative storage reference indicating where the chunk is stored.
    pub storage_reference: StorageReference,
    /// Expected size of the downloaded payload.
    pub expected_size_bytes: u64,
    /// Expected SHA-256 hex digest to verify during streaming download.
    pub expected_sha256: Option<String>,
}

/// Result returned after downloading a payload from a remote storage provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadResult {
    /// Total bytes downloaded and written to the output sink.
    pub bytes_downloaded: u64,
    /// Calculated SHA-256 digest of the downloaded payload.
    pub verified_sha256: Option<String>,
}

/// Request to verify the existence and integrity of a remote storage object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRequest {
    /// Storage reference identifying the remote object.
    pub storage_reference: StorageReference,
    /// Expected byte size of the remote object.
    pub expected_size_bytes: u64,
    /// Expected SHA-256 hex digest of the remote object, if verification is requested.
    pub expected_sha256: Option<String>,
}

/// Request to delete an object from remote storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteRequest {
    /// Storage reference identifying the remote object to delete.
    pub storage_reference: StorageReference,
}

/// High-level domain representation of a tracked physical storage object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageObject {
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Chunk index within logical file.
    pub chunk_index: u32,
    /// Stored size in bytes.
    pub size_bytes: u64,
    /// Remote storage status.
    pub status: StorageStatus,
    /// Storage reference, present once upload succeeds.
    pub storage_reference: Option<StorageReference>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_first_completion_semantics() {
        assert!(!StorageStatus::Pending.is_remotely_complete());
        assert!(!StorageStatus::Uploading.is_remotely_complete());
        assert!(!StorageStatus::Uploaded.is_remotely_complete());
        assert!(!StorageStatus::Verifying.is_remotely_complete());
        assert!(!StorageStatus::Failed.is_remotely_complete());
        assert!(!StorageStatus::Retrying.is_remotely_complete());
        assert!(!StorageStatus::Deleted.is_remotely_complete());

        // ONLY Verified represents remote-first completion
        assert!(StorageStatus::Verified.is_remotely_complete());
    }

    #[test]
    fn test_storage_status_display() {
        assert_eq!(StorageStatus::Pending.to_string(), "pending");
        assert_eq!(StorageStatus::Uploading.to_string(), "uploading");
        assert_eq!(StorageStatus::Uploaded.to_string(), "uploaded");
        assert_eq!(StorageStatus::Verifying.to_string(), "verifying");
        assert_eq!(StorageStatus::Verified.to_string(), "verified");
        assert_eq!(StorageStatus::Failed.to_string(), "failed");
        assert_eq!(StorageStatus::Retrying.to_string(), "retrying");
        assert_eq!(StorageStatus::Deleted.to_string(), "deleted");
    }
}
