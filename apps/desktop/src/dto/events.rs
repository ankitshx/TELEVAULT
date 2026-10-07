//! IPC event payload contracts for Tauri event emissions.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Throttled event payload emitted during active transfers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct TransferProgressEvent {
    pub job_id: String,
    pub file_id: String,
    pub direction: String,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub stage: String,
}

/// Throttled event payload emitted during backup operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct BackupProgressEvent {
    pub profile_id: String,
    pub snapshot_id: String,
    pub current_file: String,
    pub processed_files: u64,
    pub total_files: u64,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub stage: String,
}

/// Throttled event payload emitted during restore operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct RestoreProgressEvent {
    pub file_id: String,
    pub target_path: String,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub stage: String,
}
