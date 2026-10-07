//! System and diagnostic IPC commands.

use crate::dto::{AppInfoDto, SystemPathsDto};
use crate::error::IpcResult;
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
