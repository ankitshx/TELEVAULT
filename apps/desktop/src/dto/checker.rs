//! Backup checker and verification request/response DTOs.

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;

/// Request to query backup status of a specific local file within a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CheckFileStatusRequest {
    /// Profile identifier owning the file.
    pub profile_id: String,
    /// Relative path within the profile directory.
    pub relative_path: String,
}

/// Status report indicating whether a file is backed up and needs incremental upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileStatusDto {
    pub profile_id: String,
    pub relative_path: String,
    pub is_backed_up: bool,
    pub file_id: Option<String>,
    pub latest_version_id: Option<String>,
    pub incremental_needed: bool,
}

/// Historical version record for a backed-up file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileVersionDto {
    pub version_id: String,
    pub file_id: String,
    pub snapshot_id: String,
    pub manifest_id: String,
    pub status: String,
    pub created_at: String,
}

/// Request to verify metadata of a manifest by FileId.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct VerifyManifestRequest {
    pub file_id: String,
}

/// Lightweight fast metadata verification report (zero full payload download).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ManifestVerificationReportDto {
    pub file_id: String,
    pub relative_path: String,
    pub is_restorable: bool,
    #[specta(type = Number)]
    pub total_chunks: usize,
    #[specta(type = Number)]
    pub original_size: u64,
    pub remote_objects_verified: bool,
    pub issues: Vec<String>,
}

/// Full end-to-end trial restore verification report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FullVerificationReportDto {
    pub file_id: String,
    pub verified: bool,
    pub sha256_match: bool,
    pub size_match: bool,
    #[specta(type = Number)]
    pub duration_ms: u64,
    pub error: Option<String>,
}
