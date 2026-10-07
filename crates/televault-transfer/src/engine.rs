//! Transfer engine combining storage provider, queue, retry engine, and workers.

use crate::cancellation::CancellationToken;
use crate::error::{Result, TransferError};
use crate::job::TransferJob;
use crate::progress::ProgressCallback;
use crate::queue::TransferQueue;
use crate::retry::RetryPolicy;
use crate::worker::{DownloadWorker, UploadWorker};
use std::fs::File;
use std::io::{Read, Write};
use std::sync::Arc;
use televault_core::ids::JobId;
use televault_manifest::StorageReference;
use televault_storage::temp::TempPayloadFile;
use televault_storage::StorageProvider;

/// Configuration options for initializing the transfer engine.
#[derive(Debug, Clone)]
pub struct TransferEngineConfig {
    /// Maximum number of concurrent transfer workers.
    pub max_concurrent_transfers: usize,
    /// Configured retry and exponential backoff parameters.
    pub retry_policy: RetryPolicy,
}

impl Default for TransferEngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transfers: 2,
            retry_policy: RetryPolicy::default(),
        }
    }
}

/// Central transfer engine coordinator managing background transfers, queues, and storage adapters.
#[derive(Clone)]
pub struct TransferEngine {
    provider: Arc<dyn StorageProvider + Send + Sync>,
    queue: TransferQueue,
    config: TransferEngineConfig,
    upload_worker: UploadWorker,
    download_worker: DownloadWorker,
}

impl TransferEngine {
    /// Creates a new [`TransferEngine`].
    pub fn new(
        provider: Arc<dyn StorageProvider + Send + Sync>,
        config: TransferEngineConfig,
        progress_callback: Option<ProgressCallback>,
    ) -> Self {
        let upload_worker = UploadWorker::new(
            Arc::clone(&provider),
            config.retry_policy,
            progress_callback.clone(),
        );

        let download_worker = DownloadWorker::new(
            Arc::clone(&provider),
            config.retry_policy,
            progress_callback,
        );

        let queue = TransferQueue::new(config.max_concurrent_transfers);

        Self {
            provider,
            queue,
            config,
            upload_worker,
            download_worker,
        }
    }

    /// Returns a reference to the active storage provider.
    pub fn provider(&self) -> &Arc<dyn StorageProvider + Send + Sync> {
        &self.provider
    }

    /// Returns a reference to the transfer queue.
    pub fn queue(&self) -> &TransferQueue {
        &self.queue
    }

    /// Returns the active transfer engine configuration.
    pub fn config(&self) -> &TransferEngineConfig {
        &self.config
    }

    /// Enqueues a job into the transfer engine.
    pub fn submit(&self, job: TransferJob) -> Result<CancellationToken> {
        self.queue.submit(job)
    }

    /// Cancels a registered job by its identifier.
    pub fn cancel(&self, job_id: &JobId) -> Result<()> {
        self.queue.cancel(job_id)
    }

    /// Queries a transfer job's current state.
    pub fn get_job(&self, job_id: &JobId) -> Option<TransferJob> {
        self.queue.get_job(job_id)
    }

    /// Lists all transfer jobs currently tracked by the engine.
    pub fn list_jobs(&self) -> Vec<TransferJob> {
        self.queue.list_jobs()
    }

    /// Directly streams and uploads a single transfer job from an open `Read` source.
    pub fn upload_stream(
        &self,
        job: &mut TransferJob,
        reader: &mut dyn Read,
        cancellation: &CancellationToken,
    ) -> Result<StorageReference> {
        let res = self.upload_worker.execute_upload(job, reader, cancellation);
        self.queue.finish_job(job.clone());
        res
    }

    /// Directly downloads a single transfer job from cloud storage and streams into `writer`.
    pub fn download_stream(
        &self,
        job: &mut TransferJob,
        writer: &mut dyn Write,
        cancellation: &CancellationToken,
    ) -> Result<u64> {
        let res = self
            .download_worker
            .execute_download(job, writer, cancellation);
        self.queue.finish_job(job.clone());
        res
    }

    /// Executes an upload from a temporary staging file, enforcing the Phase 6 cleanup lifecycle.
    ///
    /// Upon successful remote upload and verification, the temporary staging file is deleted.
    /// If an error or cancellation occurs, the file is cleaned up via RAII [`TempPayloadFile::drop`].
    pub fn execute_staged_upload(
        &self,
        job: &mut TransferJob,
        staging_file: TempPayloadFile,
        cancellation: &CancellationToken,
    ) -> Result<StorageReference> {
        let path = staging_file.path().to_path_buf();
        let mut file = File::open(&path)?;

        let upload_res = self.upload_stream(job, &mut file, cancellation);

        match upload_res {
            Ok(storage_ref) => {
                // Cloud upload succeeded and verified -> explicitly cleanup temporary staging payload
                staging_file.cleanup().map_err(TransferError::Storage)?;
                Ok(storage_ref)
            }
            Err(e) => {
                // Staging file will be unlinked by RAII Drop on staging_file scope exit
                Err(e)
            }
        }
    }

    /// Synchronizes a transfer job's execution state with the SQLite database catalog.
    pub fn sync_job_to_db(&self, db: &televault_db::Database, job_id: &JobId) -> Result<()> {
        if let Some(job) = self.get_job(job_id) {
            let record = job.to_db_record();
            db.upsert_transfer_job(&record)?;
        }
        Ok(())
    }
}
