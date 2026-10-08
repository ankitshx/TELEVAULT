//! Application configuration and preferences DTOs.

use serde::{Deserialize, Serialize};
use specta::Type;
use televault_core::config::{
    AppConfig, BackupPreferences, GeneralConfig, StorageConfig, TransferConfig,
};
use televault_core::models::CompressionAlgorithm;

use crate::error::IpcError;

/// Full application configuration DTO reflecting the validated AppConfig model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppConfigDto {
    /// General UI and update preferences.
    pub general: GeneralConfigDto,
    /// Storage cache and retention limits.
    pub storage: StorageConfigDto,
    /// Transfer worker concurrency and chunk settings.
    pub transfer: TransferConfigDto,
    /// Default backup engine preferences.
    pub backup: BackupPreferencesDto,
}

/// General application user interface preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct GeneralConfigDto {
    /// Visual theme ("dark", "light", "system").
    pub theme: String,
    /// UI language code (e.g. "en-US").
    pub language: String,
    /// Whether automatic update check is active.
    pub check_updates: bool,
}

/// Storage cache and temporary file retention parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StorageConfigDto {
    /// Maximum cache size allowed in megabytes.
    pub max_cache_size_mb: u32,
    /// Retention period for temporary staging files in hours.
    pub temp_retention_hours: u32,
}

/// Transfer workers and concurrency throttling parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TransferConfigDto {
    /// Standard chunk size for file slicing in kilobytes (64 to 32768).
    pub chunk_size_kb: u32,
    /// Maximum number of concurrent transfer workers (1 to 16).
    pub max_concurrent_transfers: u8,
    /// Maximum upload bandwidth limit in KB/s (0 = unlimited).
    pub upload_limit_kbps: u32,
}

/// Default backup preferences for profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct BackupPreferencesDto {
    /// Default compression algorithm ("None" or "Zstd").
    pub default_compression: String,
    /// Default snapshot retention period in days (0 = keep indefinitely).
    pub retention_days: u32,
    /// Whether to verify cryptographic hashes immediately following backup upload.
    pub verify_after_backup: bool,
}

impl From<AppConfig> for AppConfigDto {
    fn from(cfg: AppConfig) -> Self {
        Self {
            general: GeneralConfigDto {
                theme: cfg.general.theme,
                language: cfg.general.language,
                check_updates: cfg.general.check_updates,
            },
            storage: StorageConfigDto {
                max_cache_size_mb: cfg.storage.max_cache_size_mb,
                temp_retention_hours: cfg.storage.temp_retention_hours,
            },
            transfer: TransferConfigDto {
                chunk_size_kb: cfg.transfer.chunk_size_kb,
                max_concurrent_transfers: cfg.transfer.max_concurrent_transfers,
                upload_limit_kbps: cfg.transfer.upload_limit_kbps,
            },
            backup: BackupPreferencesDto {
                default_compression: match cfg.backup.default_compression {
                    CompressionAlgorithm::None => "None".to_string(),
                    CompressionAlgorithm::Zstd => "Zstd".to_string(),
                },
                retention_days: cfg.backup.retention_days,
                verify_after_backup: cfg.backup.verify_after_backup,
            },
        }
    }
}

impl TryFrom<AppConfigDto> for AppConfig {
    type Error = IpcError;

    fn try_from(dto: AppConfigDto) -> Result<Self, Self::Error> {
        let default_compression = match dto
            .backup
            .default_compression
            .trim()
            .to_lowercase()
            .as_str()
        {
            "none" => CompressionAlgorithm::None,
            "zstd" => CompressionAlgorithm::Zstd,
            other => {
                return Err(IpcError::validation(format!(
                    "Unsupported compression algorithm '{other}'. Supported values: 'None', 'Zstd'"
                )))
            }
        };

        Ok(Self {
            general: GeneralConfig {
                theme: dto.general.theme,
                language: dto.general.language,
                check_updates: dto.general.check_updates,
            },
            storage: StorageConfig {
                max_cache_size_mb: dto.storage.max_cache_size_mb,
                temp_retention_hours: dto.storage.temp_retention_hours,
            },
            transfer: TransferConfig {
                chunk_size_kb: dto.transfer.chunk_size_kb,
                max_concurrent_transfers: dto.transfer.max_concurrent_transfers,
                upload_limit_kbps: dto.transfer.upload_limit_kbps,
            },
            backup: BackupPreferences {
                default_compression,
                retention_days: dto.backup.retention_days,
                verify_after_backup: dto.backup.verify_after_backup,
            },
        })
    }
}
