//! Backup domain request and response DTOs.

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;

/// Frontend request to register a new backup profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CreateProfileRequest {
    /// Unique profile identifier (e.g. "prof-docs").
    pub profile_id: String,
    /// Human-friendly display label (e.g. "Work Documents").
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Absolute local filesystem path to the folder to back up.
    pub source_path: String,
}

/// Frontend request to modify an existing backup profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct UpdateProfileRequest {
    /// Identifier of the profile to update.
    pub profile_id: String,
    /// New human-friendly label if modifying.
    pub name: Option<String>,
    /// Optional description if modifying.
    pub description: Option<String>,
    /// New source directory path if modifying.
    pub source_path: Option<String>,
    /// Enable or disable profile.
    pub enabled: Option<bool>,
}

/// Stable DTO representing a configured backup profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct BackupProfileDto {
    pub profile_id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_path: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Request to initiate a point-in-time snapshot backup for a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StartBackupRequest {
    /// Profile identifier to back up.
    pub profile_id: String,
    /// Optional encryption passphrase if profile has encryption enabled.
    pub passphrase: Option<String>,
}

/// Summary results of an executed backup operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct BackupSummaryDto {
    pub snapshot_id: String,
    pub profile_id: String,
    pub status: String,
    #[specta(type = Number)]
    pub new_files: usize,
    #[specta(type = Number)]
    pub modified_files: usize,
    #[specta(type = Number)]
    pub unchanged_files: usize,
    #[specta(type = Number)]
    pub deleted_files: usize,
    #[specta(type = Number)]
    pub transferred_chunks: usize,
    #[specta(type = Number)]
    pub transferred_bytes: u64,
    #[specta(type = Number)]
    pub reused_bytes: u64,
    #[specta(type = Number)]
    pub elapsed_ms: u64,
    pub error_message: Option<String>,
}

/// Historical snapshot metadata record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnapshotDto {
    pub snapshot_id: String,
    pub profile_id: String,
    pub status: String,
    pub metadata: Option<String>,
    pub created_at: String,
}

/// File item within a specific historical snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnapshotFileDto {
    pub file_id: String,
    pub relative_path: String,
    #[specta(type = Number)]
    pub size_bytes: u64,
    pub status: String,
    pub mime_type: Option<String>,
}
