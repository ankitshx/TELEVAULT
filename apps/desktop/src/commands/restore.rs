//! Restore and disaster recovery IPC commands.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use televault_backup::restore::{RestoreRequest, SnapshotRestoreRequest};
use televault_core::ids::{FileId, SnapshotId};
use televault_crypto::kdf::{derive_key, KdfParams};
use televault_crypto::key::Salt;
use televault_crypto::policy::EncryptionPolicy;
use televault_manifest::ManifestV1;

use crate::dto::{
    FullVerificationReportDto, ManifestVerificationReportDto, RestoreFileRequest,
    RestoreManifestRequest, RestoreResultDto, RestoreSnapshotRequest, SnapshotRestoreResultDto,
    VerifyManifestRequest,
};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

/// Restores a backed-up logical file to a specified local destination path.
#[tauri::command]
#[specta::specta]
pub async fn restore_file(
    state: State<'_, DesktopAppState>,
    request: RestoreFileRequest,
) -> IpcResult<RestoreResultDto> {
    state.check_auth_gate().await?;

    let fid = FileId::new(&request.file_id)
        .map_err(|e| IpcError::validation(format!("Invalid file ID: {e}")))?;

    let target_path = PathBuf::from(&request.destination_path);
    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                IpcError::new(
                    "FILESYSTEM_ERROR",
                    format!("Failed to create destination directory: {e}"),
                )
            })?;
        }
    }

    let manifest = state
        .db
        .get_manifest_by_file_id(&fid)
        .map_err(IpcError::from)?
        .ok_or_else(|| {
            IpcError::not_found(format!("Manifest for file '{}' not found", request.file_id))
        })?;

    let encryption_policy = if manifest.encryption.is_some() {
        let passphrase = request.passphrase.as_deref().unwrap_or("");
        if passphrase.is_empty() {
            return Err(IpcError::validation(
                "Passphrase required for encrypted file restore",
            ));
        }
        let salt_hash =
            Sha256::digest(format!("salt:{}", manifest.logical_file.file_id).as_bytes());
        let salt =
            Salt::from_slice(&salt_hash[..32]).map_err(|e| IpcError::internal(e.to_string()))?;
        let secret_key = derive_key(passphrase, &salt, &KdfParams::default())
            .map_err(|e| IpcError::internal(format!("Key derivation failed: {e}")))?;
        EncryptionPolicy::Enabled(secret_key)
    } else {
        EncryptionPolicy::Disabled
    };

    let restore_req = RestoreRequest::new(fid, target_path)
        .with_collision_policy(request.collision_policy.into())
        .with_encryption_policy(encryption_policy);

    let cancel_token = state.register_cancellation(&request.file_id);
    let restore_engine = Arc::clone(&state.restore_engine);
    let op_id = request.file_id.clone();

    let res = tokio::task::spawn_blocking(move || {
        restore_engine.restore_file(&restore_req, &cancel_token)
    })
    .await
    .map_err(|e| IpcError::internal(format!("Task spawn failed: {e}")))?
    .map_err(IpcError::from);

    state.unregister_cancellation(&op_id);

    let r = res?;
    Ok(RestoreResultDto {
        file_id: r.file_id.to_string(),
        target_path: r.target_path.to_string_lossy().to_string(),
        bytes_restored: r.bytes_restored,
        duration_ms: r.elapsed_ms,
        outcome: r.outcome.into(),
    })
}

/// Restores a logical file from a standalone Manifest V1 JSON string.
#[tauri::command]
#[specta::specta]
pub async fn restore_manifest(
    state: State<'_, DesktopAppState>,
    request: RestoreManifestRequest,
) -> IpcResult<RestoreResultDto> {
    state.check_auth_gate().await?;

    let manifest = ManifestV1::from_json(&request.manifest_json)
        .map_err(|e| IpcError::validation(format!("Invalid manifest JSON: {e}")))?;

    let target_path = PathBuf::from(&request.destination_path);
    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                IpcError::new(
                    "FILESYSTEM_ERROR",
                    format!("Failed to create destination directory: {e}"),
                )
            })?;
        }
    }

    let encryption_policy = if manifest.encryption.is_some() {
        let passphrase = request.passphrase.as_deref().unwrap_or("");
        if passphrase.is_empty() {
            return Err(IpcError::validation(
                "Passphrase required for encrypted manifest restore",
            ));
        }
        let salt_hash =
            Sha256::digest(format!("salt:{}", manifest.logical_file.file_id).as_bytes());
        let salt =
            Salt::from_slice(&salt_hash[..32]).map_err(|e| IpcError::internal(e.to_string()))?;
        let secret_key = derive_key(passphrase, &salt, &KdfParams::default())
            .map_err(|e| IpcError::internal(format!("Key derivation failed: {e}")))?;
        EncryptionPolicy::Enabled(secret_key)
    } else {
        EncryptionPolicy::Disabled
    };

    let op_id = format!("man-{}", manifest.logical_file.file_id);
    let cancel_token = state.register_cancellation(&op_id);
    let restore_engine = Arc::clone(&state.restore_engine);
    let collision_policy = request.collision_policy.into();
    let op_id_clone = op_id.clone();

    let res = tokio::task::spawn_blocking(move || {
        restore_engine.restore_manifest(
            &manifest,
            &target_path,
            collision_policy,
            &encryption_policy,
            &cancel_token,
        )
    })
    .await
    .map_err(|e| IpcError::internal(format!("Task spawn failed: {e}")))?
    .map_err(IpcError::from);

    state.unregister_cancellation(&op_id_clone);

    let r = res?;
    Ok(RestoreResultDto {
        file_id: r.file_id.to_string(),
        target_path: r.target_path.to_string_lossy().to_string(),
        bytes_restored: r.bytes_restored,
        duration_ms: r.elapsed_ms,
        outcome: r.outcome.into(),
    })
}

/// Restores an entire historical snapshot hierarchy into a destination folder.
#[tauri::command]
#[specta::specta]
pub async fn restore_snapshot(
    state: State<'_, DesktopAppState>,
    request: RestoreSnapshotRequest,
) -> IpcResult<SnapshotRestoreResultDto> {
    state.check_auth_gate().await?;

    let sid = SnapshotId::new(&request.snapshot_id)
        .map_err(|e| IpcError::validation(format!("Invalid snapshot ID: {e}")))?;

    let target_dir = PathBuf::from(&request.destination_directory);
    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir).map_err(|e| {
            IpcError::new(
                "FILESYSTEM_ERROR",
                format!("Failed to create destination directory: {e}"),
            )
        })?;
    }

    let encryption_policy = if let Some(passphrase) = request.passphrase {
        if !passphrase.is_empty() {
            let salt_hash = Sha256::digest(format!("salt:{}", sid).as_bytes());
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

    let snap_req = SnapshotRestoreRequest::new(sid, target_dir)
        .with_collision_policy(request.collision_policy.into())
        .with_encryption_policy(encryption_policy);

    let op_id = format!("snap-{}", request.snapshot_id);
    let cancel_token = state.register_cancellation(&op_id);
    let restore_engine = Arc::clone(&state.restore_engine);
    let op_id_clone = op_id.clone();

    let res = tokio::task::spawn_blocking(move || {
        restore_engine.restore_snapshot(&snap_req, &cancel_token)
    })
    .await
    .map_err(|e| IpcError::internal(format!("Task spawn failed: {e}")))?
    .map_err(IpcError::from);

    state.unregister_cancellation(&op_id_clone);

    let r = res?;
    Ok(SnapshotRestoreResultDto {
        snapshot_id: r.snapshot_id.to_string(),
        target_directory: request.destination_directory,
        total_files: r.total_files,
        restored_files: r.restored_files,
        skipped_files: r.skipped_files,
        failed_files: r.failed_files,
        total_bytes: r.total_bytes_restored,
        duration_ms: r.elapsed_ms,
    })
}

/// Fast, metadata-only pre-validation for a manifest (zero full-payload download).
#[tauri::command]
#[specta::specta]
pub fn verify_manifest_metadata(
    state: State<'_, DesktopAppState>,
    request: VerifyManifestRequest,
) -> IpcResult<ManifestVerificationReportDto> {
    let fid = FileId::new(&request.file_id)
        .map_err(|e| IpcError::validation(format!("Invalid file ID: {e}")))?;

    let manifest = state
        .db
        .get_manifest_by_file_id(&fid)
        .map_err(IpcError::from)?
        .ok_or_else(|| {
            IpcError::not_found(format!("Manifest for file '{}' not found", request.file_id))
        })?;

    let report = state
        .checker
        .verify_manifest_metadata(&manifest, Some(&*state.storage_provider));

    Ok(ManifestVerificationReportDto {
        file_id: report.file_id.to_string(),
        relative_path: report.relative_path,
        is_restorable: report.is_restorable,
        total_chunks: report.total_chunks,
        original_size: report.original_size,
        remote_objects_verified: report.remote_objects_verified,
        issues: report.issues,
    })
}

/// Full end-to-end trial restore verification into temporary managed staging.
#[tauri::command]
#[specta::specta]
pub async fn verify_full_restore(
    state: State<'_, DesktopAppState>,
    request: RestoreFileRequest,
) -> IpcResult<FullVerificationReportDto> {
    state.check_auth_gate().await?;

    let fid = FileId::new(&request.file_id)
        .map_err(|e| IpcError::validation(format!("Invalid file ID: {e}")))?;

    let manifest = state
        .db
        .get_manifest_by_file_id(&fid)
        .map_err(IpcError::from)?
        .ok_or_else(|| {
            IpcError::not_found(format!("Manifest for file '{}' not found", request.file_id))
        })?;

    let encryption_policy = if manifest.encryption.is_some() {
        let passphrase = request.passphrase.as_deref().unwrap_or("");
        if passphrase.is_empty() {
            return Err(IpcError::validation(
                "Passphrase required for encrypted file verification",
            ));
        }
        let salt_hash =
            Sha256::digest(format!("salt:{}", manifest.logical_file.file_id).as_bytes());
        let salt =
            Salt::from_slice(&salt_hash[..32]).map_err(|e| IpcError::internal(e.to_string()))?;
        let secret_key = derive_key(passphrase, &salt, &KdfParams::default())
            .map_err(|e| IpcError::internal(format!("Key derivation failed: {e}")))?;
        EncryptionPolicy::Enabled(secret_key)
    } else {
        EncryptionPolicy::Disabled
    };

    let op_id = format!("verify-{}", request.file_id);
    let cancel_token = state.register_cancellation(&op_id);
    let checker = Arc::clone(&state.checker);
    let transfer_engine = Arc::clone(&state.transfer_engine);
    let temp_manager = Arc::clone(&state.temp_manager);
    let op_id_clone = op_id.clone();

    let r = tokio::task::spawn_blocking(move || {
        checker.verify_full_restore(
            &manifest,
            &encryption_policy,
            &transfer_engine,
            &temp_manager,
            &cancel_token,
        )
    })
    .await
    .map_err(|e| IpcError::internal(format!("Task spawn failed: {e}")))?;

    state.unregister_cancellation(&op_id_clone);

    Ok(FullVerificationReportDto {
        file_id: r.file_id.to_string(),
        verified: r.is_valid,
        sha256_match: r.calculated_sha256.as_deref() == Some(&r.expected_sha256),
        size_match: r.error.is_none(),
        duration_ms: r.elapsed_ms,
        error: r.error,
    })
}
