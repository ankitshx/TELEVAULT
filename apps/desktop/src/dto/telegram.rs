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
