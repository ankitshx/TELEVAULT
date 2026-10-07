//! Backup orchestration, snapshot management, and retention policies for TELEVAULT.

#![deny(missing_docs)]

pub mod change_detector;
pub mod checker;
pub mod engine;
pub mod error;
pub mod pipeline;
pub mod plan;
pub mod profile;
pub mod scanner;
pub mod snapshot;

pub use change_detector::{ChangeDetector, ChangeKind, ChangeSet, FileChange};
pub use checker::{BackupChecker, FileBackupStatus};
pub use engine::BackupEngine;
pub use error::{BackupError, Result};
pub use pipeline::{PayloadPipeline, ProcessedChunk, ProcessedFile, ProcessingOptions};
pub use plan::{BackupPlan, BackupSummary};
pub use profile::{BackupProfile, ProfileConfig};
pub use scanner::{FileScanner, ScanResult, ScannedFile};
pub use snapshot::SnapshotManager;

pub use televault_core as core;
pub use televault_manifest as manifest;

/// Returns the backup engine status string.
pub fn backup_engine_status() -> &'static str {
    "idle"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_engine_status() {
        assert_eq!(backup_engine_status(), "idle");
    }
}
