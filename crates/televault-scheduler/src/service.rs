//! Production-grade background scheduler service coordinating automated backup execution.

use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};
use televault_backup::engine::BackupEngine;
use televault_backup::profile::BackupProfile;
use televault_core::ids::{ProfileId, ScheduleId, SnapshotId};
use televault_crypto::policy::EncryptionPolicy;
use televault_db::{Database, ScheduleHistoryRecord, ScheduleRecord};
use televault_transfer::cancellation::CancellationToken;

use crate::clock::{Clock, SystemClock};
use crate::error::{Result, SchedulerError};
use crate::expression::{calculate_next_run, validate_expression};
use crate::guard::ExecutionGuard;
use crate::types::{
    CreateScheduleParams, Schedule, ScheduleType, SchedulerServiceStatus, TimezoneStrategy,
    UpdateScheduleParams,
};

/// Production background scheduler service managing automated recurring backups.
pub struct SchedulerService {
    db: Arc<Database>,
    backup_engine: Arc<BackupEngine>,
    execution_guard: Arc<ExecutionGuard>,
    clock: Arc<dyn Clock + Send + Sync>,
    notify: Arc<tokio::sync::Notify>,
    cancel_token: CancellationToken,
    status: Arc<Mutex<SchedulerServiceStatus>>,
    task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl SchedulerService {
    /// Creates a new `SchedulerService` with standard production dependencies.
    pub fn new(
        db: Arc<Database>,
        backup_engine: Arc<BackupEngine>,
        custom_clock: Option<Arc<dyn Clock + Send + Sync>>,
    ) -> Self {
        let clock = custom_clock.unwrap_or_else(|| Arc::new(SystemClock::new()));
        Self {
            db,
            backup_engine,
            execution_guard: Arc::new(ExecutionGuard::new()),
            clock,
            notify: Arc::new(tokio::sync::Notify::new()),
            cancel_token: CancellationToken::new(),
            status: Arc::new(Mutex::new(SchedulerServiceStatus::Stopped)),
            task_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Access the shared per-profile execution guard.
    pub fn execution_guard(&self) -> &Arc<ExecutionGuard> {
        &self.execution_guard
    }

    /// Access the underlying database instance.
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    /// Returns the current service operational status.
    pub fn status(&self) -> SchedulerServiceStatus {
        *self.status.lock().unwrap()
    }

    /// Starts the background scheduler loop on the active Tokio runtime.
    pub fn start(&self) -> Result<()> {
        let mut status_lock = self.status.lock().unwrap();
        if *status_lock == SchedulerServiceStatus::Running {
            return Ok(());
        }
        *status_lock = SchedulerServiceStatus::Running;

        let db = Arc::clone(&self.db);
        let backup_engine = Arc::clone(&self.backup_engine);
        let execution_guard = Arc::clone(&self.execution_guard);
        let clock = Arc::clone(&self.clock);
        let notify = Arc::clone(&self.notify);
        let cancel_token = self.cancel_token.clone();
        let status = Arc::clone(&self.status);

        let handle = tokio::spawn(async move {
            run_scheduler_loop(
                db,
                backup_engine,
                execution_guard,
                clock,
                notify,
                cancel_token,
                status,
            )
            .await;
        });

        *self.task_handle.lock().unwrap() = Some(handle);
        Ok(())
    }

    /// Signals graceful shutdown to the scheduler loop and awaits its completion.
    pub async fn stop(&self) -> Result<()> {
        {
            let mut status_lock = self.status.lock().unwrap();
            *status_lock = SchedulerServiceStatus::Stopped;
        }

        self.cancel_token.cancel();
        self.notify.notify_one();

        let handle = { self.task_handle.lock().unwrap().take() };
        if let Some(h) = handle {
            let _ = h.await;
        }

        Ok(())
    }

    /// Creates and persists a new backup schedule, recalculating its initial next run.
    pub fn create_schedule(&self, params: CreateScheduleParams) -> Result<Schedule> {
        validate_expression(params.schedule_type, &params.expression)?;

        // Verify profile exists
        if self.db.get_profile(&params.profile_id)?.is_none() {
            return Err(SchedulerError::ProfileNotFound(
                params.profile_id.to_string(),
            ));
        }

        let schedule_id = params
            .schedule_id
            .unwrap_or_else(|| ScheduleId::new(format!("sched-{}", uuid_or_timestamp())).unwrap());

        let tz = params.timezone.unwrap_or_default();
        let enabled = params.enabled.unwrap_or(true);
        let now = self.clock.now_utc();

        let next_run_at = if enabled {
            Some(calculate_next_run(
                params.schedule_type,
                &params.expression,
                tz,
                now,
            )?)
        } else {
            None
        };

        let now_str = now.to_rfc3339();
        let record = ScheduleRecord {
            schedule_id: schedule_id.clone(),
            profile_id: params.profile_id.clone(),
            schedule_type: params.schedule_type.to_string(),
            expression: params.expression.clone(),
            timezone: tz.to_string(),
            enabled,
            next_run_at: next_run_at.map(|t| t.to_rfc3339()),
            last_run_at: None,
            last_status: Some("idle".to_string()),
            last_error_code: None,
            created_at: now_str.clone(),
            updated_at: now_str,
        };

        self.db.create_schedule(&record)?;
        self.notify.notify_one();

        self.get_schedule(&schedule_id)
    }

    /// Retrieves an existing schedule by ID.
    pub fn get_schedule(&self, id: &ScheduleId) -> Result<Schedule> {
        let record = self
            .db
            .get_schedule(id)?
            .ok_or_else(|| SchedulerError::ScheduleNotFound(id.to_string()))?;
        record_to_domain(record)
    }

    /// Lists all configured schedules.
    pub fn list_schedules(&self) -> Result<Vec<Schedule>> {
        let records = self.db.list_schedules()?;
        records.into_iter().map(record_to_domain).collect()
    }

    /// Lists all schedules configured for a specific profile.
    pub fn list_schedules_for_profile(&self, profile_id: &ProfileId) -> Result<Vec<Schedule>> {
        let records = self.db.list_schedules_for_profile(profile_id)?;
        records.into_iter().map(record_to_domain).collect()
    }

    /// Updates configuration of an existing schedule and recalculates next execution.
    pub fn update_schedule(&self, params: UpdateScheduleParams) -> Result<Schedule> {
        let mut existing = self.get_schedule(&params.schedule_id)?;

        let mut changed_timing = false;

        if let Some(stype) = params.schedule_type {
            existing.schedule_type = stype;
            changed_timing = true;
        }

        if let Some(expr) = params.expression {
            existing.expression = expr;
            changed_timing = true;
        }

        if let Some(tz) = params.timezone {
            existing.timezone = tz;
            changed_timing = true;
        }

        if let Some(en) = params.enabled {
            if existing.enabled != en {
                existing.enabled = en;
                changed_timing = true;
            }
        }

        validate_expression(existing.schedule_type, &existing.expression)?;

        let now = self.clock.now_utc();
        if changed_timing {
            existing.next_run_at = if existing.enabled {
                Some(calculate_next_run(
                    existing.schedule_type,
                    &existing.expression,
                    existing.timezone,
                    now,
                )?)
            } else {
                None
            };
        }

        let record = ScheduleRecord {
            schedule_id: existing.schedule_id.clone(),
            profile_id: existing.profile_id.clone(),
            schedule_type: existing.schedule_type.to_string(),
            expression: existing.expression.clone(),
            timezone: existing.timezone.to_string(),
            enabled: existing.enabled,
            next_run_at: existing.next_run_at.map(|t| t.to_rfc3339()),
            last_run_at: existing.last_run_at.map(|t| t.to_rfc3339()),
            last_status: existing.last_status.clone(),
            last_error_code: existing.last_error_code.clone(),
            created_at: existing.created_at.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };

        self.db.update_schedule(&record)?;
        self.notify.notify_one();

        self.get_schedule(&existing.schedule_id)
    }

    /// Deletes a schedule by its unique ID.
    pub fn delete_schedule(&self, id: &ScheduleId) -> Result<bool> {
        let deleted = self.db.delete_schedule(id)?;
        if deleted {
            self.notify.notify_one();
        }
        Ok(deleted)
    }

    /// Enables a schedule and recalculates its next execution.
    pub fn enable_schedule(&self, id: &ScheduleId) -> Result<Schedule> {
        self.update_schedule(UpdateScheduleParams {
            schedule_id: id.clone(),
            schedule_type: None,
            expression: None,
            timezone: None,
            enabled: Some(true),
        })
    }

    /// Disables a schedule and clears its pending execution.
    pub fn disable_schedule(&self, id: &ScheduleId) -> Result<Schedule> {
        self.update_schedule(UpdateScheduleParams {
            schedule_id: id.clone(),
            schedule_type: None,
            expression: None,
            timezone: None,
            enabled: Some(false),
        })
    }

    /// Triggers an immediate execution of the specified schedule.
    pub fn trigger_schedule_now(&self, id: &ScheduleId) -> Result<Option<SnapshotId>> {
        let schedule = self.get_schedule(id)?;
        let snap_id = execute_single_scheduled_backup(
            &self.db,
            &self.backup_engine,
            &self.execution_guard,
            &self.clock,
            &schedule,
            &self.cancel_token,
        )?;
        self.notify.notify_one();
        Ok(snap_id)
    }

    /// Lists historical execution records for a schedule.
    pub fn get_history(&self, id: &ScheduleId, limit: usize) -> Result<Vec<ScheduleHistoryRecord>> {
        Ok(self.db.list_schedule_history(id, limit)?)
    }
}

/// Core event-driven background loop. Sleeps until next execution or notification.
async fn run_scheduler_loop(
    db: Arc<Database>,
    backup_engine: Arc<BackupEngine>,
    execution_guard: Arc<ExecutionGuard>,
    clock: Arc<dyn Clock + Send + Sync>,
    notify: Arc<tokio::sync::Notify>,
    cancel_token: CancellationToken,
    status: Arc<Mutex<SchedulerServiceStatus>>,
) {
    loop {
        if cancel_token.is_cancelled() {
            break;
        }

        let is_running = { *status.lock().unwrap() == SchedulerServiceStatus::Running };
        if !is_running {
            tokio::select! {
                _ = notify.notified() => continue,
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => continue,
            }
        }

        let now = clock.now_utc();
        let now_str = now.to_rfc3339();

        // 1. Process all due schedules
        let due_schedules = match db.list_due_schedules(&now_str) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to query due schedules: {e}");
                Vec::new()
            }
        };

        for record in due_schedules {
            if cancel_token.is_cancelled() {
                break;
            }

            if let Ok(schedule) = record_to_domain(record) {
                let _ = execute_single_scheduled_backup(
                    &db,
                    &backup_engine,
                    &execution_guard,
                    &clock,
                    &schedule,
                    &cancel_token,
                );
            }
        }

        // 2. Find earliest next_run_at among all enabled schedules
        let sleep_duration = match db.list_schedules() {
            Ok(schedules) => {
                let mut earliest_future: Option<DateTime<Utc>> = None;
                for s in schedules {
                    if s.enabled {
                        if let Some(next_str) = s.next_run_at {
                            if let Ok(dt) = DateTime::parse_from_rfc3339(&next_str) {
                                let dt_utc = dt.with_timezone(&Utc);
                                if dt_utc > now {
                                    earliest_future = match earliest_future {
                                        Some(prev) => Some(prev.min(dt_utc)),
                                        None => Some(dt_utc),
                                    };
                                }
                            }
                        }
                    }
                }

                if let Some(earliest) = earliest_future {
                    let diff = (earliest - now).num_seconds();
                    // Sleep at least 1 second, at most 60 seconds (periodic sync safety)
                    tokio::time::Duration::from_secs((diff.max(1) as u64).min(60))
                } else {
                    tokio::time::Duration::from_secs(60)
                }
            }
            Err(_) => tokio::time::Duration::from_secs(60),
        };

        // 3. Sleep or wait for event notification (0% CPU idle)
        tokio::select! {
            _ = tokio::time::sleep(sleep_duration) => {},
            _ = notify.notified() => {},
        }
    }
}

/// Executes a scheduled backup for a single schedule, safely recording outcomes and updating timing.
fn execute_single_scheduled_backup(
    db: &Database,
    backup_engine: &BackupEngine,
    execution_guard: &ExecutionGuard,
    clock: &Arc<dyn Clock + Send + Sync>,
    schedule: &Schedule,
    cancel_token: &CancellationToken,
) -> Result<Option<SnapshotId>> {
    let now = clock.now_utc();
    let history_id = format!("hist-{}", uuid_or_timestamp());
    let started_at = now.to_rfc3339();

    // 1. Verify profile existence
    let profile_record = match db.get_profile(&schedule.profile_id)? {
        Some(p) => p,
        None => {
            // Profile was deleted, mark schedule invalid
            let _ = db.update_schedule_status(
                &schedule.schedule_id,
                None,
                Some(&started_at),
                Some("invalid_profile"),
                Some("PROFILE_NOT_FOUND"),
            );
            return Err(SchedulerError::ProfileNotFound(
                schedule.profile_id.to_string(),
            ));
        }
    };

    // 2. If profile is disabled, skip execution
    if !profile_record.enabled {
        let next_run = calculate_next_run(
            schedule.schedule_type,
            &schedule.expression,
            schedule.timezone,
            now,
        )?;
        let _ = db.update_schedule_status(
            &schedule.schedule_id,
            Some(&next_run.to_rfc3339()),
            Some(&started_at),
            Some("skipped_profile_disabled"),
            None,
        );
        return Ok(None);
    }

    // 3. Acquire per-profile execution guard
    let _guard = match execution_guard.try_acquire(&schedule.profile_id) {
        Ok(g) => g,
        Err(_) => {
            // Profile is busy (e.g. manual backup running). Coalesce this tick.
            let next_run = calculate_next_run(
                schedule.schedule_type,
                &schedule.expression,
                schedule.timezone,
                now,
            )?;
            let _ = db.update_schedule_status(
                &schedule.schedule_id,
                Some(&next_run.to_rfc3339()),
                schedule
                    .last_run_at
                    .as_ref()
                    .map(|t| t.to_rfc3339())
                    .as_deref(),
                Some("skipped_busy"),
                None,
            );
            return Ok(None);
        }
    };

    // 4. Construct domain profile
    let profile = BackupProfile::from_db_record(&profile_record, None, EncryptionPolicy::Disabled);

    // 5. Plan and execute backup
    let result = backup_engine
        .plan_backup(&profile)
        .and_then(|plan| backup_engine.execute_backup(plan, cancel_token));

    let completed_at = clock.now_utc().to_rfc3339();

    // 6. Calculate next run from current time
    let next_run = calculate_next_run(
        schedule.schedule_type,
        &schedule.expression,
        schedule.timezone,
        clock.now_utc(),
    )?;

    match result {
        Ok(summary) => {
            // Record execution history
            let hist = ScheduleHistoryRecord {
                history_id,
                schedule_id: schedule.schedule_id.clone(),
                profile_id: schedule.profile_id.clone(),
                started_at,
                completed_at: Some(completed_at.clone()),
                status: "completed".to_string(),
                snapshot_id: Some(summary.snapshot_id.clone()),
                files_processed: (summary.new_files
                    + summary.modified_files
                    + summary.unchanged_files) as u64,
                bytes_transferred: summary.transferred_bytes,
                error_code: None,
                error_message: None,
            };
            let _ = db.record_schedule_history(&hist);

            // Update schedule status
            let _ = db.update_schedule_status(
                &schedule.schedule_id,
                Some(&next_run.to_rfc3339()),
                Some(&completed_at),
                Some("completed"),
                None,
            );

            Ok(Some(summary.snapshot_id))
        }
        Err(err) => {
            let error_code = "BACKUP_FAILED".to_string();
            let error_msg = err.to_string();

            let hist = ScheduleHistoryRecord {
                history_id,
                schedule_id: schedule.schedule_id.clone(),
                profile_id: schedule.profile_id.clone(),
                started_at,
                completed_at: Some(completed_at.clone()),
                status: "failed".to_string(),
                snapshot_id: None,
                files_processed: 0,
                bytes_transferred: 0,
                error_code: Some(error_code.clone()),
                error_message: Some(error_msg),
            };
            let _ = db.record_schedule_history(&hist);

            let _ = db.update_schedule_status(
                &schedule.schedule_id,
                Some(&next_run.to_rfc3339()),
                Some(&completed_at),
                Some("failed"),
                Some(&error_code),
            );

            Err(SchedulerError::Backup(err))
        }
    }
}

/// Converts a database `ScheduleRecord` into a strongly-typed domain `Schedule`.
fn record_to_domain(record: ScheduleRecord) -> Result<Schedule> {
    let schedule_type: ScheduleType = record.schedule_type.parse()?;
    let timezone: TimezoneStrategy = record.timezone.parse()?;

    let next_run_at = record
        .next_run_at
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let last_run_at = record
        .last_run_at
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let created_at = DateTime::parse_from_rfc3339(&record.created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let updated_at = DateTime::parse_from_rfc3339(&record.updated_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(Schedule {
        schedule_id: record.schedule_id,
        profile_id: record.profile_id,
        schedule_type,
        expression: record.expression,
        timezone,
        enabled: record.enabled,
        next_run_at,
        last_run_at,
        last_status: record.last_status,
        last_error_code: record.last_error_code,
        created_at,
        updated_at,
    })
}

/// Helper generating unique ID string.
fn uuid_or_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", now)
}
