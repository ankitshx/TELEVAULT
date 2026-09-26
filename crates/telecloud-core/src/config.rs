use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::models::RedundancyMode;

pub const DEFAULT_CHUNK_SIZE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB default chunk
pub const MAX_TELEGRAM_SINGLE_OBJECT_BYTES: u64 = 2000 * 1024 * 1024; // 2.0 GB Telegram Limit

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_name: String,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub chunk_size: u64,
    pub max_concurrent_transfers: usize,
    pub redundancy: RedundancyMode,
    pub primary_channel_id: Option<i64>,
    pub mirror_channel_id: Option<i64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let base_dir = dirs_fallback();
        Self {
            app_name: "TeleCloud".to_string(),
            data_dir: base_dir.join("data"),
            cache_dir: base_dir.join("cache"),
            chunk_size: DEFAULT_CHUNK_SIZE_BYTES,
            max_concurrent_transfers: 3,
            redundancy: RedundancyMode::Optional,
            primary_channel_id: None,
            mirror_channel_id: None,
        }
    }
}

fn dirs_fallback() -> PathBuf {
    if let Some(mut base) = dirs_data_dir() {
        base.push("TeleCloud");
        base
    } else {
        PathBuf::from(".telecloud")
    }
}

fn dirs_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}
