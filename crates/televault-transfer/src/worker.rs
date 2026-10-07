//! Upload and download transfer execution workers.

use crate::cancellation::{CancellationToken, CancellingReader, CancellingWriter};
use crate::error::{Result, TransferError};
use crate::job::TransferJob;
use crate::progress::{ProgressCallback, ProgressReader, ProgressWriter, TransferProgress};
use crate::retry::RetryPolicy;
use crate::state::TransferState;
use std::io::{Read, Write};
use std::sync::Arc;
use televault_manifest::StorageReference;
use televault_storage::{DownloadRequest, StorageProvider, UploadRequest, VerificationRequest};

/// Dedicated worker executing upload transfer jobs through a [`StorageProvider`].
#[derive(Clone)]
pub struct UploadWorker {
    provider: Arc<dyn StorageProvider + Send + Sync>,
    retry_policy: RetryPolicy,
    progress_callback: Option<ProgressCallback>,
}

impl UploadWorker {
    /// Creates a new [`UploadWorker`].
    pub fn new(
        provider: Arc<dyn StorageProvider + Send + Sync>,
        retry_policy: RetryPolicy,
        progress_callback: Option<ProgressCallback>,
    ) -> Self {
        Self {
            provider,
            retry_policy,
            progress_callback,
        }
    }

    /// Executes an upload transfer job by streaming from `reader` into the cloud storage provider.
    pub fn execute_upload(
        &self,
        job: &mut TransferJob,
        reader: &mut dyn Read,
        cancellation: &CancellationToken,
    ) -> Result<StorageReference> {
        // 1. Cooperative cancellation check
        if cancellation.is_cancelled() {
            let _ = job.transition(TransferState::Cancelled);
            return Err(TransferError::Cancelled);
        }

        // 2. Prepare transfer
        job.transition(TransferState::Preparing)?;

        let req = UploadRequest {
            file_id: job.file_id.clone(),
            chunk_id: job.chunk_id.clone(),
            chunk_index: job.chunk_index,
            total_chunks: job.total_chunks,
            expected_size_bytes: job.expected_size_bytes,
            expected_sha256: job.expected_sha256.clone(),
            resumable: false,
        };

        // 3. Start transferring over bounded stream with cancellation checks
        job.transition(TransferState::Transferring)?;

        let progress_ctx = crate::progress::ProgressContext {
            job_id: job.job_id.clone(),
            file_id: job.file_id.clone(),
            chunk_id: job.chunk_id.clone(),
            chunk_index: job.chunk_index,
            total_chunks: job.total_chunks,
            total_bytes: job.expected_size_bytes,
        };

        let mut cancelling_reader = CancellingReader::new(reader, cancellation);
        let mut progress_reader = ProgressReader::new(
            &mut cancelling_reader,
            progress_ctx,
            self.progress_callback.clone(),
        );

        let upload_res = match self.provider.upload(&req, &mut progress_reader) {
            Ok(res) => res,
            Err(storage_err) => {
                if cancellation.is_cancelled() {
                    let _ = job.transition(TransferState::Cancelled);
                    return Err(TransferError::Cancelled);
                }

                let transfer_err = TransferError::Storage(storage_err);
                if transfer_err.is_retryable() && self.retry_policy.can_retry(job.retry_count) {
                    job.transition(TransferState::Retrying)?;
                    job.retry_count += 1;
                    job.error_message = Some(transfer_err.to_string());
                    return Err(transfer_err);
                } else {
                    job.transition(TransferState::Failed)?;
                    job.error_message = Some(transfer_err.to_string());
                    return Err(transfer_err);
                }
            }
        };

        // 4. Remote verification phase
        if cancellation.is_cancelled() {
            let _ = job.transition(TransferState::Cancelled);
            return Err(TransferError::Cancelled);
        }
        job.transition(TransferState::Verifying)?;

        let v_req = VerificationRequest {
            storage_reference: upload_res.storage_reference.clone(),
            expected_size_bytes: job.expected_size_bytes,
            expected_sha256: upload_res
                .verified_sha256
                .or_else(|| job.expected_sha256.clone()),
        };

        let verified = self
            .provider
            .verify(&v_req)
            .map_err(TransferError::Storage)?;
        if !verified {
            job.transition(TransferState::Failed)?;
            let err_msg = "Remote verification check returned false".to_string();
            job.error_message = Some(err_msg.clone());
            return Err(TransferError::VerificationFailed(err_msg));
        }

        // 5. Finalize completion
        job.transition(TransferState::Completed)?;
        job.storage_reference = Some(upload_res.storage_reference.clone());
        job.bytes_transferred = upload_res.bytes_uploaded;

        // Emit final 100% progress event
        if let Some(ref cb) = self.progress_callback {
            cb(TransferProgress {
                job_id: job.job_id.clone(),
                file_id: job.file_id.clone(),
                chunk_id: job.chunk_id.clone(),
                chunk_index: job.chunk_index,
                total_chunks: job.total_chunks,
                direction: job.direction,
                bytes_transferred: upload_res.bytes_uploaded,
                total_bytes: job.expected_size_bytes,
                state: TransferState::Completed,
            });
        }

        Ok(upload_res.storage_reference)
    }
}

/// Dedicated worker executing download transfer jobs through a [`StorageProvider`].
#[derive(Clone)]
pub struct DownloadWorker {
    provider: Arc<dyn StorageProvider + Send + Sync>,
    retry_policy: RetryPolicy,
    progress_callback: Option<ProgressCallback>,
}

impl DownloadWorker {
    /// Creates a new [`DownloadWorker`].
    pub fn new(
        provider: Arc<dyn StorageProvider + Send + Sync>,
        retry_policy: RetryPolicy,
        progress_callback: Option<ProgressCallback>,
    ) -> Self {
        Self {
            provider,
            retry_policy,
            progress_callback,
        }
    }

    /// Executes a download transfer job by streaming from cloud storage into `writer`.
    pub fn execute_download(
        &self,
        job: &mut TransferJob,
        writer: &mut dyn Write,
        cancellation: &CancellationToken,
    ) -> Result<u64> {
        if cancellation.is_cancelled() {
            let _ = job.transition(TransferState::Cancelled);
            return Err(TransferError::Cancelled);
        }

        let storage_ref = job.storage_reference.clone().ok_or_else(|| {
            TransferError::InvalidJob("Download job missing storage_reference".into())
        })?;

        job.transition(TransferState::Preparing)?;

        let req = DownloadRequest {
            file_id: job.file_id.clone(),
            chunk_id: job.chunk_id.clone(),
            storage_reference: storage_ref,
            expected_size_bytes: job.expected_size_bytes,
            expected_sha256: job.expected_sha256.clone(),
        };

        job.transition(TransferState::Transferring)?;

        let progress_ctx = crate::progress::ProgressContext {
            job_id: job.job_id.clone(),
            file_id: job.file_id.clone(),
            chunk_id: job.chunk_id.clone(),
            chunk_index: job.chunk_index,
            total_chunks: job.total_chunks,
            total_bytes: job.expected_size_bytes,
        };

        let mut cancelling_writer = CancellingWriter::new(writer, cancellation);
        let mut progress_writer = ProgressWriter::new(
            &mut cancelling_writer,
            progress_ctx,
            self.progress_callback.clone(),
        );

        let dl_res = match self.provider.download(&req, &mut progress_writer) {
            Ok(res) => res,
            Err(storage_err) => {
                if cancellation.is_cancelled() {
                    let _ = job.transition(TransferState::Cancelled);
                    return Err(TransferError::Cancelled);
                }

                let transfer_err = TransferError::Storage(storage_err);
                if transfer_err.is_retryable() && self.retry_policy.can_retry(job.retry_count) {
                    job.transition(TransferState::Retrying)?;
                    job.retry_count += 1;
                    job.error_message = Some(transfer_err.to_string());
                    return Err(transfer_err);
                } else {
                    job.transition(TransferState::Failed)?;
                    job.error_message = Some(transfer_err.to_string());
                    return Err(transfer_err);
                }
            }
        };

        // Finalize download completion
        cancellation.check_cancelled()?;
        job.transition(TransferState::Completed)?;
        job.bytes_transferred = dl_res.bytes_downloaded;

        if let Some(ref cb) = self.progress_callback {
            cb(TransferProgress {
                job_id: job.job_id.clone(),
                file_id: job.file_id.clone(),
                chunk_id: job.chunk_id.clone(),
                chunk_index: job.chunk_index,
                total_chunks: job.total_chunks,
                direction: job.direction,
                bytes_transferred: dl_res.bytes_downloaded,
                total_bytes: job.expected_size_bytes,
                state: TransferState::Completed,
            });
        }

        Ok(dl_res.bytes_downloaded)
    }
}
