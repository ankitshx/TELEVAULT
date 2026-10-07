//! Transfer engine, upload/download workers, queue, and retry state machine for TELEVAULT.

#![deny(missing_docs)]

pub mod cancellation;
pub mod engine;
pub mod error;
pub mod job;
pub mod progress;
pub mod queue;
pub mod retry;
pub mod state;
pub mod worker;

pub use cancellation::CancellationToken;
pub use engine::{TransferEngine, TransferEngineConfig};
pub use error::{Result, TransferError};
pub use job::TransferJob;
pub use progress::{ProgressCallback, TransferProgress};
pub use queue::TransferQueue;
pub use retry::RetryPolicy;
pub use state::TransferState;
pub use worker::{DownloadWorker, UploadWorker};

pub use televault_core as core;
pub use televault_db as db;
pub use televault_manifest as manifest;
pub use televault_storage as storage;

/// Returns the transfer subsystem status string.
pub fn transfer_status() -> &'static str {
    "ready"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_status() {
        assert_eq!(transfer_status(), "ready");
    }
}
