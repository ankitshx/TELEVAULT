//! Remote backup verification and integrity-audit IPC commands.

use std::sync::Arc;
use tauri::State;
use televault_core::ids::{FileId, ProfileId, SnapshotId};
use televault_integrity::types::VerificationOptions;

use crate::dto::{VerificationHistoryRecordDto, VerificationResultDto, VerifyTargetRequest};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

fn parse_options(request: &VerifyTargetRequest) -> VerificationOptions {
    let mut options = match request.level.unwrap_or(4) {
        1 => VerificationOptions::metadata_only(),
        2 => VerificationOptions::remote_availability(),
        3 => VerificationOptions::full_integrity(),
        _ => VerificationOptions::restore_readiness(),
    };
    if let Some(f) = request.full_hash_check {
        options.full_hash_check = f;
    }
    if let Some(d) = request.decrypt_check {
        options.decrypt_check = d;
    }
    options
}

/// Verifies a tracked file in a profile by auditing its latest backup manifest and chunks.
#[tauri::command]
#[specta::specta]
pub async fn verify_file_backup(
    state: State<'_, DesktopAppState>,
    request: VerifyTargetRequest,
) -> IpcResult<VerificationResultDto> {
    state.check_auth_gate().await?;

    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let fid_str = request
        .target_id
        .as_deref()
        .ok_or_else(|| IpcError::validation("Target file ID is required"))?;
    let fid =
        FileId::new(fid_str).map_err(|e| IpcError::validation(format!("Invalid file ID: {e}")))?;

    let options = parse_options(&request);
    let op_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("verify-file-{}", fid));
    let cancel = state.register_cancellation(&op_id);
    let engine = Arc::clone(&state.verification_engine);

    let result =
        tokio::task::spawn_blocking(move || engine.verify_file(&pid, &fid, &options, &cancel))
            .await
            .map_err(|e| IpcError::internal(format!("Worker thread failed: {e}")))?
            .map_err(IpcError::from)?;

    state.unregister_cancellation(&op_id);
    Ok(result.into())
}

/// Verifies a specific backup manifest record by its manifest identifier.
#[tauri::command]
#[specta::specta]
pub async fn verify_manifest(
    state: State<'_, DesktopAppState>,
    request: VerifyTargetRequest,
) -> IpcResult<VerificationResultDto> {
    state.check_auth_gate().await?;

    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let mid = request
        .target_id
        .as_deref()
        .ok_or_else(|| IpcError::validation("Target manifest ID is required"))?;

    let manifest = state
        .db
        .get_manifest(mid)
        .map_err(IpcError::from)?
        .ok_or_else(|| IpcError::not_found(format!("Manifest '{mid}' not found in catalog")))?;

    let options = parse_options(&request);
    let op_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("verify-manifest-{}", mid));
    let cancel = state.register_cancellation(&op_id);
    let engine = Arc::clone(&state.verification_engine);

    let result = tokio::task::spawn_blocking(move || {
        engine.verify_manifest(&pid, &manifest, &options, &cancel)
    })
    .await
    .map_err(|e| IpcError::internal(format!("Worker thread failed: {e}")))?
    .map_err(IpcError::from)?;

    state.unregister_cancellation(&op_id);
    Ok(result.into())
}

/// Verifies all file manifests comprising an entire snapshot.
#[tauri::command]
#[specta::specta]
pub async fn verify_snapshot(
    state: State<'_, DesktopAppState>,
    request: VerifyTargetRequest,
) -> IpcResult<VerificationResultDto> {
    state.check_auth_gate().await?;

    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let sid_str = request
        .target_id
        .as_deref()
        .ok_or_else(|| IpcError::validation("Target snapshot ID is required"))?;
    let sid = SnapshotId::new(sid_str)
        .map_err(|e| IpcError::validation(format!("Invalid snapshot ID: {e}")))?;

    let options = parse_options(&request);
    let op_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("verify-snapshot-{}", sid));
    let cancel = state.register_cancellation(&op_id);
    let engine = Arc::clone(&state.verification_engine);

    let result =
        tokio::task::spawn_blocking(move || engine.verify_snapshot(&pid, &sid, &options, &cancel))
            .await
            .map_err(|e| IpcError::internal(format!("Worker thread failed: {e}")))?
            .map_err(IpcError::from)?;

    state.unregister_cancellation(&op_id);
    Ok(result.into())
}

/// Verifies all tracked files and snapshots across an entire backup profile.
#[tauri::command]
#[specta::specta]
pub async fn verify_profile(
    state: State<'_, DesktopAppState>,
    request: VerifyTargetRequest,
) -> IpcResult<VerificationResultDto> {
    state.check_auth_gate().await?;

    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let options = parse_options(&request);
    let op_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("verify-profile-{}", pid));
    let cancel = state.register_cancellation(&op_id);
    let engine = Arc::clone(&state.verification_engine);

    let result =
        tokio::task::spawn_blocking(move || engine.verify_profile(&pid, &options, &cancel))
            .await
            .map_err(|e| IpcError::internal(format!("Worker thread failed: {e}")))?
            .map_err(IpcError::from)?;

    state.unregister_cancellation(&op_id);
    Ok(result.into())
}

/// Retrieves persisted historical verification records for a profile.
#[tauri::command]
#[specta::specta]
pub fn get_verification_history(
    state: State<'_, DesktopAppState>,
    profile_id: String,
    limit: Option<u32>,
) -> IpcResult<Vec<VerificationHistoryRecordDto>> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let records = state
        .db
        .list_verification_history(&pid, limit.unwrap_or(50) as usize)
        .map_err(IpcError::from)?;

    let dtos = records.into_iter().map(Into::into).collect();
    Ok(dtos)
}
