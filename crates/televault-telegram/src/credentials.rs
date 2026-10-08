//! Secure Telegram credentials model with redaction and storage isolation.

use crate::error::{Result, TelegramError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Telegram credentials holding secret bot token and target chat coordinate.
///
/// Implements custom `Debug` and `Display` that redact sensitive tokens,
/// preventing accidental credential leakage in diagnostic logs, error reports, or IPC responses.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramCredentials {
    /// Telegram Bot authentication token (format: `<bot_id>:<secret_token>`).
    pub bot_token: String,
    /// Target chat or channel identifier where backups are stored.
    pub target_chat_id: i64,
    /// Optional custom Bot API endpoint (defaults to "https://api.telegram.org").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_endpoint: Option<String>,
}

impl std::fmt::Debug for TelegramCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramCredentials")
            .field("target_chat_id", &self.target_chat_id)
            .field("api_endpoint", &self.api_endpoint)
            .field("bot_token", &"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Display for TelegramCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TelegramCredentials(chat_id: {}, bot_token: [REDACTED])",
            self.target_chat_id
        )
    }
}

impl TelegramCredentials {
    /// Creates and validates a new [`TelegramCredentials`] instance.
    pub fn new(
        bot_token: impl Into<String>,
        target_chat_id: i64,
        api_endpoint: Option<String>,
    ) -> Result<Self> {
        let creds = Self {
            bot_token: bot_token.into(),
            target_chat_id,
            api_endpoint,
        };
        creds.validate()?;
        Ok(creds)
    }

    /// Validates Telegram credential invariants.
    pub fn validate(&self) -> Result<()> {
        let token = self.bot_token.trim();
        if token.is_empty() {
            return Err(TelegramError::Transport(
                "Telegram bot token cannot be empty".into(),
            ));
        }
        // Telegram Bot tokens have the format `<bot_id>:<token_hash>` (e.g. 123456789:ABC...)
        if !token.contains(':') || token.len() < 15 {
            return Err(TelegramError::Transport(
                "Invalid Telegram bot token format. Expected format: '<bot_id>:<token>'".into(),
            ));
        }
        if self.target_chat_id == 0 {
            return Err(TelegramError::InvalidReference(
                "Telegram target chat ID cannot be 0".into(),
            ));
        }
        if let Some(ref endpoint) = self.api_endpoint {
            let ep = endpoint.trim();
            if ep.is_empty() || (!ep.starts_with("http://") && !ep.starts_with("https://")) {
                return Err(TelegramError::Transport(
                    "Custom API endpoint must start with 'http://' or 'https://'".into(),
                ));
            }
        }
        Ok(())
    }

    /// Returns a partially-masked representation safe for diagnostics (e.g. "123456...[REDACTED]").
    pub fn safe_masked_token(&self) -> String {
        if let Some((bot_id, _)) = self.bot_token.split_once(':') {
            format!("{}:[REDACTED]", bot_id)
        } else {
            "[REDACTED]".to_string()
        }
    }

    /// Securely persists the credentials to disk in atomic fashion.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(self).map_err(|e| {
            TelegramError::Transport(format!("Failed to serialize credentials: {e}"))
        })?;

        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, serialized.as_bytes())?;
        std::fs::rename(&temp_path, path)?;
        Ok(())
    }

    /// Loads and validates credentials from a local secure JSON file.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(TelegramError::Transport(format!(
                "Credentials file not found at {}",
                path.display()
            )));
        }
        let content = std::fs::read_to_string(path)?;
        let creds: Self = serde_json::from_str(&content)
            .map_err(|e| TelegramError::Transport(format!("Invalid credentials JSON: {e}")))?;
        creds.validate()?;
        Ok(creds)
    }

    /// Deletes the credentials file from disk.
    pub fn remove_file(path: &Path) -> Result<()> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_redaction_in_debug_and_display() {
        let creds = TelegramCredentials {
            bot_token: "123456789:AAEFakeSecretTokenABC123XYZ".into(),
            target_chat_id: -1001234567890,
            api_endpoint: None,
        };

        let debug_str = format!("{:?}", creds);
        assert!(!debug_str.contains("AAEFakeSecretTokenABC123XYZ"));
        assert!(debug_str.contains("[REDACTED]"));

        let display_str = format!("{}", creds);
        assert!(!display_str.contains("AAEFakeSecretTokenABC123XYZ"));
        assert!(display_str.contains("[REDACTED]"));

        assert_eq!(creds.safe_masked_token(), "123456789:[REDACTED]");
    }

    #[test]
    fn test_credentials_validation() {
        assert!(TelegramCredentials::new("", -100, None).is_err());
        assert!(TelegramCredentials::new("short_token", -100, None).is_err());
        assert!(TelegramCredentials::new("123456789:ValidSecretTokenFormat", 0, None).is_err());
        assert!(TelegramCredentials::new(
            "123456789:ValidSecretTokenFormat",
            -100123456,
            Some("invalid-url".into())
        )
        .is_err());
        assert!(TelegramCredentials::new(
            "123456789:ValidSecretTokenFormat",
            -100123456,
            Some("https://custom.tg.api".into())
        )
        .is_ok());
    }

    #[test]
    fn test_credentials_file_lifecycle() {
        let temp_dir = std::env::temp_dir().join("televault_creds_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let path = temp_dir.join("test_creds.json");

        let creds =
            TelegramCredentials::new("987654321:AnotherSecretTokenFormatXYZ", -100998877, None)
                .unwrap();

        creds.save_to_file(&path).expect("save");
        let loaded = TelegramCredentials::load_from_file(&path).expect("load");
        assert_eq!(creds, loaded);

        TelegramCredentials::remove_file(&path).expect("remove");
        assert!(!path.exists());
    }
}
