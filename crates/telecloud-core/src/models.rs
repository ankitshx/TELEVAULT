use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedundancyMode {
    Disabled,
    Optional,
    Enabled,
}

impl Default for RedundancyMode {
    fn default() -> Self {
        Self::Optional
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferState {
    Queued,
    Preparing,
    Uploading,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Verifying,
    Retrying,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalFile {
    pub id: Uuid,
    pub folder_id: Option<Uuid>,
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub mime_type: Option<String>,
    pub is_favorite: bool,
    pub is_encrypted: bool,
    pub manifest_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub id: Uuid,
    pub file_id: Uuid,
    pub chunk_index: u32,
    pub byte_offset: u64,
    pub size: u64,
    pub sha256: String,
    pub storage_provider: String,
    pub remote_chat_id: i64,
    pub remote_message_id: i64,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRecord {
    pub id: Uuid,
    pub file_id: Option<Uuid>,
    pub direction: TransferDirection,
    pub local_path: String,
    pub file_name: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub state: TransferState,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: Option<u64>,
    pub active_chunk_index: Option<u32>,
    pub total_chunks: Option<u32>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Breadcrumb {
    pub id: Option<Uuid>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchFilter {
    pub query: String,
    pub folder_id: Option<Uuid>,
    pub extension: Option<String>,
    pub is_favorite: Option<bool>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecentAction {
    Uploaded,
    Downloaded,
    Opened,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItem {
    pub file: LogicalFile,
    pub action: RecentAction,
    pub accessed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityRecord {
    pub file_id: Uuid,
    pub last_verified_at: DateTime<Utc>,
    pub is_valid: bool,
    pub details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSlice {
    pub index: u32,
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkPlan {
    pub file_size: u64,
    pub chunk_size: u64,
    pub total_chunks: u32,
    pub slices: Vec<ChunkSlice>,
}

