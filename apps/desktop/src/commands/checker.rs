//! Backup checker and version query IPC commands.

use tauri::State;
use televault_backup::profile::BackupProfile;
use televault_core::ids::{FileId, ProfileId};
use televault_crypto::policy::EncryptionPolicy;

use crate::dto::{CheckFileStatusRequest, FileStatusDto, FileVersionDto};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

/// Checks whether a specific relative path within a profile has been backed up.
#[tauri::command]
#[specta::specta]
pub fn check_file_status(
    state: State<'_, DesktopAppState>,
    request: CheckFileStatusRequest,
) -> IpcResult<FileStatusDto> {
    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let status = state
        .checker
        .check_file_status(&pid, &request.relative_path)
        .map_err(IpcError::from)?;

    Ok(FileStatusDto {
        profile_id: request.profile_id,
        relative_path: request.relative_path,
        is_backed_up: status.is_backed_up,
        file_id: status.file_id.map(|f| f.to_string()),
        latest_version_id: status.latest_version_id.map(|v| v.to_string()),
        incremental_needed: status.is_modified_locally || !status.is_backed_up,
    })
}

/// Checks if an incremental backup is needed for the given profile by comparing local scans against last snapshot.
#[tauri::command]
#[specta::specta]
pub async fn is_incremental_backup_needed(
    state: State<'_, DesktopAppState>,
    profile_id: String,
) -> IpcResult<bool> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let record = state
        .db
        .get_profile(&pid)
        .map_err(IpcError::from)?
        .ok_or_else(|| IpcError::not_found(format!("Profile '{profile_id}' not found")))?;

    let profile = BackupProfile::from_db_record(&record, None, EncryptionPolicy::Disabled);

    let checker = std::sync::Arc::clone(&state.checker);
    let needed =
        tokio::task::spawn_blocking(move || checker.is_incremental_backup_needed(&profile))
            .await
            .map_err(|e| IpcError::internal(format!("Worker failed: {e}")))?
            .map_err(IpcError::from)?;

    Ok(needed)
}

/// Lists all historical backed-up versions for a specific FileId.
#[tauri::command]
#[specta::specta]
pub fn get_file_versions(
    state: State<'_, DesktopAppState>,
    file_id: String,
) -> IpcResult<Vec<FileVersionDto>> {
    let fid =
        FileId::new(&file_id).map_err(|e| IpcError::validation(format!("Invalid file ID: {e}")))?;

    let records = state
        .db
        .list_versions_by_file(&fid)
        .map_err(IpcError::from)?;
    let dtos = records
        .into_iter()
        .map(|r| FileVersionDto {
            version_id: r.version_id.to_string(),
            file_id: r.file_id.to_string(),
            snapshot_id: r.snapshot_id.to_string(),
            manifest_id: r.manifest_id,
            status: r.status,
            created_at: r.created_at,
        })
        .collect();

    Ok(dtos)
}
