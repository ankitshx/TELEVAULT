//! Transfer engine request and response DTOs.

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;

/// Snapshot of a transfer job record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TransferJobDto {
    pub job_id: String,
    pub file_id: String,
    pub chunk_id: Option<String>,
    pub direction: String,
    pub status: String,
    #[specta(type = Number)]
    pub progress: u64,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Aggregate summary of transfer engine queues.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TransferStatusDto {
    #[specta(type = Number)]
    pub active_count: usize,
    #[specta(type = Number)]
    pub queued_count: usize,
    #[specta(type = Number)]
    pub completed_count: usize,
    #[specta(type = Number)]
    pub failed_count: usize,
}

/// Request to cooperatively cancel an active operation (backup, restore, transfer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CancelOperationRequest {
    /// Identifier of the operation to cancel (e.g. JobId, ProfileId, SnapshotId).
    pub operation_id: String,
}
