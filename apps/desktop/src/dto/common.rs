//! Common application information and path DTOs.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Diagnostic metadata describing the running TELEVAULT application instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppInfoDto {
    /// Canonical application name.
    pub app_name: String,
    /// Semantic version of the build.
    pub version: String,
    /// Operating system platform.
    pub platform: String,
    /// Architecture indicator.
    pub arch: String,
    /// Release or debug build mode.
    pub build_mode: String,
}

/// Standardized resolved system directories managed by PathManager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SystemPathsDto {
    /// Root data directory (e.g. %LOCALAPPDATA%\TELEVAULT).
    pub base_dir: String,
    /// Path to embedded SQLite database.
    pub database_path: String,
    /// Path to temporary payload staging directory.
    pub temp_dir: String,
    /// Path to application logs directory.
    pub logs_dir: String,
    /// Path to application config directory.
    pub config_dir: String,
}

/// Diagnostic report generated during deterministic startup recovery and state reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StartupRecoveryReportDto {
    /// Whether SQLite database integrity check reported healthy ("ok").
    pub database_integrity_ok: bool,
    /// Detailed messages from database integrity check.
    pub database_integrity_details: Vec<String>,
    /// Number of foreign key constraint violations detected (0 = clean).
    pub foreign_key_violations: u32,
    /// Detailed foreign key violation descriptions.
    pub foreign_key_violation_details: Vec<String>,
    /// Number of stale snapshots reconciled from transient states (e.g. BackingUp/Scanning) to Failed.
    pub reconciled_snapshots_count: u32,
    /// Number of stale transfer jobs reconciled from Transferring to Failed.
    pub reconciled_transfers_count: u32,
    /// Number of orphaned temporary staging files purged from disk.
    pub purged_staging_files_count: u32,
    /// ISO 8601 timestamp when recovery execution completed.
    pub recovered_at: String,
}
