//! Telegram cloud storage configuration and diagnostic IPC commands.

use crate::dto::{SaveTelegramConfigDto, TelegramConnectionTestResultDto, TelegramStatusDto};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;
use tauri::State;

/// Returns current safe Telegram storage connection and configuration status.
#[tauri::command]
#[specta::specta]
pub fn get_telegram_status(state: State<'_, DesktopAppState>) -> IpcResult<TelegramStatusDto> {
    Ok(state.get_telegram_status())
}

/// Validates, securely persists, and activates Telegram connection settings.
#[tauri::command]
#[specta::specta]
pub fn save_telegram_config(
    state: State<'_, DesktopAppState>,
    request: SaveTelegramConfigDto,
) -> IpcResult<TelegramStatusDto> {
    state
        .save_telegram_config(request)
        .map_err(|e| IpcError::validation(e.to_string()))
}

/// Executes a safe Telegram connection test without creating payloads or modifying retention.
#[tauri::command]
#[specta::specta]
pub async fn test_telegram_connection(
    state: State<'_, DesktopAppState>,
) -> IpcResult<TelegramConnectionTestResultDto> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        state
            .test_telegram_connection()
            .map_err(|e| IpcError::internal(e.to_string()))
    })
    .await
    .map_err(|e| IpcError::internal(format!("Task execution failed: {e}")))?
}

/// Disconnects Telegram configuration and falls back to mock storage.
#[tauri::command]
#[specta::specta]
pub fn disconnect_telegram(state: State<'_, DesktopAppState>) -> IpcResult<TelegramStatusDto> {
    state
        .disconnect_telegram()
        .map_err(|e| IpcError::internal(e.to_string()))
}
