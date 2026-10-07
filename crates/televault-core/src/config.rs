//! Application configuration models without secret storage.

use crate::error::{AppError, Result};
use crate::models::CompressionAlgorithm;
use serde::{Deserialize, Serialize};

/// Root application configuration model.
///
/// Note: Secrets (passwords, tokens, encryption keys, Telegram session credentials)
/// are NEVER stored in this structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// General user interface and telemetry preferences.
    pub general: GeneralConfig,
    /// Storage and cache directory preferences.
    pub storage: StorageConfig,
    /// Transfer concurrency and network limits.
    pub transfer: TransferConfig,
    /// Default backup engine settings.
    pub backup: BackupPreferences,
}

impl AppConfig {
    /// Validates configuration invariants across all sub-configurations.
    pub fn validate(&self) -> Result<()> {
        self.general.validate()?;
        self.storage.validate()?;
        self.transfer.validate()?;
        self.backup.validate()?;
        Ok(())
    }
}

/// General application preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Application visual theme (e.g., "dark", "light", "system").
    pub theme: String,
    /// UI language code (e.g., "en-US").
    pub language: String,
    /// Whether automatic update checking is enabled.
    pub check_updates: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            language: "en-US".into(),
            check_updates: true,
        }
    }
}

impl GeneralConfig {
    /// Validates general settings invariants.
    pub fn validate(&self) -> Result<()> {
        if self.theme.trim().is_empty() {
            return Err(AppError::config("general.theme", "theme cannot be empty"));
        }
        if self.language.trim().is_empty() {
            return Err(AppError::config(
                "general.language",
                "language cannot be empty",
            ));
        }
        Ok(())
    }
}

/// Storage allocation and retention parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Maximum cache size allowed in megabytes.
    pub max_cache_size_mb: u32,
    /// Retention period for temporary staging files in hours.
    pub temp_retention_hours: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            max_cache_size_mb: 2048,
            temp_retention_hours: 24,
        }
    }
}

impl StorageConfig {
    /// Validates storage settings invariants.
    pub fn validate(&self) -> Result<()> {
        if self.max_cache_size_mb == 0 {
            return Err(AppError::config(
                "storage.max_cache_size_mb",
                "cache size must be greater than zero",
            ));
        }
        Ok(())
    }
}

/// Transfer workers and concurrency throttling parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferConfig {
    /// Standard chunk size for file slicing in kilobytes (default: 4096 = 4MB).
    pub chunk_size_kb: u32,
    /// Maximum number of concurrent upload/download workers.
    pub max_concurrent_transfers: u8,
    /// Maximum upload bandwidth limit in KB/s (0 = unlimited).
    pub upload_limit_kbps: u32,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            chunk_size_kb: 4096,
            max_concurrent_transfers: 3,
            upload_limit_kbps: 0,
        }
    }
}

impl TransferConfig {
    /// Validates transfer settings invariants.
    pub fn validate(&self) -> Result<()> {
        if self.chunk_size_kb < 64 {
            return Err(AppError::config(
                "transfer.chunk_size_kb",
                "chunk size must be at least 64 KB",
            ));
        }
        if self.chunk_size_kb > 32768 {
            return Err(AppError::config(
                "transfer.chunk_size_kb",
                "chunk size cannot exceed 32 MB",
            ));
        }
        if self.max_concurrent_transfers == 0 {
            return Err(AppError::config(
                "transfer.max_concurrent_transfers",
                "max concurrent transfers must be at least 1",
            ));
        }
        Ok(())
    }
}

/// Default backup preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupPreferences {
    /// Default compression algorithm.
    pub default_compression: CompressionAlgorithm,
    /// Default snapshot retention period in days (0 = keep indefinitely).
    pub retention_days: u32,
    /// Whether to verify cryptographic hashes immediately following backup upload.
    pub verify_after_backup: bool,
}

impl Default for BackupPreferences {
    fn default() -> Self {
        Self {
            default_compression: CompressionAlgorithm::Zstd,
            retention_days: 30,
            verify_after_backup: true,
        }
    }
}

impl BackupPreferences {
    /// Validates backup preferences invariants.
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.general.theme, "dark");
        assert_eq!(config.storage.max_cache_size_mb, 2048);
        assert_eq!(config.transfer.chunk_size_kb, 4096);
        assert_eq!(
            config.backup.default_compression,
            CompressionAlgorithm::Zstd
        );
    }

    #[test]
    fn test_invalid_general_config() {
        let mut config = AppConfig::default();
        config.general.theme = "  ".into();
        assert!(config.validate().is_err());

        config.general.theme = "dark".into();
        config.general.language = "".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_storage_config() {
        let mut config = AppConfig::default();
        config.storage.max_cache_size_mb = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_transfer_config() {
        let mut config = AppConfig::default();
        config.transfer.chunk_size_kb = 32; // < 64KB
        assert!(config.validate().is_err());

        config.transfer.chunk_size_kb = 65536; // > 32MB
        assert!(config.validate().is_err());

        config.transfer.chunk_size_kb = 4096;
        config.transfer.max_concurrent_transfers = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, config);
    }
}
