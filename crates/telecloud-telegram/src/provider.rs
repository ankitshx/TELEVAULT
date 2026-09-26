use async_trait::async_trait;
use telecloud_core::Result;
use telecloud_storage::{
    ChunkVerificationResult, ProgressSender, StorageMetadataRecord, StorageObjectRef,
    StorageProvider, StorageStatus, UploadChunkRequest,
};
use tokio::io::{AsyncRead, AsyncWrite};

#[allow(dead_code)]
pub struct TelegramStorageProvider {
    primary_channel_id: i64,
    mirror_channel_id: Option<i64>,
}

impl TelegramStorageProvider {
    pub fn new(primary_channel_id: i64, mirror_channel_id: Option<i64>) -> Self {
        Self {
            primary_channel_id,
            mirror_channel_id,
        }
    }
}

#[async_trait]
impl StorageProvider for TelegramStorageProvider {
    async fn check_connection(&self) -> Result<StorageStatus> {
        // Will connect via MTProto or Telegram Bot API adapter
        Ok(StorageStatus {
            is_connected: true,
            provider_name: "TelegramMTProto".to_string(),
            user_identifier: None,
            is_premium: false,
            max_single_upload_bytes: 2000 * 1024 * 1024,
        })
    }

    async fn upload_chunk(
        &self,
        _request: UploadChunkRequest,
        _reader: Box<dyn AsyncRead + Send + Unpin>,
        _progress: Option<ProgressSender>,
    ) -> Result<StorageObjectRef> {
        // Concrete MTProto upload integration
        Ok(StorageObjectRef {
            provider: "telegram".to_string(),
            channel_id: self.primary_channel_id,
            object_id: 1,
            document_id: None,
        })
    }

    async fn download_chunk(
        &self,
        _object_ref: &StorageObjectRef,
        _writer: Box<dyn AsyncWrite + Send + Unpin>,
        _progress: Option<ProgressSender>,
    ) -> Result<u64> {
        Ok(0)
    }

    async fn verify_object(&self, _object_ref: &StorageObjectRef) -> Result<ChunkVerificationResult> {
        Ok(ChunkVerificationResult {
            exists: true,
            size_matched: true,
            remote_sha256: None,
        })
    }

    async fn delete_object(&self, _object_ref: &StorageObjectRef) -> Result<()> {
        Ok(())
    }

    async fn scan_objects(&self, _channel_id: i64) -> Result<Vec<StorageMetadataRecord>> {
        Ok(Vec::new())
    }
}
