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
