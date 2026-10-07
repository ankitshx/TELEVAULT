//! Thread-safe transfer queue with bounded concurrency and cancellation tracking.

use crate::cancellation::CancellationToken;
use crate::error::{Result, TransferError};
use crate::job::TransferJob;
use crate::state::TransferState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use televault_core::ids::JobId;

/// Internal state representation for the transfer queue.
#[derive(Debug)]
struct QueueInner {
    pending: VecDeque<JobId>,
    jobs: HashMap<JobId, TransferJob>,
    active: HashSet<JobId>,
    tokens: HashMap<JobId, CancellationToken>,
    max_concurrency: usize,
}

/// Thread-safe in-memory transfer queue managing job scheduling, concurrency limits, and status queries.
#[derive(Debug, Clone)]
pub struct TransferQueue {
    inner: Arc<Mutex<QueueInner>>,
}

impl TransferQueue {
    /// Creates a new [`TransferQueue`] with the specified maximum concurrent transfer operations.
    pub fn new(max_concurrency: usize) -> Self {
        let concurrency = max_concurrency.max(1);
        Self {
            inner: Arc::new(Mutex::new(QueueInner {
                pending: VecDeque::new(),
                jobs: HashMap::new(),
                active: HashSet::new(),
                tokens: HashMap::new(),
                max_concurrency: concurrency,
            })),
        }
    }

    /// Enqueues a new transfer job, returning its associated cooperative cancellation token.
    pub fn submit(&self, job: TransferJob) -> Result<CancellationToken> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| TransferError::Concurrency(format!("Queue lock poisoned: {e}")))?;

        if guard.jobs.contains_key(&job.job_id) {
            return Err(TransferError::InvalidJob(format!(
                "Job '{}' is already registered in queue",
                job.job_id.as_str()
            )));
        }

        let token = CancellationToken::new();
        guard.pending.push_back(job.job_id.clone());
        guard.tokens.insert(job.job_id.clone(), token.clone());
        guard.jobs.insert(job.job_id.clone(), job);

        Ok(token)
    }

    /// Cancels an active or pending job by job identifier.
    pub fn cancel(&self, job_id: &JobId) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| TransferError::Concurrency(format!("Queue lock poisoned: {e}")))?;

        if let Some(token) = guard.tokens.get(job_id) {
            token.cancel();
        }

        if let Some(job) = guard.jobs.get_mut(job_id) {
            let _ = job.transition(TransferState::Cancelled);
        }

        // If it was still in pending, remove it from queue
        guard.pending.retain(|id| id != job_id);

        Ok(())
    }

    /// Retrieves an execution snapshot of a specific transfer job.
    pub fn get_job(&self, job_id: &JobId) -> Option<TransferJob> {
        let guard = self.inner.lock().ok()?;
        guard.jobs.get(job_id).cloned()
    }

    /// Lists all transfer jobs currently tracked by the queue.
    pub fn list_jobs(&self) -> Vec<TransferJob> {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        guard.jobs.values().cloned().collect()
    }

    /// Returns the number of jobs currently queued and awaiting an execution slot.
    pub fn pending_count(&self) -> usize {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return 0,
        };
        guard.pending.len()
    }

    /// Returns the number of jobs currently active across worker slots.
    pub fn active_count(&self) -> usize {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return 0,
        };
        guard.active.len()
    }

    /// Attempts to acquire the next pending job if concurrency capacity is available.
    pub fn acquire_next_job(&self) -> Option<(TransferJob, CancellationToken)> {
        let mut guard = self.inner.lock().ok()?;

        if guard.active.len() >= guard.max_concurrency {
            return None;
        }

        while let Some(job_id) = guard.pending.pop_front() {
            if let Some(token) = guard.tokens.get(&job_id).cloned() {
                if token.is_cancelled() {
                    continue;
                }
                if let Some(job) = guard.jobs.get(&job_id).cloned() {
                    guard.active.insert(job_id);
                    return Some((job, token));
                }
            }
        }

        None
    }

    /// Reports that an active job has concluded, updating its status and freeing a concurrency slot.
    pub fn finish_job(&self, job: TransferJob) {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        guard.active.remove(&job.job_id);
        guard.jobs.insert(job.job_id.clone(), job);
    }

    /// Re-queues a failed or retried job for future worker execution.
    pub fn requeue_job(&self, job: TransferJob) {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        guard.active.remove(&job.job_id);
        guard.jobs.insert(job.job_id.clone(), job.clone());
        guard.pending.push_back(job.job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use televault_core::ids::{ChunkId, FileId};

    #[test]
    fn test_queue_concurrency_boundary() {
        let queue = TransferQueue::new(2);

        let j1 = TransferJob::new_upload(crate::job::UploadJobParams {
            job_id: JobId::new("j-1").unwrap(),
            file_id: FileId::new("f-1").unwrap(),
            chunk_id: ChunkId::new("c-1").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes: 100,
            expected_sha256: None,
            max_retries: 3,
        })
        .unwrap();

        let j2 = TransferJob::new_upload(crate::job::UploadJobParams {
            job_id: JobId::new("j-2").unwrap(),
            file_id: FileId::new("f-2").unwrap(),
            chunk_id: ChunkId::new("c-2").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes: 200,
            expected_sha256: None,
            max_retries: 3,
        })
        .unwrap();

        let j3 = TransferJob::new_upload(crate::job::UploadJobParams {
            job_id: JobId::new("j-3").unwrap(),
            file_id: FileId::new("f-3").unwrap(),
            chunk_id: ChunkId::new("c-3").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes: 300,
            expected_sha256: None,
            max_retries: 3,
        })
        .unwrap();

        queue.submit(j1).unwrap();
        queue.submit(j2).unwrap();
        queue.submit(j3).unwrap();

        assert_eq!(queue.pending_count(), 3);
        assert_eq!(queue.active_count(), 0);

        // Slot 1
        let acq1 = queue.acquire_next_job();
        assert!(acq1.is_some());
        assert_eq!(queue.active_count(), 1);

        // Slot 2
        let acq2 = queue.acquire_next_job();
        assert!(acq2.is_some());
        assert_eq!(queue.active_count(), 2);

        // Slot 3 must be blocked because max_concurrency is 2
        let acq3 = queue.acquire_next_job();
        assert!(acq3.is_none());

        // Finish slot 1 -> now slot 3 can be acquired
        let (mut finished_job, _) = acq1.unwrap();
        finished_job.state = TransferState::Completed;
        queue.finish_job(finished_job);
        assert_eq!(queue.active_count(), 1);

        let acq3_retry = queue.acquire_next_job();
        assert!(acq3_retry.is_some());
        assert_eq!(queue.active_count(), 2);
    }

    #[test]
    fn test_queue_cancellation() {
        let queue = TransferQueue::new(2);
        let j1 = TransferJob::new_upload(crate::job::UploadJobParams {
            job_id: JobId::new("j-cancel").unwrap(),
            file_id: FileId::new("f-cancel").unwrap(),
            chunk_id: ChunkId::new("c-cancel").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            source_path: None,
            expected_size_bytes: 100,
            expected_sha256: None,
            max_retries: 3,
        })
        .unwrap();

        let token = queue.submit(j1).unwrap();
        assert!(!token.is_cancelled());

        queue.cancel(&JobId::new("j-cancel").unwrap()).unwrap();
        assert!(token.is_cancelled());

        let job = queue.get_job(&JobId::new("j-cancel").unwrap()).unwrap();
        assert_eq!(job.state, TransferState::Cancelled);
    }
}
