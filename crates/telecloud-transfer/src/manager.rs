use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use telecloud_core::{
    LogicalFile, Result, TeleCloudError, TransferDirection, TransferRecord, TransferState,
};
use telecloud_db::DatabaseRepository;
use telecloud_storage::StorageProvider;
use tokio::sync::{watch, RwLock};
use uuid::Uuid;

use crate::pipeline::{DownloadParams, TransferPipeline, TransferSignal, UploadParams};

pub struct TransferManager {
    storage: Arc<dyn StorageProvider>,
    db: Arc<DatabaseRepository>,
    active_transfers: Arc<RwLock<HashMap<Uuid, TransferRecord>>>,
    signals: Arc<RwLock<HashMap<Uuid, watch::Sender<TransferSignal>>>>,
}

impl TransferManager {
    pub fn new(storage: Arc<dyn StorageProvider>, db: Arc<DatabaseRepository>) -> Self {
        Self {
            storage,
            db,
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            signals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn storage(&self) -> &Arc<dyn StorageProvider> {
        &self.storage
    }

    pub fn db(&self) -> &Arc<DatabaseRepository> {
        &self.db
    }

    /// Enqueues and begins streaming file upload in background
    pub async fn enqueue_upload(&self, params: UploadParams) -> Result<Uuid> {
        let transfer_id = params.transfer_id;
        let file_path = &params.local_path;

        let metadata = tokio::fs::metadata(file_path).await.map_err(|e| TeleCloudError::Io {
            operation: "enqueue_upload_metadata".to_string(),
            message: format!("Failed to read metadata for {:?}: {}", file_path, e),
            recoverable: false,
        })?;

        let file_size = metadata.len();
        let chunk_size = params.chunk_size.unwrap_or(20 * 1024 * 1024);
        let total_chunks = if file_size == 0 {
            1
        } else {
            ((file_size + chunk_size - 1) / chunk_size) as u32
        };

        let file_id = params.file_id.unwrap_or_else(Uuid::new_v4);
        let mut params = params;
        params.file_id = Some(file_id);

        let initial_file = LogicalFile {
            id: file_id,
            folder_id: params.folder_id,
            name: params.file_name.clone(),
            size: file_size,
            sha256: String::new(),
            mime_type: None,
            is_favorite: false,
            is_encrypted: params.passphrase.is_some(),
            manifest_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.db.insert_file(&initial_file)?;

        let record = TransferRecord {
            id: transfer_id,
            file_id: Some(file_id),
            direction: TransferDirection::Upload,
            local_path: params.local_path.to_string_lossy().to_string(),
            file_name: params.file_name.clone(),
            total_bytes: file_size,
            transferred_bytes: 0,
            state: TransferState::Queued,
            speed_bytes_per_sec: 0,
            eta_seconds: None,
            active_chunk_index: Some(0),
            total_chunks: Some(total_chunks),
            error_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.upsert_transfer(&record)?;
        self.active_transfers.write().await.insert(transfer_id, record);

        let (sig_tx, sig_rx) = watch::channel(TransferSignal::Running);
        self.signals.write().await.insert(transfer_id, sig_tx);

        let storage_clone = self.storage.clone();
        let db_clone = self.db.clone();
        let active_clone = self.active_transfers.clone();
        let signals_clone = self.signals.clone();

        tokio::spawn(async move {
            let res = TransferPipeline::execute_upload(storage_clone, db_clone.clone(), params, sig_rx).await;
            if let Err(ref e) = res {
                tracing::error!("Upload failed for transfer {}: {}", transfer_id, e);
                if let Ok(Some(mut tr)) = db_clone.get_transfer(transfer_id) {
                    if tr.state != TransferState::Cancelled && tr.state != TransferState::Paused {
                        tr.state = TransferState::Failed;
                        tr.error_message = Some(e.to_string());
                        tr.updated_at = Utc::now();
                        let _ = db_clone.upsert_transfer(&tr);
                    }
                }
            }

            active_clone.write().await.remove(&transfer_id);
            signals_clone.write().await.remove(&transfer_id);
        });

        Ok(transfer_id)
    }

    /// Enqueues and begins streaming file download in background
    pub async fn enqueue_download(&self, params: DownloadParams) -> Result<Uuid> {
        let transfer_id = params.transfer_id;
        let file = self.db.get_file(params.file_id)?.ok_or_else(|| TeleCloudError::Database {
            operation: "enqueue_download".to_string(),
            message: format!("File {} not found", params.file_id),
            recoverable: false,
        })?;

        let record = TransferRecord {
            id: transfer_id,
            file_id: Some(file.id),
            direction: TransferDirection::Download,
            local_path: params.dest_path.to_string_lossy().to_string(),
            file_name: file.name.clone(),
            total_bytes: file.size,
            transferred_bytes: 0,
            state: TransferState::Queued,
            speed_bytes_per_sec: 0,
            eta_seconds: None,
            active_chunk_index: Some(0),
            total_chunks: None,
            error_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.upsert_transfer(&record)?;
        self.active_transfers.write().await.insert(transfer_id, record);

        let (sig_tx, sig_rx) = watch::channel(TransferSignal::Running);
        self.signals.write().await.insert(transfer_id, sig_tx);

        let storage_clone = self.storage.clone();
        let db_clone = self.db.clone();
        let active_clone = self.active_transfers.clone();
        let signals_clone = self.signals.clone();

        tokio::spawn(async move {
            let res = TransferPipeline::execute_download(storage_clone, db_clone.clone(), params, sig_rx).await;
            if let Err(ref e) = res {
                tracing::error!("Download failed for transfer {}: {}", transfer_id, e);
                if let Ok(Some(mut tr)) = db_clone.get_transfer(transfer_id) {
                    if tr.state != TransferState::Cancelled && tr.state != TransferState::Paused {
                        tr.state = TransferState::Failed;
                        tr.error_message = Some(e.to_string());
                        tr.updated_at = Utc::now();
                        let _ = db_clone.upsert_transfer(&tr);
                    }
                }
            }

            active_clone.write().await.remove(&transfer_id);
            signals_clone.write().await.remove(&transfer_id);
        });

        Ok(transfer_id)
    }

    /// Synchronous / direct execution of upload without spawning background task (ideal for deterministic tests and CLI commands)
    pub async fn execute_upload_now(&self, params: UploadParams) -> Result<LogicalFile> {
        let (_sig_tx, sig_rx) = watch::channel(TransferSignal::Running);
        TransferPipeline::execute_upload(self.storage.clone(), self.db.clone(), params, sig_rx).await
    }

    /// Synchronous / direct execution of download without spawning background task (ideal for deterministic tests and CLI commands)
    pub async fn execute_download_now(&self, params: DownloadParams) -> Result<()> {
        let (_sig_tx, sig_rx) = watch::channel(TransferSignal::Running);
        TransferPipeline::execute_download(self.storage.clone(), self.db.clone(), params, sig_rx).await
    }

    pub async fn pause_transfer(&self, transfer_id: Uuid) -> Result<()> {
        let signals = self.signals.read().await;
        if let Some(tx) = signals.get(&transfer_id) {
            let _ = tx.send(TransferSignal::Paused);
        }

        if let Some(mut record) = self.db.get_transfer(transfer_id)? {
            record.state = TransferState::Paused;
            record.updated_at = Utc::now();
            self.db.upsert_transfer(&record)?;
            self.active_transfers.write().await.insert(transfer_id, record);
            Ok(())
        } else {
            Err(TeleCloudError::Transfer {
                operation: "pause_transfer".to_string(),
                message: format!("Transfer {} not found in database", transfer_id),
                recoverable: true,
            })
        }
    }

    pub async fn resume_transfer(&self, transfer_id: Uuid) -> Result<()> {
        let signals = self.signals.read().await;
        if let Some(tx) = signals.get(&transfer_id) {
            let _ = tx.send(TransferSignal::Running);
        }

        if let Some(mut record) = self.db.get_transfer(transfer_id)? {
            record.state = match record.direction {
                TransferDirection::Upload => TransferState::Uploading,
                TransferDirection::Download => TransferState::Downloading,
            };
            record.updated_at = Utc::now();
            self.db.upsert_transfer(&record)?;
            self.active_transfers.write().await.insert(transfer_id, record);
            Ok(())
        } else {
            Err(TeleCloudError::Transfer {
                operation: "resume_transfer".to_string(),
                message: format!("Transfer {} not found in database", transfer_id),
                recoverable: true,
            })
        }
    }

    pub async fn cancel_transfer(&self, transfer_id: Uuid) -> Result<()> {
        let signals = self.signals.read().await;
        if let Some(tx) = signals.get(&transfer_id) {
            let _ = tx.send(TransferSignal::Cancelled);
        }

        if let Some(mut record) = self.db.get_transfer(transfer_id)? {
            record.state = TransferState::Cancelled;
            record.updated_at = Utc::now();
            self.db.upsert_transfer(&record)?;
            self.active_transfers.write().await.remove(&transfer_id);
            Ok(())
        } else {
            Err(TeleCloudError::Transfer {
                operation: "cancel_transfer".to_string(),
                message: format!("Transfer {} not found in database", transfer_id),
                recoverable: false,
            })
        }
    }

    pub async fn get_transfer(&self, transfer_id: Uuid) -> Result<Option<TransferRecord>> {
        if let Some(tr) = self.active_transfers.read().await.get(&transfer_id) {
            return Ok(Some(tr.clone()));
        }
        self.db.get_transfer(transfer_id)
    }

    pub fn list_transfers(&self) -> Result<Vec<TransferRecord>> {
        self.db.list_transfers()
    }

    pub fn list_active_transfers(&self) -> Result<Vec<TransferRecord>> {
        self.db.list_active_transfers()
    }
}
