//! Foundational cross-cutting domain enums and models.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Execution status of a backup operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupStatus {
    /// Backup is configured and waiting to start.
    Idle,
    /// Scanning directories and calculating delta manifest.
    Scanning,
    /// Compressing, encrypting, and uploading chunks.
    BackingUp,
    /// Backup successfully verified and finalized.
    Completed,
    /// Backup encountered an unrecoverable failure.
    Failed,
    /// Backup was explicitly cancelled by the user.
    Cancelled,
}

impl fmt::Display for BackupStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Scanning => write!(f, "scanning"),
            Self::BackingUp => write!(f, "backing_up"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// State of an individual chunk or file transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferStatus {
    /// Queued in the persistent transfer queue.
    Pending,
    /// Actively transmitting data over the wire.
    Transferring,
    /// Temporarily paused by scheduler or network backoff.
    Paused,
    /// Successfully transferred and confirmed.
    Completed,
    /// Transfer failed after retries exhausted.
    Failed,
    /// Cancelled by user or job abort.
    Cancelled,
}

impl fmt::Display for TransferStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Transferring => write!(f, "transferring"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Direction of data transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferDirection {
    /// Upload to cloud/storage provider.
    Upload,
    /// Download from cloud/storage provider.
    Download,
}

impl fmt::Display for TransferDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Upload => write!(f, "upload"),
            Self::Download => write!(f, "download"),
        }
    }
}

/// Status of a data restore operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestoreStatus {
    /// Restore job is queued.
    Pending,
    /// Restoring, decrypting, and decompressing files to target disk.
    Restoring,
    /// Restore completed and file integrity verified.
    Completed,
    /// Restore failed.
    Failed,
    /// Restore was cancelled.
    Cancelled,
}

impl fmt::Display for RestoreStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Restoring => write!(f, "restoring"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Operational state of a backup schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleStatus {
    /// Schedule is active and monitoring next run time.
    Active,
    /// Schedule is temporarily paused.
    Paused,
    /// Schedule is currently triggering a job run.
    Executing,
    /// Schedule is disabled.
    Disabled,
}

impl fmt::Display for ScheduleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Executing => write!(f, "executing"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

/// Supported compression algorithms for backup payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgorithm {
    /// No compression applied.
    None,
    /// Zstandard compression (default).
    #[default]
    Zstd,
}

impl fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Zstd => write!(f, "zstd"),
        }
    }
}

/// Supported authenticated encryption algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM authenticated symmetric encryption (default).
    #[default]
    Aes256Gcm,
}

impl fmt::Display for EncryptionAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aes256Gcm => write!(f, "aes256gcm"),
        }
    }
}

/// Global lifecycle state of the TELEVAULT application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppState {
    /// Application runtime is initializing internal subsystems.
    Initializing,
    /// Application is ready and awaiting user or scheduled commands.
    Ready,
    /// Application is actively executing intensive operations.
    Busy,
    /// Application has encountered a fatal subsystem error.
    Error,
    /// Application is gracefully terminating.
    Terminating,
}

impl fmt::Display for AppState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Initializing => write!(f, "initializing"),
            Self::Ready => write!(f, "ready"),
            Self::Busy => write!(f, "busy"),
            Self::Error => write!(f, "error"),
            Self::Terminating => write!(f, "terminating"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_displays() {
        assert_eq!(BackupStatus::BackingUp.to_string(), "backing_up");
        assert_eq!(TransferStatus::Transferring.to_string(), "transferring");
        assert_eq!(TransferDirection::Upload.to_string(), "upload");
        assert_eq!(RestoreStatus::Restoring.to_string(), "restoring");
        assert_eq!(ScheduleStatus::Active.to_string(), "active");
        assert_eq!(CompressionAlgorithm::Zstd.to_string(), "zstd");
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.to_string(), "aes256gcm");
        assert_eq!(AppState::Ready.to_string(), "ready");
    }

    #[test]
    fn test_enum_serialization() {
        let status = BackupStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");
        let deserialized: BackupStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}
