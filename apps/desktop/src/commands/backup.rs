//! Backup management IPC commands.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use televault_backup::profile::BackupProfile;
use televault_core::ids::{ProfileId, SnapshotId};
use televault_crypto::kdf::{derive_key, KdfParams};
use televault_crypto::key::Salt;
use televault_crypto::policy::EncryptionPolicy;

use crate::dto::{
    BackupProfileDto, BackupSummaryDto, CreateProfileRequest, SnapshotDto, SnapshotFileDto,
    StartBackupRequest, UpdateProfileRequest,
};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

/// Lists all configured backup profiles from the database.
#[tauri::command]
#[specta::specta]
pub fn list_backup_profiles(state: State<'_, DesktopAppState>) -> IpcResult<Vec<BackupProfileDto>> {
    let records = state.db.list_profiles().map_err(IpcError::from)?;
    let dtos = records
        .into_iter()
        .map(|r| BackupProfileDto {
            profile_id: r.profile_id.to_string(),
            name: r.name,
            description: r.description,
            source_path: r.source_path,
            enabled: r.enabled,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();
    Ok(dtos)
}

/// Retrieves a specific backup profile by ProfileId.
#[tauri::command]
#[specta::specta]
pub fn get_backup_profile(
    state: State<'_, DesktopAppState>,
    profile_id: String,
) -> IpcResult<BackupProfileDto> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let record = state
        .db
        .get_profile(&pid)
        .map_err(IpcError::from)?
        .ok_or_else(|| IpcError::not_found(format!("Profile '{profile_id}' not found")))?;

    Ok(BackupProfileDto {
        profile_id: record.profile_id.to_string(),
        name: record.name,
        description: record.description,
        source_path: record.source_path,
        enabled: record.enabled,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Creates and persists a new backup profile.
#[tauri::command]
#[specta::specta]
pub fn create_backup_profile(
    state: State<'_, DesktopAppState>,
    request: CreateProfileRequest,
) -> IpcResult<BackupProfileDto> {
    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let source = PathBuf::from(&request.source_path);
    if !source.exists() || !source.is_dir() {
        return Err(IpcError::validation(format!(
            "Source path '{}' does not exist or is not a directory",
            request.source_path
        )));
    }

    let now = format_rfc3339();
    let record = televault_db::ProfileRecord {
        profile_id: pid,
        name: request.name,
        description: request.description,
        source_path: request.source_path,
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
    };
    state.db.create_profile(&record).map_err(IpcError::from)?;

    Ok(BackupProfileDto {
        profile_id: record.profile_id.to_string(),
        name: record.name,
        description: record.description,
        source_path: record.source_path,
        enabled: record.enabled,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Updates an existing backup profile.
#[tauri::command]
#[specta::specta]
pub fn update_backup_profile(
    state: State<'_, DesktopAppState>,
    request: UpdateProfileRequest,
) -> IpcResult<BackupProfileDto> {
    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let mut record = state
        .db
        .get_profile(&pid)
        .map_err(IpcError::from)?
        .ok_or_else(|| {
            IpcError::not_found(format!("Profile '{}' not found", request.profile_id))
        })?;

    if let Some(name) = request.name {
        record.name = name;
    }
    if let Some(desc) = request.description {
        record.description = Some(desc);
    }
    if let Some(src) = request.source_path {
        let p = PathBuf::from(&src);
        if !p.exists() || !p.is_dir() {
            return Err(IpcError::validation(format!(
                "Source path '{src}' is not a valid directory"
            )));
        }
        record.source_path = src;
    }
    if let Some(enabled) = request.enabled {
        record.enabled = enabled;
    }

    record.updated_at = format_rfc3339();

    state.db.update_profile(&record).map_err(IpcError::from)?;

    Ok(BackupProfileDto {
        profile_id: record.profile_id.to_string(),
        name: record.name,
        description: record.description,
        source_path: record.source_path,
        enabled: record.enabled,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// Deletes a backup profile.
#[tauri::command]
#[specta::specta]
pub fn delete_backup_profile(
    state: State<'_, DesktopAppState>,
    profile_id: String,
) -> IpcResult<bool> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    state.db.delete_profile(&pid).map_err(IpcError::from)?;
    Ok(true)
}

/// Initiates a point-in-time snapshot backup for the specified profile.
///
/// Execution runs asynchronously offloaded to tokio worker threads to preserve UI responsiveness.
#[tauri::command]
#[specta::specta]
pub async fn start_backup(
    state: State<'_, DesktopAppState>,
    request: StartBackupRequest,
) -> IpcResult<BackupSummaryDto> {
    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let record = state
        .db
        .get_profile(&pid)
        .map_err(IpcError::from)?
        .ok_or_else(|| {
            IpcError::not_found(format!("Profile '{}' not found", request.profile_id))
        })?;

    let encryption_policy = if let Some(passphrase) = request.passphrase {
        if !passphrase.is_empty() {
            let salt_hash = Sha256::digest(format!("salt:{}", record.profile_id).as_bytes());
            let salt = Salt::from_slice(&salt_hash[..32])
                .map_err(|e| IpcError::internal(e.to_string()))?;
            let secret_key = derive_key(&passphrase, &salt, &KdfParams::default())
                .map_err(|e| IpcError::internal(format!("Key derivation failed: {e}")))?;
            EncryptionPolicy::Enabled(secret_key)
        } else {
            EncryptionPolicy::Disabled
        }
    } else {
        EncryptionPolicy::Disabled
    };

    let profile = BackupProfile::from_db_record(&record, None, encryption_policy);

    let cancel_token = state.register_cancellation(&request.profile_id);
    let backup_engine = Arc::clone(&state.backup_engine);
    let op_id = request.profile_id.clone();

    let res = tokio::task::spawn_blocking(move || {
        let plan = backup_engine.plan_backup(&profile)?;
        backup_engine.execute_backup(plan, &cancel_token)
    })
    .await
    .map_err(|e| IpcError::internal(format!("Task spawn failed: {e}")))?
    .map_err(IpcError::from);

    state.unregister_cancellation(&op_id);

    let summary = res?;
    Ok(BackupSummaryDto {
        snapshot_id: summary.snapshot_id.to_string(),
        profile_id: summary.profile_id.to_string(),
        status: summary.status.to_string(),
        new_files: summary.new_files,
        modified_files: summary.modified_files,
        unchanged_files: summary.unchanged_files,
        deleted_files: summary.deleted_files,
        transferred_chunks: summary.transferred_chunks,
        transferred_bytes: summary.transferred_bytes,
        reused_bytes: summary.reused_bytes,
        elapsed_ms: summary.elapsed_ms,
        error_message: summary.error_message,
    })
}

/// Lists all historical snapshots for a backup profile.
#[tauri::command]
#[specta::specta]
pub fn list_snapshots(
    state: State<'_, DesktopAppState>,
    profile_id: String,
) -> IpcResult<Vec<SnapshotDto>> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let records = state
        .db
        .list_snapshots_by_profile(&pid)
        .map_err(IpcError::from)?;
    let dtos = records
        .into_iter()
        .map(|r| SnapshotDto {
            snapshot_id: r.snapshot_id.to_string(),
            profile_id: r.profile_id.to_string(),
            status: r.status.to_string(),
            metadata: r.metadata,
            created_at: r.created_at,
        })
        .collect();

    Ok(dtos)
}

/// Lists all files and version records belonging to a historical snapshot.
#[tauri::command]
#[specta::specta]
pub fn get_snapshot_files(
    state: State<'_, DesktopAppState>,
    snapshot_id: String,
) -> IpcResult<Vec<SnapshotFileDto>> {
    let sid = SnapshotId::new(&snapshot_id)
        .map_err(|e| IpcError::validation(format!("Invalid snapshot ID: {e}")))?;

    let records = state
        .backup_engine
        .snapshot_manager()
        .list_snapshot_files(&sid)
        .map_err(IpcError::from)?;

    let dtos = records
        .into_iter()
        .map(|(file, version)| SnapshotFileDto {
            file_id: file.file_id.to_string(),
            relative_path: file.relative_path,
            size_bytes: file.original_size,
            status: version.status,
            mime_type: file.mime_type,
        })
        .collect();

    Ok(dtos)
}

fn format_rfc3339() -> String {
    // Generate ISO 8601 / RFC 3339 UTC string
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple deterministic formatting (e.g. 2026-10-07T12:00:00Z format)
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;
    // Approximating calendar year/month/day since epoch
    let mut y = 1970;
    let mut rem_days = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if rem_days < days_in_year {
            let month_days = [
                31,
                if leap { 29 } else { 28 },
                31,
                30,
                31,
                30,
                31,
                31,
                30,
                31,
                30,
                31,
            ];
            for (m, &d) in (1..).zip(month_days.iter()) {
                if rem_days < d {
                    return format!(
                        "{y:04}-{m:02}-{:02}T{hours:02}:{minutes:02}:{seconds:02}Z",
                        rem_days + 1
                    );
                }
                rem_days -= d;
            }
            return format!("{y:04}-12-31T{hours:02}:{minutes:02}:{seconds:02}Z");
        }
        rem_days -= days_in_year;
        y += 1;
    }
}
