//! Data transfer objects for background scheduler IPC commands.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Parameters for creating a new recurring backup schedule.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateScheduleRequest {
    /// Target backup profile identifier.
    pub profile_id: String,
    /// Schedule type: 'interval', 'daily', 'weekly', 'cron'.
    pub schedule_type: String,
    /// Recurrence expression (e.g. "15m", "02:00", "Sun@03:00", "0 2 * * *").
    pub expression: String,
    /// Timezone strategy: 'local' (default) or 'utc'.
    pub timezone: Option<String>,
    /// Whether the schedule is active upon creation.
    pub enabled: Option<bool>,
}

/// Parameters for updating an existing schedule configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateScheduleRequest {
    /// Target schedule identifier.
    pub schedule_id: String,
    /// New schedule type, if updating.
    pub schedule_type: Option<String>,
    /// New recurrence expression, if updating.
    pub expression: Option<String>,
    /// New timezone strategy, if updating.
    pub timezone: Option<String>,
    /// New enabled state, if updating.
    pub enabled: Option<bool>,
}

/// User-facing representation of a recurring backup schedule.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScheduleDto {
    /// Unique schedule identifier.
    pub schedule_id: String,
    /// Associated backup profile identifier.
    pub profile_id: String,
    /// Schedule recurrence type.
    pub schedule_type: String,
    /// Schedule expression string.
    pub expression: String,
    /// Timezone calculation strategy.
    pub timezone: String,
    /// Whether the schedule is currently enabled.
    pub enabled: bool,
    /// ISO-8601 UTC timestamp of next scheduled execution.
    pub next_run_at: Option<String>,
    /// ISO-8601 UTC timestamp of last executed run.
    pub last_run_at: Option<String>,
    /// Execution status from the last run.
    pub last_status: Option<String>,
    /// Error code if the last run failed.
    pub last_error_code: Option<String>,
    /// Schedule creation timestamp.
    pub created_at: String,
    /// Schedule last update timestamp.
    pub updated_at: String,
}

/// Execution history record for a scheduled backup run.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScheduleHistoryDto {
    /// Unique execution history record identifier.
    pub history_id: String,
    /// Associated schedule identifier.
    pub schedule_id: String,
    /// Associated profile identifier.
    pub profile_id: String,
    /// Execution start timestamp.
    pub started_at: String,
    /// Execution completion timestamp, if finished.
    pub completed_at: Option<String>,
    /// Execution status ('completed', 'failed', 'cancelled', 'skipped').
    pub status: String,
    /// Snapshot identifier if backup completed.
    pub snapshot_id: Option<String>,
    /// Total files processed.
    #[specta(type = specta_typescript::Number)]
    pub files_processed: u64,
    /// Total payload bytes transferred.
    #[specta(type = specta_typescript::Number)]
    pub bytes_transferred: u64,
    /// Error code if execution failed.
    pub error_code: Option<String>,
    /// Safe human-readable error message.
    pub error_message: Option<String>,
}

/// Overall status and health of the scheduler subsystem.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SchedulerStatusDto {
    /// Operational status: 'running', 'stopped', 'paused'.
    pub status: String,
    /// Number of configured enabled schedules.
    pub active_schedules_count: u32,
    /// Profiles currently executing backups.
    pub running_profiles: Vec<String>,
}
