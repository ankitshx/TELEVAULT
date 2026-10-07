//! Typed scheduler domain errors and result aliases.

use thiserror::Error;

/// Structured error type for scheduler and automated backup operations.
#[derive(Debug, Error)]
pub enum SchedulerError {
    /// Underlying database operation failed.
    #[error("Database error: {0}")]
    Database(#[from] televault_db::DbError),

    /// Underlying backup engine operation failed.
    #[error("Backup engine error: {0}")]
    Backup(#[from] televault_backup::error::BackupError),

    /// Domain core error.
    #[error("Core domain error: {0}")]
    Core(#[from] televault_core::error::AppError),

    /// Schedule was not found in catalog.
    #[error("Schedule '{0}' not found")]
    ScheduleNotFound(String),

    /// Profile referenced by schedule does not exist.
    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),

    /// Schedule definition or configuration is invalid.
    #[error("Invalid schedule definition: {0}")]
    InvalidSchedule(String),

    /// Schedule expression is malformed or invalid.
    #[error("Invalid schedule expression: {0}")]
    InvalidExpression(String),

    /// Schedule conflict detected.
    #[error("Schedule conflict: {0}")]
    ScheduleConflict(String),

    /// Backup for this profile is already running.
    #[error("Backup for profile '{0}' is already running")]
    ProfileAlreadyRunning(String),

    /// Operation was cancelled.
    #[error("Scheduler operation cancelled")]
    Cancelled,

    /// Scheduler service is unavailable or stopped.
    #[error("Scheduler service is unavailable: {0}")]
    Unavailable(String),

    /// Internal scheduler runtime error.
    #[error("Internal scheduler error: {0}")]
    Internal(String),
}

/// Convenience result alias for scheduler operations.
pub type Result<T> = std::result::Result<T, SchedulerError>;

impl From<SchedulerError> for televault_core::error::AppError {
    fn from(err: SchedulerError) -> Self {
        match err {
            SchedulerError::Core(e) => e,
            SchedulerError::Database(e) => televault_core::error::AppError::from(e),
            SchedulerError::Backup(e) => televault_core::error::AppError::from(e),
            SchedulerError::Cancelled => televault_core::error::AppError::Internal(
                "Scheduler operation cancelled".to_string(),
            ),
            SchedulerError::ScheduleNotFound(id) => televault_core::error::AppError::NotFound {
                entity: "Schedule",
                id,
            },
            SchedulerError::ProfileNotFound(id) => televault_core::error::AppError::NotFound {
                entity: "Profile",
                id,
            },
            SchedulerError::InvalidSchedule(message) => {
                televault_core::error::AppError::Validation {
                    field: "schedule",
                    message,
                }
            }
            SchedulerError::InvalidExpression(message) => {
                televault_core::error::AppError::Validation {
                    field: "expression",
                    message,
                }
            }
            SchedulerError::ScheduleConflict(reason) => televault_core::error::AppError::Conflict {
                entity: "Schedule",
                reason,
            },
            SchedulerError::ProfileAlreadyRunning(id) => {
                televault_core::error::AppError::Conflict {
                    entity: "Profile",
                    reason: format!("Backup already running for profile {id}"),
                }
            }
            SchedulerError::Unavailable(msg) | SchedulerError::Internal(msg) => {
                televault_core::error::AppError::Internal(msg)
            }
        }
    }
}
