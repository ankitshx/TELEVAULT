//! System and diagnostic IPC commands.

use crate::dto::{AppConfigDto, AppInfoDto, SystemPathsDto};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;
use tauri::State;

/// Returns application metadata and runtime diagnostic information.
#[tauri::command]
#[specta::specta]
pub fn get_app_info() -> IpcResult<AppInfoDto> {
    Ok(AppInfoDto {
        app_name: "TELEVAULT".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        build_mode: if cfg!(debug_assertions) {
            "debug".into()
        } else {
            "release".into()
        },
    })
}

/// Returns the resolved canonical filesystem paths used by TELEVAULT.
#[tauri::command]
#[specta::specta]
pub fn get_system_paths(state: State<'_, DesktopAppState>) -> IpcResult<SystemPathsDto> {
    Ok(SystemPathsDto {
        base_dir: state.paths.base_dir().to_string_lossy().to_string(),
        database_path: state.paths.db_file().to_string_lossy().to_string(),
        temp_dir: state.paths.temp_dir().to_string_lossy().to_string(),
        logs_dir: state.paths.logs_dir().to_string_lossy().to_string(),
        config_dir: state.paths.config_dir().to_string_lossy().to_string(),
    })
}

/// Returns the current application configuration preferences from disk or default.
#[tauri::command]
#[specta::specta]
pub fn get_app_config(state: State<'_, DesktopAppState>) -> IpcResult<AppConfigDto> {
    let config_path = state.paths.config_file();
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            IpcError::new(
                "FILESYSTEM_ERROR",
                format!("Failed to read config file: {e}"),
            )
        })?;
        let config: televault_core::config::AppConfig = serde_json::from_str(&content)
            .map_err(|e| IpcError::internal(format!("Failed to parse config file: {e}")))?;
        Ok(config.into())
    } else {
        Ok(televault_core::config::AppConfig::default().into())
    }
}

/// Validates, persists, and returns updated application configuration preferences.
#[tauri::command]
#[specta::specta]
pub fn update_app_config(
    state: State<'_, DesktopAppState>,
    request: AppConfigDto,
) -> IpcResult<AppConfigDto> {
    let config: televault_core::config::AppConfig = request.try_into()?;
    config
        .validate()
        .map_err(|e| IpcError::validation(e.to_string()))?;

    let config_dir = state.paths.config_dir();
    std::fs::create_dir_all(&config_dir).map_err(|e| {
        IpcError::new(
            "FILESYSTEM_ERROR",
            format!("Failed to create config directory: {e}"),
        )
    })?;

    let json_bytes = serde_json::to_string_pretty(&config)
        .map_err(|e| IpcError::internal(format!("Failed to serialize config: {e}")))?;

    let target_file = state.paths.config_file();
    let temp_file = target_file.with_extension("tmp");

    std::fs::write(&temp_file, json_bytes.as_bytes()).map_err(|e| {
        IpcError::new(
            "FILESYSTEM_ERROR",
            format!("Failed to write temporary config file: {e}"),
        )
    })?;

    std::fs::rename(&temp_file, &target_file).map_err(|e| {
        // Clean up temp file on rename failure
        let _ = std::fs::remove_file(&temp_file);
        IpcError::new(
            "FILESYSTEM_ERROR",
            format!("Failed to commit config file atomically: {e}"),
        )
    })?;

    Ok(config.into())
}

/// Returns the deterministic startup recovery and state reconciliation diagnostic report.
#[tauri::command]
#[specta::specta]
pub fn get_startup_recovery_report(
    state: State<'_, DesktopAppState>,
) -> IpcResult<crate::dto::StartupRecoveryReportDto> {
    Ok((*state.startup_recovery_report).clone())
}
