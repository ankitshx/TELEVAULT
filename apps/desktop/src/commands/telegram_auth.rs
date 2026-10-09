//! Telegram personal account MTProto authentication and channel setup IPC commands.

use crate::dto::{
    SetupChannelDto, StartTelegramAuthDto, SubmitAuthCodeDto, SubmitAuthPasswordDto,
    TelegramAccountInfoDto, TelegramAuthStatusDto,
};
use crate::error::{IpcError, IpcResult};
use crate::state::DesktopAppState;
use tauri::State;

/// Returns current personal Telegram authentication and dedicated backup channel setup status.
#[tauri::command]
#[specta::specta]
pub async fn get_telegram_auth_status(
    state: State<'_, DesktopAppState>,
) -> IpcResult<TelegramAuthStatusDto> {
    Ok(state.get_telegram_auth_status().await)
}

/// Initiates personal Telegram account authentication by requesting a verification code.
#[tauri::command]
#[specta::specta]
pub async fn start_telegram_auth(
    state: State<'_, DesktopAppState>,
    request: StartTelegramAuthDto,
) -> IpcResult<TelegramAuthStatusDto> {
    let api_id = request.api_id.unwrap_or(2040);
    let api_hash = request
        .api_hash
        .unwrap_or_else(|| "b18441a1ff607e10a989891a5462e627".to_string());

    state
        .mtproto_auth
        .start_auth(&request.phone_number, api_id, &api_hash)
        .await
        .map_err(|e| IpcError::validation(e.to_string()))?;

    Ok(state.get_telegram_auth_status().await)
}

/// Submits the verification code received via Telegram.
#[tauri::command]
#[specta::specta]
pub async fn submit_telegram_auth_code(
    state: State<'_, DesktopAppState>,
    request: SubmitAuthCodeDto,
) -> IpcResult<TelegramAuthStatusDto> {
    state
        .mtproto_auth
        .submit_code(&request.code)
        .await
        .map_err(|e| IpcError::validation(e.to_string()))?;

    Ok(state.get_telegram_auth_status().await)
}

/// Submits the Two-Step Verification (2FA) cloud password.
#[tauri::command]
#[specta::specta]
pub async fn submit_telegram_auth_password(
    state: State<'_, DesktopAppState>,
    request: SubmitAuthPasswordDto,
) -> IpcResult<TelegramAuthStatusDto> {
    state
        .mtproto_auth
        .submit_password(&request.password)
        .await
        .map_err(|e| IpcError::validation(e.to_string()))?;

    Ok(state.get_telegram_auth_status().await)
}

/// Cancels an in-progress authentication attempt and safely resets state.
#[tauri::command]
#[specta::specta]
pub async fn cancel_telegram_auth(
    state: State<'_, DesktopAppState>,
) -> IpcResult<TelegramAuthStatusDto> {
    let _ = state.mtproto_auth.cancel_auth().await;
    Ok(state.get_telegram_auth_status().await)
}

/// Sets up the private backup channel (auto-creates if channel_id is None, or verifies manual channel).
#[tauri::command]
#[specta::specta]
pub async fn setup_backup_channel(
    state: State<'_, DesktopAppState>,
    request: SetupChannelDto,
) -> IpcResult<TelegramAuthStatusDto> {
    state
        .mtproto_auth
        .setup_backup_channel(request.channel_id)
        .await
        .map_err(|e| IpcError::validation(e.to_string()))?;

    Ok(state.get_telegram_auth_status().await)
}

/// Explicitly verifies connectivity and permissions on the configured private backup channel.
#[tauri::command]
#[specta::specta]
pub async fn verify_backup_channel(
    state: State<'_, DesktopAppState>,
) -> IpcResult<TelegramAuthStatusDto> {
    state
        .mtproto_auth
        .verify_backup_channel()
        .await
        .map_err(|e| IpcError::validation(e.to_string()))?;

    Ok(state.get_telegram_auth_status().await)
}

/// Logs out of Telegram, purges session material from disk, and locks the application gate.
#[tauri::command]
#[specta::specta]
pub async fn logout_telegram(
    state: State<'_, DesktopAppState>,
) -> IpcResult<TelegramAuthStatusDto> {
    let _ = state.db.clear_telegram_auth_data();
    state
        .mtproto_auth
        .logout()
        .await
        .map_err(|e| IpcError::internal(e.to_string()))?;

    Ok(state.get_telegram_auth_status().await)
}

/// Retrieves safe personal account details if currently authenticated.
#[tauri::command]
#[specta::specta]
pub async fn get_account_info(
    state: State<'_, DesktopAppState>,
) -> IpcResult<Option<TelegramAccountInfoDto>> {
    let status = state.mtproto_auth.status().await;
    Ok(status.account.map(Into::into))
}
