//! Data Transfer Objects for secure Telegram cloud storage configuration and testing.

use serde::{Deserialize, Serialize};
use specta_typescript::Number;

/// High-level Telegram cloud connection health status for the desktop UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum TelegramConnectionStatus {
    /// Telegram credentials are not configured.
    NotConfigured,
    /// Credentials configured locally but not yet validated against Telegram servers.
    Configured,
    /// Connection handshake or test currently in progress.
    Connecting,
    /// Connection verified successfully; credentials and chat access confirmed.
    Connected,
    /// Connection test failed or service unreachable.
    ConnectionFailed,
    /// Transient network reconnect attempt in progress.
    Reconnecting,
}

/// Safe Telegram status DTO exposed to the UI without sensitive credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TelegramStatusDto {
    /// Whether valid Telegram credentials exist on the local machine.
    pub is_configured: bool,
    /// High-level connection health status.
    pub status: TelegramConnectionStatus,
    /// Configured target chat or channel ID.
    #[specta(type = Option<Number>)]
    pub target_chat_id: Option<i64>,
    /// Confirmed bot username (e.g., "TeleVaultBackupBot").
    pub bot_username: Option<String>,
    /// ISO-8601 timestamp when connection was last tested.
    pub last_tested_at: Option<String>,
    /// Sanitized, safe error message if last connection test or transfer failed.
    pub last_error: Option<String>,
    /// Name of the active storage provider backend currently processing transfers.
    pub active_backend: String,
}

/// Request payload submitted by the UI to configure Telegram credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SaveTelegramConfigDto {
    /// Secret Telegram Bot API token (never echoed back in status responses).
    pub bot_token: String,
    /// Target chat or channel ID where backups will be stored.
    #[specta(type = Number)]
    pub target_chat_id: i64,
    /// Optional custom Bot API endpoint (for self-hosted or local Bot API servers).
    pub api_endpoint: Option<String>,
}

/// Result returned from an explicit connection test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TelegramConnectionTestResultDto {
    /// Whether connection and chat accessibility succeeded.
    pub success: bool,
    /// Bot username verified from Telegram.
    pub bot_username: Option<String>,
    /// Bot numerical user ID verified from Telegram.
    #[specta(type = Option<Number>)]
    pub bot_id: Option<i64>,
    /// Title or username of the target channel/chat.
    pub chat_title: Option<String>,
    /// Sanitized, safe error message if the connection test failed.
    pub error_message: Option<String>,
}

/// High-level authentication and authorization states for personal Telegram account integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AuthStateDto {
    /// Initializing session state.
    Initializing,
    /// Login required before application can be unlocked.
    AuthenticationRequired,
    /// Authentication in progress (code or password challenge).
    Authenticating,
    /// Authentication failed.
    AuthenticationFailed,
    /// Account verified; channel setup required.
    Authenticated,
    /// Channel setup required.
    ChannelSetupRequired,
    /// Channel setup in progress.
    ChannelSetupInProgress,
    /// Channel verification failed.
    ChannelVerificationFailed,
    /// Fully ready and unlocked.
    Ready,
    /// Session expired or revoked.
    SessionExpired,
    /// Logging out.
    LoggingOut,
    /// Recoverable error.
    RecoverableError,
}

/// Safe personal Telegram account profile details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TelegramAccountInfoDto {
    /// Telegram user ID.
    #[specta(type = Number)]
    pub user_id: i64,
    /// First name.
    pub first_name: String,
    /// Optional last name.
    pub last_name: Option<String>,
    /// Optional username.
    pub username: Option<String>,
    /// Redacted phone number for safe display.
    pub phone_number: String,
}

/// Metadata and status for dedicated private backup channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TelegramChannelInfoDto {
    /// Telegram channel ID.
    #[specta(type = Number)]
    pub channel_id: i64,
    /// Channel title.
    pub channel_title: String,
    /// Whether channel is private.
    pub is_private: bool,
    /// Whether channel permissions were verified.
    pub verified: bool,
    /// Whether created automatically by TELEVAULT.
    pub created_by_televault: bool,
}

/// Complete current authentication and channel setup status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TelegramAuthStatusDto {
    /// Current state machine status.
    pub state: AuthStateDto,
    /// Authenticated account details.
    pub account: Option<TelegramAccountInfoDto>,
    /// Dedicated backup channel details.
    pub channel: Option<TelegramChannelInfoDto>,
    /// Whether 2FA password is required.
    pub requires_password: bool,
    /// Safe error message.
    pub error_message: Option<String>,
}

/// Payload to initiate personal Telegram login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct StartTelegramAuthDto {
    /// Phone number with country code (e.g. +1234567890).
    pub phone_number: String,
    /// Telegram API ID (optional: if omitted, uses application default).
    pub api_id: Option<i32>,
    /// Telegram API Hash (optional: if omitted, uses application default).
    pub api_hash: Option<String>,
}

/// Payload to submit Telegram verification code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SubmitAuthCodeDto {
    /// Verification code received from Telegram.
    pub code: String,
}

/// Payload to submit Two-Step Verification (2FA) cloud password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SubmitAuthPasswordDto {
    /// Cloud password.
    pub password: String,
}

/// Payload to setup or configure private backup channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SetupChannelDto {
    /// Optional manual channel ID. If None, automatically creates dedicated channel.
    #[specta(type = Option<Number>)]
    pub channel_id: Option<i64>,
}

/// Status indicating whether Telegram MTProto API credentials are configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TelegramApiConfigStatusDto {
    /// Whether valid Telegram API credentials are configured locally or via env.
    pub is_configured: bool,
    /// Telegram application numeric API ID (safe to display, API hash is redacted).
    pub api_id: Option<i32>,
}
