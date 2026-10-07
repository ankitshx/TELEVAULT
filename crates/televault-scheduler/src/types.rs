//! Core scheduler domain models and configuration types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use televault_core::ids::{ProfileId, ScheduleId};

use crate::error::SchedulerError;

/// Recurrence pattern model for schedules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleType {
    /// Fixed interval between backup executions (e.g. "15m", "1h", "24h").
    Interval,
    /// Daily execution at a specific clock time (e.g. "02:00").
    Daily,
    /// Weekly execution on a designated weekday and clock time (e.g. "Sun@03:00").
    Weekly,
    /// Standard 5-field cron expression (e.g. "0 2 * * *").
    Cron,
}

impl fmt::Display for ScheduleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interval => write!(f, "interval"),
            Self::Daily => write!(f, "daily"),
            Self::Weekly => write!(f, "weekly"),
            Self::Cron => write!(f, "cron"),
        }
    }
}

impl FromStr for ScheduleType {
    type Err = SchedulerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "interval" => Ok(Self::Interval),
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "cron" => Ok(Self::Cron),
            other => Err(SchedulerError::InvalidSchedule(format!(
                "Unknown schedule type '{other}'. Expected: interval, daily, weekly, cron"
            ))),
        }
    }
}

/// Timezone calculation strategy for scheduled execution times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimezoneStrategy {
    /// Local system timezone (default for desktop experience, DST-aware).
    #[default]
    Local,
    /// Coordinated Universal Time (UTC).
    Utc,
}

impl fmt::Display for TimezoneStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Utc => write!(f, "utc"),
        }
    }
}

impl FromStr for TimezoneStrategy {
    type Err = SchedulerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "utc" => Ok(Self::Utc),
            other => Err(SchedulerError::InvalidSchedule(format!(
                "Unknown timezone strategy '{other}'. Expected: local, utc"
            ))),
        }
    }
}

/// Policy dictating behavior when a scheduled run is missed during application downtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MissedSchedulePolicy {
    /// Execute at most one catch-up backup if missed within the catch-up horizon.
    #[default]
    RunOnceIfMissed,
    /// Skip missed runs and wait for the next future scheduled occurrence.
    SkipToNext,
}

/// Authoritative domain representation of a recurring backup schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    /// Unique schedule identifier.
    pub schedule_id: ScheduleId,
    /// Target backup profile identifier.
    pub profile_id: ProfileId,
    /// Recurrence pattern type.
    pub schedule_type: ScheduleType,
    /// Schedule expression string.
    pub expression: String,
    /// Timezone strategy.
    pub timezone: TimezoneStrategy,
    /// Whether the schedule is active.
    pub enabled: bool,
    /// Next calculated execution time in UTC.
    pub next_run_at: Option<DateTime<Utc>>,
    /// Last executed run timestamp in UTC.
    pub last_run_at: Option<DateTime<Utc>>,
    /// Status result of the last execution.
    pub last_status: Option<String>,
    /// Error code of the last execution if failed.
    pub last_error_code: Option<String>,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp in UTC.
    pub updated_at: DateTime<Utc>,
}

/// Parameters required to create a new recurring backup schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScheduleParams {
    /// Unique schedule identifier (or auto-generated if None).
    pub schedule_id: Option<ScheduleId>,
    /// Target backup profile identifier.
    pub profile_id: ProfileId,
    /// Recurrence pattern type.
    pub schedule_type: ScheduleType,
    /// Schedule expression string.
    pub expression: String,
    /// Timezone strategy.
    pub timezone: Option<TimezoneStrategy>,
    /// Whether the schedule is initially active (defaults to true).
    pub enabled: Option<bool>,
}

/// Parameters for updating an existing recurring schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateScheduleParams {
    /// Target schedule identifier.
    pub schedule_id: ScheduleId,
    /// New schedule pattern type, if changing.
    pub schedule_type: Option<ScheduleType>,
    /// New schedule expression string, if changing.
    pub expression: Option<String>,
    /// New timezone strategy, if changing.
    pub timezone: Option<TimezoneStrategy>,
    /// New active status, if changing.
    pub enabled: Option<bool>,
}

/// Operational state of the background scheduler service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchedulerServiceStatus {
    /// Scheduler is actively monitoring schedules.
    Running,
    /// Scheduler is stopped and idle.
    Stopped,
    /// Scheduler is temporarily paused.
    Paused,
}

impl fmt::Display for SchedulerServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Paused => write!(f, "paused"),
        }
    }
}
