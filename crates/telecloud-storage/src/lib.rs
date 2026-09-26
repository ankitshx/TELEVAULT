use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use telecloud_core::Result;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use uuid::Uuid;

pub mod mock;
pub use mock::MockStorageProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStatus {
    pub is_connected: bool,
    pub provider_name: String,
    pub user_identifier: Option<String>,
    pub is_premium: bool,
    pub max_single_upload_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadChunkRequest {
    pub file_id: Uuid,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub size_bytes: u64,
    pub sha256: String,
    pub mime_type: Option<String>,
    pub caption_metadata: Option<String>,
    pub target_channel_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageObjectRef {
    pub provider: String,
    pub channel_id: i64,
    pub object_id: i64,
    pub document_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkVerificationResult {
    pub exists: bool,
    pub size_matched: bool,
    pub remote_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadataRecord {
    pub channel_id: i64,
    pub message_id: i64,
    pub document_id: Option<i64>,
    pub file_name: Option<String>,
    pub size: u64,
    pub caption: Option<String>,
}

pub type ProgressSender = mpsc::UnboundedSender<(u64, u64)>; // (bytes_transferred, total_bytes)

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn check_connection(&self) -> Result<StorageStatus>;

    async fn upload_chunk(
        &self,
        request: UploadChunkRequest,
        reader: Box<dyn AsyncRead + Send + Unpin>,
        progress: Option<ProgressSender>,
    ) -> Result<StorageObjectRef>;

    async fn download_chunk(
        &self,
        object_ref: &StorageObjectRef,
        writer: Box<dyn AsyncWrite + Send + Unpin>,
        progress: Option<ProgressSender>,
    ) -> Result<u64>;

    async fn verify_object(&self, object_ref: &StorageObjectRef) -> Result<ChunkVerificationResult>;

    async fn delete_object(&self, object_ref: &StorageObjectRef) -> Result<()>;

    async fn scan_objects(&self, channel_id: i64) -> Result<Vec<StorageMetadataRecord>>;
}
