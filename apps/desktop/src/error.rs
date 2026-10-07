//! Structured IPC error definitions and mappings for TELEVAULT Tauri IPC.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Universal structured error DTO returned across the Tauri IPC boundary.
///
/// Ensures stable, typed error codes for React to branch upon without parsing English strings.
/// Secrets, keys, and raw internal panic messages are strictly excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct IpcError {
    /// Stable machine-readable error code (e.g. "VALIDATION_ERROR", "NOT_FOUND").
    pub code: String,
    /// Human-readable diagnostic description safe for UI presentation.
    pub message: String,
    /// Optional contextual detail (e.g. affected identifier or field name).
    pub details: Option<String>,
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for IpcError {}

impl IpcError {
    /// Creates a new [`IpcError`] with code and message.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Appends optional diagnostic detail.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Convenience constructor for user input validation errors.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("VALIDATION_ERROR", message)
    }

    /// Convenience constructor for entity not found errors.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", message)
    }

    /// Convenience constructor for state conflict errors.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("CONFLICT", message)
    }

    /// Convenience constructor for cancelled operations.
    pub fn cancelled() -> Self {
        Self::new("CANCELLED", "The operation was cancelled by user request")
    }

    /// Convenience constructor for path safety violations.
    pub fn unsafe_path(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new("UNSAFE_PATH", reason).with_details(path)
    }

    /// Convenience constructor for internal errors.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("INTERNAL_ERROR", message)
    }
}

pub type IpcResult<T> = Result<T, IpcError>;

// Conversions from domain errors to IpcError

impl From<televault_core::error::AppError> for IpcError {
    fn from(err: televault_core::error::AppError) -> Self {
        match err {
            televault_core::error::AppError::NotFound { entity, id } => {
                Self::new("NOT_FOUND", format!("{entity} with id '{id}' not found"))
                    .with_details(id)
            }
            televault_core::error::AppError::InvalidState { expected, current } => Self::new(
                "INVALID_STATE",
                format!("Expected state '{expected}', but was '{current}'"),
            ),
            televault_core::error::AppError::Io(e) => Self::new("FILESYSTEM_ERROR", e.to_string()),
            televault_core::error::AppError::Path(p) => {
                Self::new("UNSAFE_PATH", "Path error").with_details(p)
            }
            other => Self::new("INTERNAL_ERROR", other.to_string()),
        }
    }
}

impl From<televault_backup::BackupError> for IpcError {
    fn from(err: televault_backup::BackupError) -> Self {
        match err {
            televault_backup::BackupError::ProfileNotFound(id) => {
                Self::new("NOT_FOUND", format!("Backup profile '{id}' not found")).with_details(id)
            }
            televault_backup::BackupError::InvalidProfile(msg) => {
                Self::new("VALIDATION_ERROR", msg)
            }
            televault_backup::BackupError::Cancelled => Self::cancelled(),
            televault_backup::BackupError::PathError(msg) => Self::new("UNSAFE_PATH", msg),
            televault_backup::BackupError::InaccessibleFile { path, reason } => {
                Self::new("FILESYSTEM_ERROR", reason).with_details(path)
            }
            televault_backup::BackupError::ScanError(msg) => Self::new("FILESYSTEM_ERROR", msg),
            televault_backup::BackupError::SnapshotError(msg) => Self::new("SNAPSHOT_ERROR", msg),
            televault_backup::BackupError::Io(e) => Self::new("FILESYSTEM_ERROR", e),
            televault_backup::BackupError::DatabaseError(e) => Self::new("DATABASE_ERROR", e),
            televault_backup::BackupError::ManifestError(e) => Self::new("MANIFEST_ERROR", e),
            televault_backup::BackupError::CryptoError(e) => Self::new("CRYPTO_ERROR", e),
            televault_backup::BackupError::TransferError(e) => Self::new("TRANSFER_ERROR", e),
            televault_backup::BackupError::StorageError(e) => Self::new("STORAGE_ERROR", e),
        }
    }
}

impl From<televault_backup::RestoreError> for IpcError {
    fn from(err: televault_backup::RestoreError) -> Self {
        match err {
            televault_backup::RestoreError::Cancelled => Self::cancelled(),
            televault_backup::RestoreError::DecryptionFailed { chunk_id, reason } => Self::new(
                "DECRYPTION_FAILED",
                format!("Decryption failed for chunk {chunk_id}: {reason}"),
            )
            .with_details(chunk_id),
            televault_backup::RestoreError::DecompressionFailed { chunk_id, reason } => Self::new(
                "DECOMPRESSION_FAILED",
                format!("Decompression failed for chunk {chunk_id}: {reason}"),
            )
            .with_details(chunk_id),
            televault_backup::RestoreError::IntegrityMismatch {
                file_id,
                expected,
                actual,
            } => Self::new(
                "INTEGRITY_MISMATCH",
                format!("Integrity check failed: expected {expected}, calculated {actual}"),
            )
            .with_details(file_id),
            televault_backup::RestoreError::ChunkIntegrityMismatch {
                chunk_id,
                expected,
                actual,
            } => Self::new(
                "CHUNK_INTEGRITY_MISMATCH",
                format!("Chunk integrity failed: expected {expected}, calculated {actual}"),
            )
            .with_details(chunk_id),
            televault_backup::RestoreError::SizeMismatch {
                file_id,
                expected,
                actual,
            } => Self::new(
                "SIZE_MISMATCH",
                format!("Size mismatch: expected {expected} bytes, calculated {actual} bytes"),
            )
            .with_details(file_id),
            televault_backup::RestoreError::IncompleteBackup { reason } => {
                Self::new("INCOMPLETE_BACKUP", reason)
            }
            televault_backup::RestoreError::PendingChunk { chunk_id, index } => Self::new(
                "INCOMPLETE_BACKUP",
                format!("Chunk {index} ({chunk_id}) is still pending"),
            )
            .with_details(chunk_id),
            televault_backup::RestoreError::MissingChunk { chunk_id, index } => Self::new(
                "MISSING_CHUNK",
                format!("Chunk {index} ({chunk_id}) is missing"),
            )
            .with_details(chunk_id),
            televault_backup::RestoreError::RemoteObjectUnavailable { reference } => Self::new(
                "STORAGE_ERROR",
                format!("Remote object unavailable: {reference:?}"),
            ),
            televault_backup::RestoreError::DestinationConflict { path } => Self::new(
                "DESTINATION_CONFLICT",
                format!("Target file already exists: {path}"),
            )
            .with_details(path),
            televault_backup::RestoreError::UnsafePath { path, reason } => {
                Self::new("UNSAFE_PATH", reason).with_details(path)
            }
            televault_backup::RestoreError::FileNotFound(f) => {
                Self::new("NOT_FOUND", format!("File '{f}' not found in catalog")).with_details(f)
            }
            televault_backup::RestoreError::VersionNotFound(v) => {
                Self::new("NOT_FOUND", format!("Version '{v}' not found in catalog"))
                    .with_details(v)
            }
            televault_backup::RestoreError::SnapshotNotFound(s) => {
                Self::new("NOT_FOUND", format!("Snapshot '{s}' not found in catalog"))
                    .with_details(s)
            }
            televault_backup::RestoreError::EncryptionMismatch { reason } => {
                Self::new("ENCRYPTION_MISMATCH", reason)
            }
            televault_backup::RestoreError::Io(e) => Self::new("FILESYSTEM_ERROR", e),
            televault_backup::RestoreError::Manifest(e) => {
                Self::new("MANIFEST_ERROR", e.to_string())
            }
            televault_backup::RestoreError::Database(e) => e.into(),
            televault_backup::RestoreError::Storage(e) => Self::new("STORAGE_ERROR", e.to_string()),
            televault_backup::RestoreError::Transfer(e) => e.into(),
            televault_backup::RestoreError::Crypto(e) => Self::new("CRYPTO_ERROR", e.to_string()),
        }
    }
}

impl From<televault_db::DbError> for IpcError {
    fn from(err: televault_db::DbError) -> Self {
        match err {
            televault_db::DbError::NotFound { entity, id } => {
                Self::new("NOT_FOUND", format!("{entity} with id '{id}' not found"))
                    .with_details(id)
            }
            televault_db::DbError::Constraint(msg) => Self::new("CONFLICT", msg),
            televault_db::DbError::Sql(e) => Self::new("DATABASE_ERROR", e.to_string()),
            televault_db::DbError::Connection(e) => Self::new("DATABASE_ERROR", e),
            televault_db::DbError::Migration(e) => Self::new("DATABASE_ERROR", e),
            televault_db::DbError::Serialization(e) => {
                Self::new("SERIALIZATION_ERROR", e.to_string())
            }
            televault_db::DbError::Manifest(e) => Self::new("MANIFEST_ERROR", e.to_string()),
            televault_db::DbError::InvalidData(e) => Self::new("DATABASE_ERROR", e),
            televault_db::DbError::Transaction(e) => Self::new("DATABASE_ERROR", e),
            televault_db::DbError::LockPoisoned(e) => Self::new("INTERNAL_ERROR", e),
        }
    }
}

impl From<televault_transfer::error::TransferError> for IpcError {
    fn from(err: televault_transfer::error::TransferError) -> Self {
        match err {
            televault_transfer::error::TransferError::Cancelled => Self::cancelled(),
            televault_transfer::error::TransferError::InvalidJob(msg) => {
                Self::new("VALIDATION_ERROR", msg)
            }
            televault_transfer::error::TransferError::VerificationFailed(msg) => {
                Self::new("INTEGRITY_MISMATCH", msg)
            }
            televault_transfer::error::TransferError::Storage(s) => {
                Self::new("STORAGE_ERROR", s.to_string())
            }
            televault_transfer::error::TransferError::Io(e) => {
                Self::new("FILESYSTEM_ERROR", e.to_string())
            }
            televault_transfer::error::TransferError::RetriesExhausted {
                attempts,
                last_error,
            } => Self::new(
                "RETRIES_EXHAUSTED",
                format!("Failed after {attempts} attempts: {last_error}"),
            ),
            televault_transfer::error::TransferError::Database(db_err) => db_err.into(),
            other => Self::new("TRANSFER_ERROR", other.to_string()),
        }
    }
}

impl From<televault_scheduler::SchedulerError> for IpcError {
    fn from(err: televault_scheduler::SchedulerError) -> Self {
        match err {
            televault_scheduler::SchedulerError::Database(e) => e.into(),
            televault_scheduler::SchedulerError::Backup(e) => e.into(),
            televault_scheduler::SchedulerError::Core(e) => e.into(),
            televault_scheduler::SchedulerError::ScheduleNotFound(id) => Self::new(
                "SCHEDULE_NOT_FOUND",
                format!("Schedule with ID '{id}' was not found"),
            )
            .with_details(id),
            televault_scheduler::SchedulerError::ProfileNotFound(id) => Self::new(
                "PROFILE_NOT_FOUND",
                format!("Profile with ID '{id}' was not found"),
            )
            .with_details(id),
            televault_scheduler::SchedulerError::InvalidSchedule(msg) => {
                Self::new("INVALID_SCHEDULE", msg)
            }
            televault_scheduler::SchedulerError::InvalidExpression(msg) => {
                Self::new("INVALID_EXPRESSION", msg)
            }
            televault_scheduler::SchedulerError::ScheduleConflict(msg) => {
                Self::new("SCHEDULE_CONFLICT", msg)
            }
            televault_scheduler::SchedulerError::ProfileAlreadyRunning(id) => Self::new(
                "SCHEDULE_ALREADY_RUNNING",
                format!("Backup for profile '{id}' is already running"),
            )
            .with_details(id),
            televault_scheduler::SchedulerError::Cancelled => Self::cancelled(),
            televault_scheduler::SchedulerError::Unavailable(msg) => {
                Self::new("SCHEDULER_UNAVAILABLE", msg)
            }
            televault_scheduler::SchedulerError::Internal(msg) => Self::internal(msg),
        }
    }
}

impl From<televault_backup::RetentionError> for IpcError {
    fn from(err: televault_backup::RetentionError) -> Self {
        match err {
            televault_backup::RetentionError::ProfileNotFound(id) => Self::new(
                "PROFILE_NOT_FOUND",
                format!("Profile with ID '{id}' was not found"),
            )
            .with_details(id),
            televault_backup::RetentionError::SnapshotNotFound(id) => Self::new(
                "SNAPSHOT_NOT_FOUND",
                format!("Snapshot with ID '{id}' was not found"),
            )
            .with_details(id),
            televault_backup::RetentionError::InvalidPolicy(msg) => {
                Self::new("INVALID_POLICY", msg)
            }
            televault_backup::RetentionError::ProfileBusy(msg) => {
                Self::new("OPERATION_CONFLICT", msg)
            }
            televault_backup::RetentionError::SnapshotProtected(id, reason) => Self::new(
                "SNAPSHOT_PROTECTED",
                format!("Snapshot '{id}' is protected: {reason}"),
            )
            .with_details(id),
            televault_backup::RetentionError::Database(e) => Self::new("DATABASE_ERROR", e),
            televault_backup::RetentionError::Cancelled => Self::cancelled(),
            televault_backup::RetentionError::Internal(msg) => Self::internal(msg),
        }
    }
}
