//! Core domain models, errors, path management, and shared configuration for TELEVAULT.

#![deny(missing_docs)]

pub mod config;
pub mod error;
pub mod ids;
pub mod models;
pub mod paths;
pub mod validation;

pub use config::{AppConfig, BackupPreferences, GeneralConfig, StorageConfig, TransferConfig};
pub use error::{AppError, Result};
pub use ids::{ChunkId, FileId, JobId, ManifestId, ProfileId, ScheduleId, SnapshotId, VersionId};
pub use models::{
    AppState, BackupStatus, CompressionAlgorithm, EncryptionAlgorithm, RestoreStatus,
    ScheduleStatus, TransferDirection, TransferStatus,
};
pub use paths::PathManager;
