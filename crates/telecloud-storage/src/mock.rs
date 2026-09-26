use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use telecloud_core::{Result, TeleCloudError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::RwLock;

use crate::{
    ChunkVerificationResult, ProgressSender, StorageMetadataRecord, StorageObjectRef,
    StorageProvider, StorageStatus, UploadChunkRequest,
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StoredObject {
    pub channel_id: i64,
    pub object_id: i64,
    pub data: Vec<u8>,
    pub sha256: String,
    pub caption: Option<String>,
}

#[derive(Clone)]
pub struct MockStorageProvider {
    primary_channel_id: i64,
    next_object_id: Arc<AtomicI64>,
    storage: Arc<RwLock<HashMap<(i64, i64), StoredObject>>>,
    storage_dir: Option<PathBuf>,
}

impl MockStorageProvider {
    pub fn new(primary_channel_id: i64) -> Self {
        Self {
            primary_channel_id,
            next_object_id: Arc::new(AtomicI64::new(1000)),
            storage: Arc::new(RwLock::new(HashMap::new())),
            storage_dir: None,
        }
    }

    pub fn with_storage_dir(primary_channel_id: i64, dir: impl Into<PathBuf>) -> Self {
        let path = dir.into();
        let _ = std::fs::create_dir_all(&path);
        let mut max_id = 1000;
        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    if let Some(stem) = file_name.strip_suffix(".dat") {
                        if let Some((_, obj_str)) = stem.split_once('_') {
                            if let Ok(id) = obj_str.parse::<i64>() {
                                if id >= max_id {
                                    max_id = id + 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        Self {
            primary_channel_id,
            next_object_id: Arc::new(AtomicI64::new(max_id)),
            storage: Arc::new(RwLock::new(HashMap::new())),
            storage_dir: Some(path),
        }
    }

    pub async fn stored_count(&self) -> usize {
        self.storage.read().await.len()
    }
}

#[async_trait]
impl StorageProvider for MockStorageProvider {
    async fn check_connection(&self) -> Result<StorageStatus> {
        Ok(StorageStatus {
            is_connected: true,
            provider_name: "MockTelegram".to_string(),
            user_identifier: Some("+1234567890".to_string()),
            is_premium: false,
            max_single_upload_bytes: 2000 * 1024 * 1024,
        })
    }

    async fn upload_chunk(
        &self,
        request: UploadChunkRequest,
        mut reader: Box<dyn AsyncRead + Send + Unpin>,
        progress: Option<ProgressSender>,
    ) -> Result<StorageObjectRef> {
        let channel_id = request.target_channel_id.unwrap_or(self.primary_channel_id);
        let object_id = self.next_object_id.fetch_add(1, Ordering::SeqCst);

        let mut data = Vec::with_capacity(request.size_bytes as usize);
        let mut buf = [0u8; 64 * 1024];
        let mut total_read = 0u64;

        loop {
            let n = reader.read(&mut buf).await.map_err(|e| TeleCloudError::Io {
                operation: "mock_upload_read".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            total_read += n as u64;

            if let Some(ref p) = progress {
                let _ = p.send((total_read, request.size_bytes));
            }
        }

        if let Some(ref dir) = self.storage_dir {
            let file_path = dir.join(format!("{}_{}.dat", channel_id, object_id));
            if let Ok(mut f) = tokio::fs::File::create(&file_path).await {
                let _ = f.write_all(&data).await;
            }
            if let Some(ref caption) = request.caption_metadata {
                let meta_path = dir.join(format!("{}_{}.meta", channel_id, object_id));
                let _ = tokio::fs::write(&meta_path, caption).await;
            }
        }

        let obj = StoredObject {
            channel_id,
            object_id,
            data,
            sha256: request.sha256,
            caption: request.caption_metadata,
        };

        self.storage.write().await.insert((channel_id, object_id), obj);

        Ok(StorageObjectRef {
            provider: "mock_telegram".to_string(),
            channel_id,
            object_id,
            document_id: Some(object_id * 10),
        })
    }

    async fn download_chunk(
        &self,
        object_ref: &StorageObjectRef,
        mut writer: Box<dyn AsyncWrite + Send + Unpin>,
        progress: Option<ProgressSender>,
    ) -> Result<u64> {
        // 1. Check in-memory first
        {
            let storage = self.storage.read().await;
            if let Some(obj) = storage.get(&(object_ref.channel_id, object_ref.object_id)) {
                let total_size = obj.data.len() as u64;
                let chunk_size = 64 * 1024;
                let mut written = 0u64;

                for chunk in obj.data.chunks(chunk_size) {
                    writer.write_all(chunk).await.map_err(|e| TeleCloudError::Io {
                        operation: "mock_download_write".to_string(),
                        message: e.to_string(),
                        recoverable: false,
                    })?;
                    written += chunk.len() as u64;

                    if let Some(ref p) = progress {
                        let _ = p.send((written, total_size));
                    }
                }

                writer.flush().await.map_err(|e| TeleCloudError::Io {
                    operation: "mock_download_flush".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;

                return Ok(written);
            }
        }

        // 2. Check disk if storage_dir is configured
        if let Some(ref dir) = self.storage_dir {
            let file_path = dir.join(format!("{}_{}.dat", object_ref.channel_id, object_ref.object_id));
            if file_path.exists() {
                let mut file = tokio::fs::File::open(&file_path).await.map_err(|e| TeleCloudError::Io {
                    operation: "mock_download_open".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;
                let meta = file.metadata().await.map_err(|e| TeleCloudError::Io {
                    operation: "mock_download_metadata".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;
                let total_size = meta.len();
                let mut buf = [0u8; 64 * 1024];
                let mut written = 0u64;

                loop {
                    let n = file.read(&mut buf).await.map_err(|e| TeleCloudError::Io {
                        operation: "mock_download_read".to_string(),
                        message: e.to_string(),
                        recoverable: false,
                    })?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n]).await.map_err(|e| TeleCloudError::Io {
                        operation: "mock_download_write".to_string(),
                        message: e.to_string(),
                        recoverable: false,
                    })?;
                    written += n as u64;
                    if let Some(ref p) = progress {
                        let _ = p.send((written, total_size));
                    }
                }

                writer.flush().await.map_err(|e| TeleCloudError::Io {
                    operation: "mock_download_flush".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;

                return Ok(written);
            }
        }

        Err(TeleCloudError::Storage {
            operation: "mock_download_chunk".to_string(),
            message: format!("Object {} not found", object_ref.object_id),
            recoverable: false,
        })
    }

    async fn verify_object(&self, object_ref: &StorageObjectRef) -> Result<ChunkVerificationResult> {
        let storage = self.storage.read().await;
        if let Some(obj) = storage.get(&(object_ref.channel_id, object_ref.object_id)) {
            return Ok(ChunkVerificationResult {
                exists: true,
                size_matched: true,
                remote_sha256: Some(obj.sha256.clone()),
            });
        }

        if let Some(ref dir) = self.storage_dir {
            let file_path = dir.join(format!("{}_{}.dat", object_ref.channel_id, object_ref.object_id));
            if file_path.exists() {
                return Ok(ChunkVerificationResult {
                    exists: true,
                    size_matched: true,
                    remote_sha256: None,
                });
            }
        }

        Ok(ChunkVerificationResult {
            exists: false,
            size_matched: false,
            remote_sha256: None,
        })
    }

    async fn delete_object(&self, object_ref: &StorageObjectRef) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.remove(&(object_ref.channel_id, object_ref.object_id));
        if let Some(ref dir) = self.storage_dir {
            let file_path = dir.join(format!("{}_{}.dat", object_ref.channel_id, object_ref.object_id));
            let meta_path = dir.join(format!("{}_{}.meta", object_ref.channel_id, object_ref.object_id));
            let _ = tokio::fs::remove_file(file_path).await;
            let _ = tokio::fs::remove_file(meta_path).await;
        }
        Ok(())
    }

    async fn scan_objects(&self, channel_id: i64) -> Result<Vec<StorageMetadataRecord>> {
        let storage = self.storage.read().await;
        let mut records = Vec::new();
        let mut seen = HashSet::new();

        for ((c_id, m_id), obj) in storage.iter() {
            if *c_id == channel_id {
                seen.insert(*m_id);
                records.push(StorageMetadataRecord {
                    channel_id: *c_id,
                    message_id: *m_id,
                    document_id: Some(m_id * 10),
                    file_name: None,
                    size: obj.data.len() as u64,
                    caption: obj.caption.clone(),
                });
            }
        }

        if let Some(ref dir) = self.storage_dir {
            if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Some(name) = entry.file_name().to_str() {
                        if let Some(stem) = name.strip_suffix(".dat") {
                            if let Some((c_str, obj_str)) = stem.split_once('_') {
                                if let (Ok(c), Ok(obj_id)) = (c_str.parse::<i64>(), obj_str.parse::<i64>()) {
                                    if c == channel_id && !seen.contains(&obj_id) {
                                        let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
                                        let meta_path = dir.join(format!("{}_{}.meta", c, obj_id));
                                        let caption = tokio::fs::read_to_string(meta_path).await.ok();
                                        records.push(StorageMetadataRecord {
                                            channel_id: c,
                                            message_id: obj_id,
                                            document_id: Some(obj_id * 10),
                                            file_name: None,
                                            size,
                                            caption,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(records)
    }
}
