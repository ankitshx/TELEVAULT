//! Tauri IPC command handlers for background scheduler and automated backup operations.

use tauri::State;
use televault_core::ids::{ProfileId, ScheduleId};
use televault_scheduler::types::{
    CreateScheduleParams, Schedule, ScheduleType, TimezoneStrategy, UpdateScheduleParams,
};

use crate::dto::scheduler::{
    CreateScheduleRequest, ScheduleDto, ScheduleHistoryDto, SchedulerStatusDto,
    UpdateScheduleRequest,
};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;

/// Converts a domain `Schedule` into an IPC `ScheduleDto`.
fn domain_to_dto(sched: Schedule) -> ScheduleDto {
    ScheduleDto {
        schedule_id: sched.schedule_id.to_string(),
        profile_id: sched.profile_id.to_string(),
        schedule_type: sched.schedule_type.to_string(),
        expression: sched.expression,
        timezone: sched.timezone.to_string(),
        enabled: sched.enabled,
        next_run_at: sched.next_run_at.map(|t| t.to_rfc3339()),
        last_run_at: sched.last_run_at.map(|t| t.to_rfc3339()),
        last_status: sched.last_status,
        last_error_code: sched.last_error_code,
        created_at: sched.created_at.to_rfc3339(),
        updated_at: sched.updated_at.to_rfc3339(),
    }
}

/// Lists all configured backup schedules.
#[tauri::command]
#[specta::specta]
pub fn list_schedules(state: State<'_, DesktopAppState>) -> IpcResult<Vec<ScheduleDto>> {
    let schedules = state
        .scheduler_service
        .list_schedules()
        .map_err(IpcError::from)?;
    Ok(schedules.into_iter().map(domain_to_dto).collect())
}

/// Retrieves a specific backup schedule by ID.
#[tauri::command]
#[specta::specta]
pub fn get_schedule(
    state: State<'_, DesktopAppState>,
    schedule_id: String,
) -> IpcResult<ScheduleDto> {
    let sid = ScheduleId::new(&schedule_id)
        .map_err(|e| IpcError::validation(format!("Invalid schedule ID: {e}")))?;
    let schedule = state
        .scheduler_service
        .get_schedule(&sid)
        .map_err(IpcError::from)?;
    Ok(domain_to_dto(schedule))
}

/// Creates a new recurring backup schedule.
#[tauri::command]
#[specta::specta]
pub fn create_schedule(
    state: State<'_, DesktopAppState>,
    request: CreateScheduleRequest,
) -> IpcResult<ScheduleDto> {
    let pid = ProfileId::new(&request.profile_id)
        .map_err(|e| IpcError::validation(format!("Invalid profile ID: {e}")))?;
    let stype: ScheduleType = request
        .schedule_type
        .parse()
        .map_err(|e| IpcError::validation(format!("Invalid schedule type: {e}")))?;
    let tz = match request.timezone.as_deref() {
        Some(s) => Some(
            s.parse::<TimezoneStrategy>()
                .map_err(|e| IpcError::validation(format!("Invalid timezone: {e}")))?,
        ),
        None => None,
    };

    let schedule = state
        .scheduler_service
        .create_schedule(CreateScheduleParams {
            schedule_id: None,
            profile_id: pid,
            schedule_type: stype,
            expression: request.expression,
            timezone: tz,
            enabled: request.enabled,
        })
        .map_err(IpcError::from)?;

    Ok(domain_to_dto(schedule))
}

/// Updates an existing backup schedule's expression or timing.
#[tauri::command]
#[specta::specta]
pub fn update_schedule(
    state: State<'_, DesktopAppState>,
    request: UpdateScheduleRequest,
) -> IpcResult<ScheduleDto> {
    let sid = ScheduleId::new(&request.schedule_id)
        .map_err(|e| IpcError::validation(format!("Invalid schedule ID: {e}")))?;
    let stype = match request.schedule_type {
        Some(s) => Some(
            s.parse::<ScheduleType>()
                .map_err(|e| IpcError::validation(format!("Invalid schedule type: {e}")))?,
        ),
        None => None,
    };
    let tz = match request.timezone {
        Some(s) => Some(
            s.parse::<TimezoneStrategy>()
                .map_err(|e| IpcError::validation(format!("Invalid timezone: {e}")))?,
        ),
        None => None,
    };

    let schedule = state
        .scheduler_service
        .update_schedule(UpdateScheduleParams {
            schedule_id: sid,
            schedule_type: stype,
            expression: request.expression,
            timezone: tz,
            enabled: request.enabled,
        })
        .map_err(IpcError::from)?;

    Ok(domain_to_dto(schedule))
}

/// Deletes a backup schedule by its unique ID.
#[tauri::command]
#[specta::specta]
pub fn delete_schedule(state: State<'_, DesktopAppState>, schedule_id: String) -> IpcResult<bool> {
    let sid = ScheduleId::new(&schedule_id)
        .map_err(|e| IpcError::validation(format!("Invalid schedule ID: {e}")))?;
    state
        .scheduler_service
        .delete_schedule(&sid)
        .map_err(IpcError::from)
}

/// Enables a schedule and recalculates its next execution time.
#[tauri::command]
#[specta::specta]
pub fn enable_schedule(
    state: State<'_, DesktopAppState>,
    schedule_id: String,
) -> IpcResult<ScheduleDto> {
    let sid = ScheduleId::new(&schedule_id)
        .map_err(|e| IpcError::validation(format!("Invalid schedule ID: {e}")))?;
    let schedule = state
        .scheduler_service
        .enable_schedule(&sid)
        .map_err(IpcError::from)?;
    Ok(domain_to_dto(schedule))
}

/// Disables a schedule and cancels pending executions.
#[tauri::command]
#[specta::specta]
pub fn disable_schedule(
    state: State<'_, DesktopAppState>,
    schedule_id: String,
) -> IpcResult<ScheduleDto> {
    let sid = ScheduleId::new(&schedule_id)
        .map_err(|e| IpcError::validation(format!("Invalid schedule ID: {e}")))?;
    let schedule = state
        .scheduler_service
        .disable_schedule(&sid)
        .map_err(IpcError::from)?;
    Ok(domain_to_dto(schedule))
}

/// Triggers an immediate execution of a scheduled backup.
#[tauri::command]
#[specta::specta]
pub async fn run_schedule_now(
    state: State<'_, DesktopAppState>,
    schedule_id: String,
) -> IpcResult<Option<String>> {
    state.check_auth_gate().await?;

    let sid = ScheduleId::new(&schedule_id)
        .map_err(|e| IpcError::validation(format!("Invalid schedule ID: {e}")))?;

    let scheduler = state.scheduler_service.clone();
    let res = tokio::task::spawn_blocking(move || scheduler.trigger_schedule_now(&sid))
        .await
        .map_err(|e| IpcError::internal(format!("Task spawn failed: {e}")))?
        .map_err(IpcError::from)?;

    Ok(res.map(|s| s.to_string()))
}

/// Queries current operational status and active workers of the scheduler.
#[tauri::command]
#[specta::specta]
pub fn get_scheduler_status(state: State<'_, DesktopAppState>) -> IpcResult<SchedulerStatusDto> {
    let status = state.scheduler_service.status();
    let schedules = state
        .scheduler_service
        .list_schedules()
        .map_err(IpcError::from)?;
    let active_count = schedules.iter().filter(|s| s.enabled).count() as u32;
    let running = state
        .scheduler_service
        .execution_guard()
        .active_profiles()
        .into_iter()
        .map(|p| p.to_string())
        .collect();

    Ok(SchedulerStatusDto {
        status: status.to_string(),
        active_schedules_count: active_count,
        running_profiles: running,
    })
}

/// Retrieves execution history for a schedule.
#[tauri::command]
#[specta::specta]
pub fn get_schedule_history(
    state: State<'_, DesktopAppState>,
    schedule_id: String,
    limit: Option<u32>,
) -> IpcResult<Vec<ScheduleHistoryDto>> {
    let sid = ScheduleId::new(&schedule_id)
        .map_err(|e| IpcError::validation(format!("Invalid schedule ID: {e}")))?;
    let limit = limit.unwrap_or(50) as usize;

    let records = state
        .scheduler_service
        .get_history(&sid, limit)
        .map_err(IpcError::from)?;

    Ok(records
        .into_iter()
        .map(|r| ScheduleHistoryDto {
            history_id: r.history_id,
            schedule_id: r.schedule_id.to_string(),
            profile_id: r.profile_id.to_string(),
            started_at: r.started_at,
            completed_at: r.completed_at,
            status: r.status,
            snapshot_id: r.snapshot_id.map(|s| s.to_string()),
            files_processed: r.files_processed,
            bytes_transferred: r.bytes_transferred,
            error_code: r.error_code,
            error_message: r.error_message,
        })
        .collect())
}
