//! Strongly typed transfer job specification and database record mapping.

use crate::error::{Result, TransferError};
use crate::state::TransferState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use televault_core::ids::{ChunkId, FileId, JobId};
use televault_core::TransferDirection;
use televault_db::TransferJobRecord;
use televault_manifest::StorageReference;

/// Full specification of a transfer execution unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferJob {
    /// Unique transfer job identifier.
    pub job_id: JobId,
    /// Associated logical file identifier.
    pub file_id: FileId,
    /// Associated physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Contiguous 0-based index of this chunk.
    pub chunk_index: u32,
    /// Total count of chunks comprising the logical file.
    pub total_chunks: u32,
    /// Transfer direction (Upload or Download).
    pub direction: TransferDirection,
    /// Local file source path (for uploads or local staging).
    pub source_path: Option<PathBuf>,
    /// Local file destination path (for downloads).
    pub destination_path: Option<PathBuf>,
    /// Authoritative remote storage reference (for downloads or confirmed uploads).
    pub storage_reference: Option<StorageReference>,
    /// Expected transfer payload size in bytes.
    pub expected_size_bytes: u64,
    /// Expected SHA-256 hexadecimal digest for integrity verification.
    pub expected_sha256: Option<String>,
    /// Current fine-grained execution state.
    pub state: TransferState,
    /// Transferred byte counter.
    pub bytes_transferred: u64,
    /// Number of retries already performed.
    pub retry_count: u32,
    /// Maximum retries permitted for this job.
    pub max_retries: u32,
    /// Last error message encountered, if any.
    pub error_message: Option<String>,
    /// Creation timestamp (ISO-8601).
    pub created_at: String,
    /// Last update timestamp (ISO-8601).
    pub updated_at: String,
}

/// Parameters for creating a new upload transfer job.
#[derive(Debug, Clone)]
pub struct UploadJobParams {
    /// Unique transfer job identifier.
    pub job_id: JobId,
    /// Logical file identifier.
    pub file_id: FileId,
    /// Physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Zero-based chunk index.
    pub chunk_index: u32,
    /// Total chunks in logical file.
    pub total_chunks: u32,
    /// Optional local source path.
    pub source_path: Option<PathBuf>,
    /// Expected transfer payload size in bytes.
    pub expected_size_bytes: u64,
    /// Optional expected SHA-256 digest.
    pub expected_sha256: Option<String>,
    /// Maximum retries permitted.
    pub max_retries: u32,
}

impl UploadJobParams {
    /// Creates parameters for an upload job with default single-chunk configuration.
    pub fn new(
        job_id: JobId,
        file_id: FileId,
        chunk_id: ChunkId,
        expected_size_bytes: u64,
    ) -> Self {
        Self {
            job_id,
            file_id,
            chunk_id,
            chunk_index: 0,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes,
            expected_sha256: None,
            max_retries: 3,
        }
    }

    /// Sets chunk index and total chunk count.
    pub fn with_chunk(mut self, chunk_index: u32, total_chunks: u32) -> Self {
        self.chunk_index = chunk_index;
        self.total_chunks = total_chunks;
        self
    }

    /// Sets optional local source path.
    pub fn with_source_path(mut self, path: PathBuf) -> Self {
        self.source_path = Some(path);
        self
    }

    /// Sets expected SHA-256 digest.
    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.expected_sha256 = Some(sha256.into());
        self
    }

    /// Sets maximum retry attempts.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Parameters for creating a new download transfer job.
#[derive(Debug, Clone)]
pub struct DownloadJobParams {
    /// Unique transfer job identifier.
    pub job_id: JobId,
    /// Logical file identifier.
    pub file_id: FileId,
    /// Physical chunk identifier.
    pub chunk_id: ChunkId,
    /// Zero-based chunk index.
    pub chunk_index: u32,
    /// Total chunks in logical file.
    pub total_chunks: u32,
    /// Optional local destination path.
    pub destination_path: Option<PathBuf>,
    /// Authoritative remote storage reference.
    pub storage_reference: StorageReference,
    /// Expected transfer payload size in bytes.
    pub expected_size_bytes: u64,
    /// Optional expected SHA-256 digest.
    pub expected_sha256: Option<String>,
    /// Maximum retries permitted.
    pub max_retries: u32,
}

impl DownloadJobParams {
    /// Creates parameters for a download job with default single-chunk configuration.
    pub fn new(
        job_id: JobId,
        file_id: FileId,
        chunk_id: ChunkId,
        storage_reference: StorageReference,
        expected_size_bytes: u64,
    ) -> Self {
        Self {
            job_id,
            file_id,
            chunk_id,
            chunk_index: 0,
            total_chunks: 1,
            destination_path: None,
            storage_reference,
            expected_size_bytes,
            expected_sha256: None,
            max_retries: 3,
        }
    }

    /// Sets chunk index and total chunk count.
    pub fn with_chunk(mut self, chunk_index: u32, total_chunks: u32) -> Self {
        self.chunk_index = chunk_index;
        self.total_chunks = total_chunks;
        self
    }

    /// Sets optional local destination path.
    pub fn with_destination_path(mut self, path: PathBuf) -> Self {
        self.destination_path = Some(path);
        self
    }

    /// Sets expected SHA-256 digest.
    pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.expected_sha256 = Some(sha256.into());
        self
    }

    /// Sets maximum retry attempts.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Context for reconstructing a `TransferJob` from a persistent `TransferJobRecord`.
#[derive(Debug, Clone)]
pub struct DbJobContext {
    /// Zero-based chunk index.
    pub chunk_index: u32,
    /// Total chunks in logical file.
    pub total_chunks: u32,
    /// Expected transfer payload size in bytes.
    pub expected_size_bytes: u64,
    /// Optional expected SHA-256 digest.
    pub expected_sha256: Option<String>,
    /// Optional authoritative remote storage reference.
    pub storage_reference: Option<StorageReference>,
    /// Optional local source path.
    pub source_path: Option<PathBuf>,
    /// Optional local destination path.
    pub destination_path: Option<PathBuf>,
    /// Maximum retries permitted.
    pub max_retries: u32,
}

impl TransferJob {
    /// Creates a new upload transfer job.
    pub fn new_upload(params: UploadJobParams) -> Result<Self> {
        let job = Self {
            job_id: params.job_id,
            file_id: params.file_id,
            chunk_id: params.chunk_id,
            chunk_index: params.chunk_index,
            total_chunks: params.total_chunks,
            direction: TransferDirection::Upload,
            source_path: params.source_path,
            destination_path: None,
            storage_reference: None,
            expected_size_bytes: params.expected_size_bytes,
            expected_sha256: params.expected_sha256,
            state: TransferState::Pending,
            bytes_transferred: 0,
            retry_count: 0,
            max_retries: params.max_retries,
            error_message: None,
            created_at: "2026-10-07T12:00:00Z".into(),
            updated_at: "2026-10-07T12:00:00Z".into(),
        };
        job.validate()?;
        Ok(job)
    }

    /// Creates a new download transfer job.
    pub fn new_download(params: DownloadJobParams) -> Result<Self> {
        let job = Self {
            job_id: params.job_id,
            file_id: params.file_id,
            chunk_id: params.chunk_id,
            chunk_index: params.chunk_index,
            total_chunks: params.total_chunks,
            direction: TransferDirection::Download,
            source_path: None,
            destination_path: params.destination_path,
            storage_reference: Some(params.storage_reference),
            expected_size_bytes: params.expected_size_bytes,
            expected_sha256: params.expected_sha256,
            state: TransferState::Pending,
            bytes_transferred: 0,
            retry_count: 0,
            max_retries: params.max_retries,
            error_message: None,
            created_at: "2026-10-07T12:00:00Z".into(),
            updated_at: "2026-10-07T12:00:00Z".into(),
        };
        job.validate()?;
        Ok(job)
    }

    /// Validates the transfer job invariants.
    pub fn validate(&self) -> Result<()> {
        if self.total_chunks == 0 {
            return Err(TransferError::InvalidJob(
                "total_chunks must be greater than zero".into(),
            ));
        }
        if self.chunk_index >= self.total_chunks {
            return Err(TransferError::InvalidJob(format!(
                "chunk_index {} out of bounds for total_chunks {}",
                self.chunk_index, self.total_chunks
            )));
        }
        match self.direction {
            TransferDirection::Download => {
                if self.storage_reference.is_none() {
                    return Err(TransferError::InvalidJob(
                        "Download job requires a valid storage_reference".into(),
                    ));
                }
            }
            TransferDirection::Upload => {}
        }
        Ok(())
    }

    /// Mutates the job state following state machine transition rules.
    pub fn transition(&mut self, next: TransferState) -> Result<()> {
        self.state.transition_to(next)?;
        self.updated_at = "2026-10-07T12:00:00Z".into();
        Ok(())
    }

    /// Converts this in-memory transfer job into a persistent `televault-db` `TransferJobRecord`.
    pub fn to_db_record(&self) -> TransferJobRecord {
        TransferJobRecord {
            job_id: self.job_id.clone(),
            file_id: self.file_id.clone(),
            chunk_id: Some(self.chunk_id.clone()),
            direction: self.direction,
            status: self.state.to_transfer_status(),
            progress: self.bytes_transferred,
            retry_count: self.retry_count,
            error_message: self.error_message.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    /// Reconstructs a `TransferJob` from a persistent `TransferJobRecord` and contextual metadata.
    pub fn from_db_record(record: &TransferJobRecord, ctx: DbJobContext) -> Result<Self> {
        let chunk_id = record
            .chunk_id
            .clone()
            .unwrap_or_else(|| ChunkId::new("chunk-0").unwrap());

        let state = match record.status {
            televault_core::TransferStatus::Pending => TransferState::Pending,
            televault_core::TransferStatus::Transferring => TransferState::Transferring,
            televault_core::TransferStatus::Paused => TransferState::Retrying,
            televault_core::TransferStatus::Completed => TransferState::Completed,
            televault_core::TransferStatus::Failed => TransferState::Failed,
            televault_core::TransferStatus::Cancelled => TransferState::Cancelled,
        };

        let job = Self {
            job_id: record.job_id.clone(),
            file_id: record.file_id.clone(),
            chunk_id,
            chunk_index: ctx.chunk_index,
            total_chunks: ctx.total_chunks,
            direction: record.direction,
            source_path: ctx.source_path,
            destination_path: ctx.destination_path,
            storage_reference: ctx.storage_reference,
            expected_size_bytes: ctx.expected_size_bytes,
            expected_sha256: ctx.expected_sha256,
            state,
            bytes_transferred: record.progress,
            retry_count: record.retry_count,
            max_retries: ctx.max_retries,
            error_message: record.error_message.clone(),
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
        };
        job.validate()?;
        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_job_validation() {
        let job = TransferJob::new_upload(UploadJobParams {
            job_id: JobId::new("job-01").unwrap(),
            file_id: FileId::new("file-01").unwrap(),
            chunk_id: ChunkId::new("chunk-01").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes: 1024,
            expected_sha256: None,
            max_retries: 3,
        });
        assert!(job.is_ok());

        // Out of bounds chunk index
        let invalid = TransferJob::new_upload(UploadJobParams {
            job_id: JobId::new("job-02").unwrap(),
            file_id: FileId::new("file-02").unwrap(),
            chunk_id: ChunkId::new("chunk-02").unwrap(),
            chunk_index: 5,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes: 1024,
            expected_sha256: None,
            max_retries: 3,
        });
        assert!(invalid.is_err());
    }

    #[test]
    fn test_db_record_roundtrip() {
        let job = TransferJob::new_upload(UploadJobParams {
            job_id: JobId::new("job-db").unwrap(),
            file_id: FileId::new("file-db").unwrap(),
            chunk_id: ChunkId::new("chunk-db").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes: 2048,
            expected_sha256: Some("abcdef123456".into()),
            max_retries: 3,
        })
        .unwrap();

        let record = job.to_db_record();
        assert_eq!(record.job_id, job.job_id);
        assert_eq!(record.file_id, job.file_id);
        assert_eq!(record.direction, TransferDirection::Upload);

        let reconstructed = TransferJob::from_db_record(
            &record,
            DbJobContext {
                chunk_index: 0,
                total_chunks: 1,
                expected_size_bytes: 2048,
                expected_sha256: Some("abcdef123456".into()),
                storage_reference: None,
                source_path: None,
                destination_path: None,
                max_retries: 3,
            },
        )
        .unwrap();

        assert_eq!(reconstructed.job_id, job.job_id);
        assert_eq!(reconstructed.chunk_id, job.chunk_id);
    }
}
