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

/// Returns whether Telegram MTProto API credentials are configured locally or via env.
#[tauri::command]
#[specta::specta]
pub async fn get_telegram_api_config(
    state: State<'_, DesktopAppState>,
) -> IpcResult<crate::dto::TelegramApiConfigStatusDto> {
    let configured = televault_telegram::TelegramApiCredentials::load_from_env_or_file(
        &state.paths.config_dir(),
    );
    Ok(crate::dto::TelegramApiConfigStatusDto {
        is_configured: configured.is_some(),
        api_id: configured.map(|c| c.api_id),
    })
}

/// Initiates personal Telegram account authentication by requesting a verification code.
#[tauri::command]
#[specta::specta]
pub async fn start_telegram_auth(
    state: State<'_, DesktopAppState>,
    request: StartTelegramAuthDto,
) -> IpcResult<TelegramAuthStatusDto> {
    let phone = request.phone_number.trim();
    if phone.is_empty() || !phone.starts_with('+') || phone.len() < 8 {
        return Err(IpcError::validation(
            "Please enter a valid international phone number starting with '+' (e.g. +1234567890)"
                .to_string(),
        ));
    }

    // Resolve API credentials:
    // Priority: 1. Request parameters -> 2. telegram_api.json / env vars
    let (api_id, api_hash) = match (request.api_id, request.api_hash.as_deref()) {
        (Some(id), Some(hash)) if id > 0 && !hash.trim().is_empty() => {
            let creds = televault_telegram::TelegramApiCredentials {
                api_id: id,
                api_hash: hash.trim().to_string(),
            };
            let _ = creds.save_to_file(&state.paths.config_dir());
            (id, hash.trim().to_string())
        }
        _ => {
            if let Some(loaded) = televault_telegram::TelegramApiCredentials::load_from_env_or_file(
                &state.paths.config_dir(),
            ) {
                (loaded.api_id, loaded.api_hash)
            } else if request.api_id.is_some() && request.api_hash.is_none() {
                return Err(IpcError::validation(
                    "Telegram API Hash is required when specifying custom API ID.".to_string(),
                ));
            } else {
                return Err(IpcError::validation(
                    "Telegram API ID and API Hash are required to connect via MTProto. Please enter your credentials from https://my.telegram.org in the API Credentials section, or set TELEGRAM_API_ID and TELEGRAM_API_HASH.".to_string(),
                ));
            }
        }
    };

    state
        .mtproto_auth
        .start_auth(phone, api_id, &api_hash)
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
