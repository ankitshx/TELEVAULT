//! Strongly typed progress reporting and streaming throttled counters.

use crate::state::TransferState;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::Arc;
use televault_core::ids::{ChunkId, FileId, JobId};
use televault_core::TransferDirection;

/// Minimum delta in transferred bytes between emitting intermediate progress events (256 KiB).
pub const PROGRESS_THROTTLE_BYTE_DELTA: u64 = 256 * 1024;

/// Strongly typed snapshot of transfer execution progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferProgress {
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
    /// Direction of transfer.
    pub direction: TransferDirection,
    /// Bytes successfully transferred so far.
    pub bytes_transferred: u64,
    /// Expected total size in bytes.
    pub total_bytes: u64,
    /// Current execution state.
    pub state: TransferState,
}

impl TransferProgress {
    /// Calculates current completion percentage as a float in the range [0.0, 100.0].
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            if self.state == TransferState::Completed {
                100.0
            } else {
                0.0
            }
        } else {
            ((self.bytes_transferred as f64 / self.total_bytes as f64) * 100.0).clamp(0.0, 100.0)
        }
    }
}

/// Thread-safe progress event callback.
pub type ProgressCallback = Arc<dyn Fn(TransferProgress) + Send + Sync>;

/// Contextual metadata for tracking transfer progress.
#[derive(Debug, Clone)]
pub struct ProgressContext {
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
    /// Expected total size in bytes.
    pub total_bytes: u64,
}

/// Helper stream updating progress counts and invoking throttled callbacks on read.
pub struct ProgressReader<'a> {
    inner: &'a mut dyn Read,
    ctx: ProgressContext,
    bytes_read: u64,
    last_reported_bytes: u64,
    callback: Option<ProgressCallback>,
}

impl<'a> ProgressReader<'a> {
    /// Creates a new progress reader wrapping an existing `Read` source.
    pub fn new(
        inner: &'a mut dyn Read,
        ctx: ProgressContext,
        callback: Option<ProgressCallback>,
    ) -> Self {
        Self {
            inner,
            ctx,
            bytes_read: 0,
            last_reported_bytes: 0,
            callback,
        }
    }

    /// Emits a progress update if the throttle threshold is exceeded or forced.
    fn check_emit_progress(&mut self, force: bool) {
        if let Some(ref cb) = self.callback {
            let delta = self.bytes_read.saturating_sub(self.last_reported_bytes);
            if force || delta >= PROGRESS_THROTTLE_BYTE_DELTA {
                self.last_reported_bytes = self.bytes_read;
                cb(TransferProgress {
                    job_id: self.ctx.job_id.clone(),
                    file_id: self.ctx.file_id.clone(),
                    chunk_id: self.ctx.chunk_id.clone(),
                    chunk_index: self.ctx.chunk_index,
                    total_chunks: self.ctx.total_chunks,
                    direction: TransferDirection::Upload,
                    bytes_transferred: self.bytes_read,
                    total_bytes: self.ctx.total_bytes,
                    state: TransferState::Transferring,
                });
            }
        }
    }

    /// Total bytes read through this stream.
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

impl<'a> Read for ProgressReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.bytes_read += n as u64;
            self.check_emit_progress(false);
        } else {
            // End of stream -> force final progress report
            self.check_emit_progress(true);
        }
        Ok(n)
    }
}

/// Helper stream updating progress counts and invoking throttled callbacks on write.
pub struct ProgressWriter<'a> {
    inner: &'a mut dyn Write,
    ctx: ProgressContext,
    bytes_written: u64,
    last_reported_bytes: u64,
    callback: Option<ProgressCallback>,
}

impl<'a> ProgressWriter<'a> {
    /// Creates a new progress writer wrapping an existing `Write` destination.
    pub fn new(
        inner: &'a mut dyn Write,
        ctx: ProgressContext,
        callback: Option<ProgressCallback>,
    ) -> Self {
        Self {
            inner,
            ctx,
            bytes_written: 0,
            last_reported_bytes: 0,
            callback,
        }
    }

    /// Emits a progress update if the throttle threshold is exceeded or forced.
    pub fn check_emit_progress(&mut self, force: bool) {
        if let Some(ref cb) = self.callback {
            let delta = self.bytes_written.saturating_sub(self.last_reported_bytes);
            if force || delta >= PROGRESS_THROTTLE_BYTE_DELTA {
                self.last_reported_bytes = self.bytes_written;
                cb(TransferProgress {
                    job_id: self.ctx.job_id.clone(),
                    file_id: self.ctx.file_id.clone(),
                    chunk_id: self.ctx.chunk_id.clone(),
                    chunk_index: self.ctx.chunk_index,
                    total_chunks: self.ctx.total_chunks,
                    direction: TransferDirection::Download,
                    bytes_transferred: self.bytes_written,
                    total_bytes: self.ctx.total_bytes,
                    state: TransferState::Transferring,
                });
            }
        }
    }

    /// Total bytes written through this stream.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

impl<'a> Write for ProgressWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        if n > 0 {
            self.bytes_written += n as u64;
            self.check_emit_progress(false);
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_progress_percentage_calculation() {
        let progress = TransferProgress {
            job_id: JobId::new("job-1").unwrap(),
            file_id: FileId::new("file-1").unwrap(),
            chunk_id: ChunkId::new("chunk-1").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            direction: TransferDirection::Upload,
            bytes_transferred: 500,
            total_bytes: 1000,
            state: TransferState::Transferring,
        };
        assert!((progress.percentage() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_progress_reader_throttling() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let cb: ProgressCallback = Arc::new(move |_| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // 300 KiB payload (> PROGRESS_THROTTLE_BYTE_DELTA of 256 KiB)
        let payload = vec![0x42u8; 300 * 1024];
        let mut slice = &payload[..];

        let ctx = ProgressContext {
            job_id: JobId::new("job-2").unwrap(),
            file_id: FileId::new("file-2").unwrap(),
            chunk_id: ChunkId::new("chunk-2").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            total_bytes: payload.len() as u64,
        };

        let mut reader = ProgressReader::new(&mut slice, ctx, Some(cb));

        let mut buf = [0u8; 64 * 1024];
        while reader.read(&mut buf).unwrap() > 0 {}

        assert!(call_count.load(Ordering::SeqCst) >= 2);
    }
}
