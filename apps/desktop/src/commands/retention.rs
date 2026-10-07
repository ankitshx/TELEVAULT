//! Retention policy and snapshot pruning IPC commands.

use chrono::Utc;
use tauri::State;
use televault_backup::retention::RetentionPolicy;
use televault_core::ids::ProfileId;

use crate::dto::{
    RetentionDecisionDto, RetentionEvaluationDto, RetentionHistoryDto, RetentionPolicyDto,
    RetentionResultDto, SetRetentionPolicyRequest,
};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

/// Retrieves the configured retention policy for a backup profile.
#[tauri::command]
#[specta::specta]
pub fn get_retention_policy(
    state: State<'_, DesktopAppState>,
    profile_id: String,
) -> IpcResult<RetentionPolicyDto> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let policy = state
        .retention_engine
        .get_retention_policy(&pid)
        .map_err(IpcError::from)?;

    Ok(RetentionPolicyDto {
        profile_id: policy.profile_id.to_string(),
        keep_latest_n: policy.keep_latest_n,
        keep_newer_than_secs: policy
            .keep_newer_than_secs
            .map(|s| s.min(u32::MAX as u64) as u32),
        keep_latest_successful: policy.keep_latest_successful,
        keep_latest_always: policy.keep_latest_always,
        prune_failed: policy.prune_failed,
        prune_empty: policy.prune_empty,
        enabled: policy.enabled,
    })
}

/// Updates and persists the retention policy configuration for a profile.
#[tauri::command]
#[specta::specta]
pub fn set_retention_policy(
    state: State<'_, DesktopAppState>,
    request: SetRetentionPolicyRequest,
) -> IpcResult<RetentionPolicyDto> {
    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let policy = RetentionPolicy {
        profile_id: pid.clone(),
        keep_latest_n: request.keep_latest_n,
        keep_newer_than_secs: request.keep_newer_than_secs.map(|s| s as u64),
        keep_latest_successful: request.keep_latest_successful.unwrap_or(true),
        keep_latest_always: request.keep_latest_always.unwrap_or(true),
        prune_failed: request.prune_failed.unwrap_or(false),
        prune_empty: request.prune_empty.unwrap_or(false),
        enabled: request.enabled.unwrap_or(true),
    };

    state
        .retention_engine
        .save_retention_policy(&policy)
        .map_err(IpcError::from)?;

    Ok(RetentionPolicyDto {
        profile_id: policy.profile_id.to_string(),
        keep_latest_n: policy.keep_latest_n,
        keep_newer_than_secs: policy
            .keep_newer_than_secs
            .map(|s| s.min(u32::MAX as u64) as u32),
        keep_latest_successful: policy.keep_latest_successful,
        keep_latest_always: policy.keep_latest_always,
        prune_failed: policy.prune_failed,
        prune_empty: policy.prune_empty,
        enabled: policy.enabled,
    })
}

/// Previews retention policy evaluation (dry-run) without modifying database records.
#[tauri::command]
#[specta::specta]
pub fn preview_retention(
    state: State<'_, DesktopAppState>,
    profile_id: String,
) -> IpcResult<RetentionEvaluationDto> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let active_snapshots = state.active_snapshots();
    let evaluation = state
        .retention_engine
        .evaluate_retention(&pid, None, &active_snapshots, Utc::now())
        .map_err(IpcError::from)?;

    let decisions = evaluation
        .decisions
        .into_iter()
        .map(|d| RetentionDecisionDto {
            snapshot_id: d.snapshot_id.to_string(),
            action: d.action.to_string(),
            reason: d.reason.to_string(),
            description: d.description,
            snapshot_status: d.snapshot_status.to_string(),
            snapshot_created_at: d.snapshot_created_at,
            age_secs: d.age_secs.min(u32::MAX as u64) as u32,
            version_count: d.version_count as u32,
        })
        .collect();

    Ok(RetentionEvaluationDto {
        profile_id: evaluation.profile_id.to_string(),
        evaluated_at: evaluation.evaluated_at,
        policy: RetentionPolicyDto {
            profile_id: evaluation.policy.profile_id.to_string(),
            keep_latest_n: evaluation.policy.keep_latest_n,
            keep_newer_than_secs: evaluation
                .policy
                .keep_newer_than_secs
                .map(|s| s.min(u32::MAX as u64) as u32),
            keep_latest_successful: evaluation.policy.keep_latest_successful,
            keep_latest_always: evaluation.policy.keep_latest_always,
            prune_failed: evaluation.policy.prune_failed,
            prune_empty: evaluation.policy.prune_empty,
            enabled: evaluation.policy.enabled,
        },
        decisions,
        snapshots_evaluated: evaluation.snapshots_evaluated as u32,
        snapshots_kept: evaluation.snapshots_kept as u32,
        snapshots_pruned: evaluation.snapshots_pruned as u32,
    })
}

/// Executes retention pruning on local metadata for a profile.
#[tauri::command]
#[specta::specta]
pub fn execute_retention(
    state: State<'_, DesktopAppState>,
    profile_id: String,
) -> IpcResult<RetentionResultDto> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    // Mutual exclusion check with active backup operations
    if state.scheduler_service.execution_guard().is_running(&pid) {
        return Err(IpcError::conflict(format!(
            "Backup for profile '{profile_id}' is currently running; retention pruning deferred"
        )));
    }

    let active_snapshots = state.active_snapshots();
    let result = state
        .retention_engine
        .execute_retention(&pid, None, &active_snapshots, false, Utc::now())
        .map_err(IpcError::from)?;

    let decisions = result
        .evaluation
        .decisions
        .into_iter()
        .map(|d| RetentionDecisionDto {
            snapshot_id: d.snapshot_id.to_string(),
            action: d.action.to_string(),
            reason: d.reason.to_string(),
            description: d.description,
            snapshot_status: d.snapshot_status.to_string(),
            snapshot_created_at: d.snapshot_created_at,
            age_secs: d.age_secs.min(u32::MAX as u64) as u32,
            version_count: d.version_count as u32,
        })
        .collect();

    Ok(RetentionResultDto {
        profile_id: result.profile_id.to_string(),
        executed_at: result.executed_at,
        dry_run: result.dry_run,
        evaluation: RetentionEvaluationDto {
            profile_id: result.evaluation.profile_id.to_string(),
            evaluated_at: result.evaluation.evaluated_at,
            policy: RetentionPolicyDto {
                profile_id: result.evaluation.policy.profile_id.to_string(),
                keep_latest_n: result.evaluation.policy.keep_latest_n,
                keep_newer_than_secs: result
                    .evaluation
                    .policy
                    .keep_newer_than_secs
                    .map(|s| s.min(u32::MAX as u64) as u32),
                keep_latest_successful: result.evaluation.policy.keep_latest_successful,
                keep_latest_always: result.evaluation.policy.keep_latest_always,
                prune_failed: result.evaluation.policy.prune_failed,
                prune_empty: result.evaluation.policy.prune_empty,
                enabled: result.evaluation.policy.enabled,
            },
            decisions,
            snapshots_evaluated: result.evaluation.snapshots_evaluated as u32,
            snapshots_kept: result.evaluation.snapshots_kept as u32,
            snapshots_pruned: result.evaluation.snapshots_pruned as u32,
        },
        pruned_snapshots: result
            .pruned_snapshots
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        pruned_versions_count: result.pruned_versions_count as u32,
        success: result.success,
        error_message: result.error_message,
    })
}

/// Retrieves retention execution audit history for a profile.
#[tauri::command]
#[specta::specta]
pub fn get_retention_history(
    state: State<'_, DesktopAppState>,
    profile_id: String,
    limit: Option<u32>,
) -> IpcResult<Vec<RetentionHistoryDto>> {
    let pid = ProfileId::new(&profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;

    let records = state
        .retention_engine
        .list_retention_history(&pid, limit.unwrap_or(20) as usize)
        .map_err(IpcError::from)?;

    let dtos = records
        .into_iter()
        .map(|r| {
            let pruned_snapshot_ids: Vec<String> =
                serde_json::from_str(&r.pruned_snapshot_ids).unwrap_or_default();

            RetentionHistoryDto {
                history_id: r.history_id,
                profile_id: r.profile_id.to_string(),
                executed_at: r.executed_at,
                dry_run: r.dry_run,
                snapshots_evaluated: r.snapshots_evaluated,
                snapshots_kept: r.snapshots_kept,
                snapshots_pruned: r.snapshots_pruned,
                pruned_snapshot_ids,
                status: r.status,
                error_message: r.error_message,
            }
        })
        .collect();

    Ok(dtos)
}
