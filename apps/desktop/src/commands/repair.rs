//! Remote backup repair and chunk recovery IPC commands.

use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;
use televault_core::ids::{FileId, ProfileId, SnapshotId};
use televault_crypto::kdf::{derive_key, KdfParams};
use televault_crypto::key::Salt;
use televault_crypto::policy::EncryptionPolicy;

use crate::dto::{
    PreviewRepairRequest, RepairExecutionResultDto, RepairFileRequest, RepairHistoryRecordDto,
    RepairPreviewDto, RepairSnapshotRequest,
};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

fn derive_encryption_policy(
    profile_id: &ProfileId,
    passphrase: Option<&str>,
) -> IpcResult<EncryptionPolicy> {
    if let Some(pass) = passphrase {
        if !pass.is_empty() {
            let salt_hash = Sha256::digest(format!("salt:{profile_id}").as_bytes());
            let salt = Salt::from_slice(&salt_hash[..32])
                .map_err(|e| IpcError::internal(e.to_string()))?;
            let secret_key = derive_key(pass, &salt, &KdfParams::default())
                .map_err(|e| IpcError::internal(format!("Key derivation failed: {e}")))?;
            return Ok(EncryptionPolicy::Enabled(secret_key));
        }
    }
    Ok(EncryptionPolicy::Disabled)
}

/// Generates a preview analysis of needed repairs without mutating remote or local state.
#[tauri::command]
#[specta::specta]
pub async fn preview_repair(
    state: State<'_, DesktopAppState>,
    request: PreviewRepairRequest,
) -> IpcResult<RepairPreviewDto> {
    state.check_auth_gate().await?;

    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let enc_policy = derive_encryption_policy(&pid, request.passphrase.as_deref())?;
    let engine = state
        .repair_engine
        .as_ref()
        .clone()
        .with_encryption_policy(Arc::new(enc_policy));

    let op_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("preview-repair-{}-{}", pid, request.target_id));
    let cancel = state.register_cancellation(&op_id);

    let target_type = request.target_type.clone();
    let target_id = request.target_id.clone();

    let result = tokio::task::spawn_blocking(move || match target_type.as_str() {
        "file" => {
            let fid = FileId::new(&target_id)
                .map_err(|e| IpcError::validation(format!("Invalid file ID: {e}")))?;
            engine
                .preview_file_repair(&pid, &fid, &cancel)
                .map_err(IpcError::from)
        }
        "snapshot" => {
            let sid = SnapshotId::new(&target_id)
                .map_err(|e| IpcError::validation(format!("Invalid snapshot ID: {e}")))?;
            engine
                .preview_snapshot_repair(&pid, &sid, &cancel)
                .map_err(IpcError::from)
        }
        other => Err(IpcError::validation(format!(
            "Unsupported target type '{other}', expected 'file' or 'snapshot'"
        ))),
    })
    .await
    .map_err(|e| IpcError::internal(format!("Worker thread failed: {e}")))?;

    state.unregister_cancellation(&op_id);
    let preview = result?;
    Ok(preview.into())
}

/// Executes remote repair for an individual logical file.
#[tauri::command]
#[specta::specta]
pub async fn repair_file(
    state: State<'_, DesktopAppState>,
    request: RepairFileRequest,
) -> IpcResult<RepairExecutionResultDto> {
    state.check_auth_gate().await?;

    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;
    let fid = FileId::new(&request.file_id)
        .map_err(|e| IpcError::validation(format!("Invalid file ID: {e}")))?;

    // Mutual exclusion: ensure no concurrent scheduled or manual backup runs on same profile
    let _guard = state
        .scheduler_service
        .execution_guard()
        .try_acquire(&pid)
        .map_err(|e| IpcError::conflict(format!("Profile is currently busy: {e}")))?;

    let enc_policy = derive_encryption_policy(&pid, request.passphrase.as_deref())?;
    let engine = state
        .repair_engine
        .as_ref()
        .clone()
        .with_encryption_policy(Arc::new(enc_policy));

    let op_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("repair-file-{}", fid));
    let cancel = state.register_cancellation(&op_id);
    let dry_run = request.dry_run.unwrap_or(false);

    let result = tokio::task::spawn_blocking(move || {
        engine
            .repair_file(&pid, &fid, dry_run, &cancel)
            .map_err(IpcError::from)
    })
    .await
    .map_err(|e| IpcError::internal(format!("Worker thread failed: {e}")))?;

    state.unregister_cancellation(&op_id);
    let exec_res = result?;
    Ok(exec_res.into())
}

/// Executes remote repair across all files in a backup snapshot.
#[tauri::command]
#[specta::specta]
pub async fn repair_snapshot(
    state: State<'_, DesktopAppState>,
    request: RepairSnapshotRequest,
) -> IpcResult<RepairExecutionResultDto> {
    state.check_auth_gate().await?;

    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;
    let sid = SnapshotId::new(&request.snapshot_id)
        .map_err(|e| IpcError::validation(format!("Invalid snapshot ID: {e}")))?;

    // Mutual exclusion: ensure no concurrent scheduled or manual backup runs on same profile
    let _guard = state
        .scheduler_service
        .execution_guard()
        .try_acquire(&pid)
        .map_err(|e| IpcError::conflict(format!("Profile is currently busy: {e}")))?;

    let enc_policy = derive_encryption_policy(&pid, request.passphrase.as_deref())?;
    let engine = state
        .repair_engine
        .as_ref()
        .clone()
        .with_encryption_policy(Arc::new(enc_policy));

    let op_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("repair-snapshot-{}", sid));
    let cancel = state.register_cancellation(&op_id);
    let dry_run = request.dry_run.unwrap_or(false);

    let result = tokio::task::spawn_blocking(move || {
        engine
            .repair_snapshot(&pid, &sid, dry_run, &cancel)
            .map_err(IpcError::from)
    })
    .await
    .map_err(|e| IpcError::internal(format!("Worker thread failed: {e}")))?;

    state.unregister_cancellation(&op_id);
    let exec_res = result?;
    Ok(exec_res.into())
}

/// Lists remote repair history records for a profile, ordered newest first.
#[tauri::command]
#[specta::specta]
pub fn get_repair_history(
    state: State<'_, DesktopAppState>,
    profile_id: String,
    limit: Option<u32>,
) -> IpcResult<Vec<RepairHistoryRecordDto>> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let records = state
        .db
        .list_repair_history(&pid, limit.unwrap_or(50) as usize)
        .map_err(IpcError::from)?;

    let dtos = records.into_iter().map(Into::into).collect();
    Ok(dtos)
}
