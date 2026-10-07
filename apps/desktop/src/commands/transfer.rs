//! Transfer engine IPC commands and cancellation control.

use tauri::State;
use televault_core::ids::JobId;
use televault_core::TransferStatus;

use crate::dto::{CancelOperationRequest, TransferJobDto, TransferStatusDto};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

/// Lists recent transfer jobs from the SQLite catalog.
#[tauri::command]
#[specta::specta]
pub fn list_transfer_jobs(
    state: State<'_, DesktopAppState>,
    limit: Option<u32>,
) -> IpcResult<Vec<TransferJobDto>> {
    let lim = limit.unwrap_or(50).clamp(1, 500) as usize;
    let records = state
        .db
        .list_recent_transfer_jobs(lim)
        .map_err(IpcError::from)?;

    let dtos = records
        .into_iter()
        .map(|r| TransferJobDto {
            job_id: r.job_id.to_string(),
            file_id: r.file_id.to_string(),
            chunk_id: r.chunk_id.map(|c| c.to_string()),
            direction: r.direction.to_string(),
            status: r.status.to_string(),
            progress: r.progress,
            retry_count: r.retry_count,
            error_message: r.error_message,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(dtos)
}

/// Retrieves a specific transfer job record by JobId.
#[tauri::command]
#[specta::specta]
pub fn get_transfer_job(
    state: State<'_, DesktopAppState>,
    job_id: String,
) -> IpcResult<TransferJobDto> {
    let jid =
        JobId::new(&job_id).map_err(|e| IpcError::validation(format!("Invalid job ID: {e}")))?;

    let record = state
        .db
        .get_transfer_job(&jid)
        .map_err(IpcError::from)?
        .ok_or_else(|| IpcError::not_found(format!("Transfer job '{job_id}' not found")))?;

    Ok(TransferJobDto {
        job_id: record.job_id.to_string(),
        file_id: record.file_id.to_string(),
        chunk_id: record.chunk_id.map(|c| c.to_string()),
        direction: record.direction.to_string(),
        status: record.status.to_string(),
        progress: record.progress,
        retry_count: record.retry_count,
        error_message: record.error_message,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Queries aggregate counts across transfer queues.
#[tauri::command]
#[specta::specta]
pub fn get_transfer_status(state: State<'_, DesktopAppState>) -> IpcResult<TransferStatusDto> {
    let active = state
        .db
        .list_transfer_jobs_by_status(TransferStatus::Transferring)
        .map_err(IpcError::from)?
        .len();

    let queued = state
        .db
        .list_transfer_jobs_by_status(TransferStatus::Pending)
        .map_err(IpcError::from)?
        .len();

    let completed = state
        .db
        .list_transfer_jobs_by_status(TransferStatus::Completed)
        .map_err(IpcError::from)?
        .len();

    let failed = state
        .db
        .list_transfer_jobs_by_status(TransferStatus::Failed)
        .map_err(IpcError::from)?
        .len();

    Ok(TransferStatusDto {
        active_count: active,
        queued_count: queued,
        completed_count: completed,
        failed_count: failed,
    })
}

/// Cooperatively cancels an active operation (backup, restore, transfer) via its registered cancellation token.
#[tauri::command]
#[specta::specta]
pub fn cancel_operation(
    state: State<'_, DesktopAppState>,
    request: CancelOperationRequest,
) -> IpcResult<bool> {
    let cancelled = state.cancel_operation(&request.operation_id);
    if cancelled {
        Ok(true)
    } else {
        // Even if not actively running in registry, attempt to cancel any matching transfer job in DB
        if let Ok(jid) = JobId::new(&request.operation_id) {
            let _ = state.db.update_transfer_job_status(
                &jid,
                TransferStatus::Cancelled,
                Some("Cancelled by user"),
                &std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    .to_string(),
            );
        }
        Ok(false)
    }
}
