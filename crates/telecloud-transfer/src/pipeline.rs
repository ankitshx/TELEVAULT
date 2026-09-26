use chrono::Utc;
use sha2::{Digest, Sha256};
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;
use telecloud_core::{
    ChunkPlanner, FileChunk, LogicalFile, RecentAction, Result, TeleCloudError,
    TransferDirection, TransferRecord, TransferState,
};
use telecloud_crypto::{
    calculate_sha256, decrypt_chunk, derive_key, encrypt_chunk, generate_salt, hex_decode,
    hex_encode, NONCE_LEN, SALT_LEN,
};
use telecloud_db::DatabaseRepository;
use telecloud_integrity::IntegrityVerifier;
use telecloud_manifest::{
    FileManifest, ManifestEncryptionConfig, ManifestStorageObject,
};
use telecloud_storage::{StorageObjectRef, StorageProvider, UploadChunkRequest};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::watch;
use uuid::Uuid;

use crate::progress::ProgressTracker;

#[derive(Clone)]
pub struct SharedVecWriter(pub Arc<std::sync::Mutex<Vec<u8>>>);

impl tokio::io::AsyncWrite for SharedVecWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.0.lock().unwrap().extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferSignal {
    Running,
    Paused,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct UploadParams {
    pub transfer_id: Uuid,
    pub file_id: Option<Uuid>,
    pub folder_id: Option<Uuid>,
    pub local_path: PathBuf,
    pub file_name: String,
    pub passphrase: Option<String>,
    pub chunk_size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DownloadParams {
    pub transfer_id: Uuid,
    pub file_id: Uuid,
    pub dest_path: PathBuf,
    pub passphrase: Option<String>,
}

pub struct TransferPipeline;

impl TransferPipeline {
    /// Executes a streaming file upload with optional AES-256-GCM authenticated chunk encryption.
    pub async fn execute_upload(
        storage: Arc<dyn StorageProvider>,
        db: Arc<DatabaseRepository>,
        params: UploadParams,
        mut signal_rx: watch::Receiver<TransferSignal>,
    ) -> Result<LogicalFile> {
        let file_path = &params.local_path;
        let metadata = fs::metadata(file_path).await.map_err(|e| TeleCloudError::Io {
            operation: "read_metadata".to_string(),
            message: format!("Failed to read metadata for {:?}: {}", file_path, e),
            recoverable: false,
        })?;

        let file_size = metadata.len();
        let chunk_size = params.chunk_size.unwrap_or(20 * 1024 * 1024); // 20 MB default
        let plan = ChunkPlanner::plan(file_size, chunk_size);
        let file_id = params.file_id.unwrap_or_else(Uuid::new_v4);

        // Pre-insert logical file to satisfy foreign key constraints for transfers and file_chunks
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
        db.insert_file(&initial_file)?;

        // Update transfer status to PREPARING / UPLOADING in DB
        let mut transfer_record = db
            .get_transfer(params.transfer_id)?
            .unwrap_or_else(|| TransferRecord {
                id: params.transfer_id,
                file_id: Some(file_id),
                direction: TransferDirection::Upload,
                local_path: params.local_path.to_string_lossy().to_string(),
                file_name: params.file_name.clone(),
                total_bytes: file_size,
                transferred_bytes: 0,
                state: TransferState::Preparing,
                speed_bytes_per_sec: 0,
                eta_seconds: None,
                active_chunk_index: Some(0),
                total_chunks: Some(plan.total_chunks),
                error_message: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });

        transfer_record.file_id = Some(file_id);
        transfer_record.state = TransferState::Uploading;
        transfer_record.updated_at = Utc::now();
        db.upsert_transfer(&transfer_record)?;

        // Key Derivation if passphrase is provided
        let (derived_key, enc_config) = if let Some(ref pass) = params.passphrase {
            let salt = generate_salt();
            let key = derive_key(pass, &salt)?;
            let config = ManifestEncryptionConfig {
                algorithm: "AES-256-GCM".to_string(),
                kdf: "Argon2id".to_string(),
                salt: hex_encode(salt),
            };
            (Some(key), Some(config))
        } else {
            (None, None)
        };

        let mut manifest = FileManifest::new(
            file_id,
            params.file_name.clone(),
            file_size,
            String::new(),
            chunk_size,
            enc_config,
        );

        let mut source_file = File::open(file_path).await.map_err(|e| TeleCloudError::Io {
            operation: "open_file_for_upload".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;

        let mut full_hasher = Sha256::new();
        let mut tracker = ProgressTracker::new(file_size, 0);
        let mut total_transferred = 0u64;

        // Check if any chunks were previously uploaded (resumed transfer)
        let existing_chunks = db.get_file_chunks(file_id)?;
        let mut uploaded_chunk_indices = std::collections::HashSet::new();
        for ec in &existing_chunks {
            uploaded_chunk_indices.insert(ec.chunk_index);
            if (ec.chunk_index as usize) < manifest.chunks.len() {
                manifest.chunks[ec.chunk_index as usize].sha256 = ec.sha256.clone();
                manifest.chunks[ec.chunk_index as usize].status = "uploaded".to_string();
                manifest.chunks[ec.chunk_index as usize].storage_objects = vec![ManifestStorageObject {
                    provider: ec.storage_provider.clone(),
                    chat_id: ec.remote_chat_id,
                    object_id: ec.remote_message_id,
                    is_primary: ec.is_primary,
                }];
            }
        }

        for slice in &plan.slices {
            // Check signals
            loop {
                let current_sig = *signal_rx.borrow();
                match current_sig {
                    TransferSignal::Cancelled => {
                        transfer_record.state = TransferState::Cancelled;
                        transfer_record.updated_at = Utc::now();
                        let _ = db.upsert_transfer(&transfer_record);
                        return Err(TeleCloudError::Transfer {
                            operation: "upload_chunk".to_string(),
                            message: "Transfer cancelled by user".to_string(),
                            recoverable: false,
                        });
                    }
                    TransferSignal::Paused => {
                        transfer_record.state = TransferState::Paused;
                        transfer_record.updated_at = Utc::now();
                        let _ = db.upsert_transfer(&transfer_record);
                        if signal_rx.changed().await.is_err() {
                            return Err(TeleCloudError::Transfer {
                                operation: "upload_chunk".to_string(),
                                message: "Signal channel closed while paused".to_string(),
                                recoverable: false,
                            });
                        }
                    }
                    TransferSignal::Running => break,
                }
            }

            source_file
                .seek(SeekFrom::Start(slice.offset))
                .await
                .map_err(|e| TeleCloudError::Io {
                    operation: "seek_chunk".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;

            let mut chunk_bytes = vec![0u8; slice.size as usize];
            source_file
                .read_exact(&mut chunk_bytes)
                .await
                .map_err(|e| TeleCloudError::Io {
                    operation: "read_chunk".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;

            full_hasher.update(&chunk_bytes);
            let chunk_sha256 = calculate_sha256(&chunk_bytes);

            // Skip upload if already saved in db from prior attempt
            if uploaded_chunk_indices.contains(&slice.index) {
                total_transferred += slice.size;
                tracker.update(total_transferred);
                continue;
            }

            // Encrypt chunk if configured
            let upload_payload = if let Some(ref key) = derived_key {
                let (ciphertext, nonce) = encrypt_chunk(key, slice.index, &chunk_bytes)?;
                let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
                payload.extend_from_slice(&nonce);
                payload.extend_from_slice(&ciphertext);
                payload
            } else {
                chunk_bytes
            };

            let payload_size = upload_payload.len() as u64;
            let upload_req = UploadChunkRequest {
                file_id,
                chunk_index: slice.index,
                total_chunks: plan.total_chunks,
                size_bytes: payload_size,
                sha256: chunk_sha256.clone(),
                mime_type: None,
                caption_metadata: Some(format!("tc1:f={}:c={}:s={}", file_id, slice.index, slice.size)),
                target_channel_id: None,
            };

            let reader = Box::new(std::io::Cursor::new(upload_payload));
            let storage_ref = storage.upload_chunk(upload_req, reader, None).await?;

            // Record chunk in DB
            let chunk_record = FileChunk {
                id: Uuid::new_v4(),
                file_id,
                chunk_index: slice.index,
                byte_offset: slice.offset,
                size: slice.size,
                sha256: chunk_sha256.clone(),
                storage_provider: storage_ref.provider.clone(),
                remote_chat_id: storage_ref.channel_id,
                remote_message_id: storage_ref.object_id,
                is_primary: true,
                created_at: Utc::now(),
            };

            db.save_file_chunks(&[chunk_record])?;

            // Update manifest
            let m_chunk = &mut manifest.chunks[slice.index as usize];
            m_chunk.sha256 = chunk_sha256;
            m_chunk.status = "uploaded".to_string();
            m_chunk.storage_objects = vec![ManifestStorageObject {
                provider: storage_ref.provider,
                chat_id: storage_ref.channel_id,
                object_id: storage_ref.object_id,
                is_primary: true,
            }];

            total_transferred += slice.size;
            tracker.update(total_transferred);

            // Update Transfer Record in DB
            transfer_record.transferred_bytes = total_transferred;
            transfer_record.speed_bytes_per_sec = tracker.speed_bytes_per_sec();
            transfer_record.eta_seconds = tracker.eta_seconds();
            transfer_record.active_chunk_index = Some(slice.index + 1);
            transfer_record.updated_at = Utc::now();
            db.upsert_transfer(&transfer_record)?;
        }

        // Finalize whole-file verification & Manifest
        let full_sha256 = hex_encode(full_hasher.finalize());
        manifest.full_sha256 = full_sha256.clone();
        manifest.validate()?;

        let manifest_json = manifest.to_json()?;
        db.save_manifest(file_id, manifest.manifest_version, &manifest_json)?;

        // Create Logical File entry
        let logical_file = LogicalFile {
            id: file_id,
            folder_id: params.folder_id,
            name: params.file_name,
            size: file_size,
            sha256: full_sha256,
            mime_type: None,
            is_favorite: false,
            is_encrypted: params.passphrase.is_some(),
            manifest_id: Some(file_id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        db.insert_file(&logical_file)?;
        let _ = db.record_recent_access(file_id, RecentAction::Uploaded);

        // Mark transfer completed
        transfer_record.state = TransferState::Completed;
        transfer_record.transferred_bytes = file_size;
        transfer_record.speed_bytes_per_sec = 0;
        transfer_record.eta_seconds = None;
        transfer_record.updated_at = Utc::now();
        db.upsert_transfer(&transfer_record)?;

        Ok(logical_file)
    }

    /// Executes a streaming file download with optional AES-256-GCM chunk decryption and full SHA-256 integrity verification.
    pub async fn execute_download(
        storage: Arc<dyn StorageProvider>,
        db: Arc<DatabaseRepository>,
        params: DownloadParams,
        mut signal_rx: watch::Receiver<TransferSignal>,
    ) -> Result<()> {
        let file = db
            .get_file(params.file_id)?
            .ok_or_else(|| TeleCloudError::Database {
                operation: "get_file_for_download".to_string(),
                message: format!("File {} not found", params.file_id),
                recoverable: false,
            })?;

        let manifest_json = db
            .get_manifest(params.file_id)?
            .ok_or_else(|| TeleCloudError::Manifest {
                operation: "get_manifest_for_download".to_string(),
                message: format!("Manifest for file {} not found", params.file_id),
                recoverable: false,
            })?;

        let manifest = FileManifest::from_json(&manifest_json)?;

        // Setup Decryption Key if file was encrypted
        let derived_key = if let Some(ref enc) = manifest.encryption {
            let pass = params.passphrase.as_ref().ok_or_else(|| TeleCloudError::Crypto {
                operation: "verify_passphrase".to_string(),
                message: "Encryption passphrase is required to download this file".to_string(),
                recoverable: false,
            })?;

            let salt_bytes = hex_decode(&enc.salt)?;
            if salt_bytes.len() != SALT_LEN {
                return Err(TeleCloudError::Crypto {
                    operation: "decode_salt".to_string(),
                    message: "Invalid salt length in manifest".to_string(),
                    recoverable: false,
                });
            }

            let mut salt_arr = [0u8; SALT_LEN];
            salt_arr.copy_from_slice(&salt_bytes);
            Some(derive_key(pass, &salt_arr)?)
        } else {
            None
        };

        // Ensure parent directory exists
        if let Some(parent) = params.dest_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| TeleCloudError::Io {
                operation: "create_dest_parent_dir".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;
        }

        // Open destination file
        let mut dest_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&params.dest_path)
            .await
            .map_err(|e| TeleCloudError::Io {
                operation: "create_dest_file".to_string(),
                message: e.to_string(),
                recoverable: false,
            })?;

        let mut transfer_record = db
            .get_transfer(params.transfer_id)?
            .unwrap_or_else(|| TransferRecord {
                id: params.transfer_id,
                file_id: Some(file.id),
                direction: TransferDirection::Download,
                local_path: params.dest_path.to_string_lossy().to_string(),
                file_name: file.name.clone(),
                total_bytes: file.size,
                transferred_bytes: 0,
                state: TransferState::Downloading,
                speed_bytes_per_sec: 0,
                eta_seconds: None,
                active_chunk_index: Some(0),
                total_chunks: Some(manifest.total_chunks),
                error_message: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });

        transfer_record.state = TransferState::Downloading;
        transfer_record.updated_at = Utc::now();
        db.upsert_transfer(&transfer_record)?;

        let mut tracker = ProgressTracker::new(file.size, 0);
        let mut total_transferred = 0u64;

        for chunk in &manifest.chunks {
            // Signal Check
            loop {
                let current_sig = *signal_rx.borrow();
                match current_sig {
                    TransferSignal::Cancelled => {
                        transfer_record.state = TransferState::Cancelled;
                        transfer_record.updated_at = Utc::now();
                        let _ = db.upsert_transfer(&transfer_record);
                        return Err(TeleCloudError::Transfer {
                            operation: "download_chunk".to_string(),
                            message: "Transfer cancelled by user".to_string(),
                            recoverable: false,
                        });
                    }
                    TransferSignal::Paused => {
                        transfer_record.state = TransferState::Paused;
                        transfer_record.updated_at = Utc::now();
                        let _ = db.upsert_transfer(&transfer_record);
                        if signal_rx.changed().await.is_err() {
                            return Err(TeleCloudError::Transfer {
                                operation: "download_chunk".to_string(),
                                message: "Signal channel closed while paused".to_string(),
                                recoverable: false,
                            });
                        }
                    }
                    TransferSignal::Running => break,
                }
            }

            let primary_obj = chunk
                .storage_objects
                .iter()
                .find(|o| o.is_primary)
                .or_else(|| chunk.storage_objects.first())
                .ok_or_else(|| TeleCloudError::Storage {
                    operation: "find_storage_chunk".to_string(),
                    message: format!("No storage object found for chunk {}", chunk.index),
                    recoverable: false,
                })?;

            let object_ref = StorageObjectRef {
                provider: primary_obj.provider.clone(),
                channel_id: primary_obj.chat_id,
                object_id: primary_obj.object_id,
                document_id: None,
            };

            let shared_buf = Arc::new(std::sync::Mutex::new(Vec::new()));
            storage
                .download_chunk(&object_ref, Box::new(SharedVecWriter(shared_buf.clone())), None)
                .await?;

            let chunk_buffer = std::mem::take(&mut *shared_buf.lock().unwrap());

            let plaintext = if let Some(ref key) = derived_key {
                if chunk_buffer.len() < NONCE_LEN {
                    return Err(TeleCloudError::Crypto {
                        operation: "read_nonce".to_string(),
                        message: "Chunk buffer too small to contain nonce".to_string(),
                        recoverable: false,
                    });
                }
                let mut nonce = [0u8; NONCE_LEN];
                nonce.copy_from_slice(&chunk_buffer[..NONCE_LEN]);
                let ciphertext = &chunk_buffer[NONCE_LEN..];
                decrypt_chunk(key, chunk.index, &nonce, ciphertext)?
            } else {
                chunk_buffer
            };

            // Verify chunk plaintext hash
            if !IntegrityVerifier::verify_chunk_sha256(&plaintext, &chunk.sha256) {
                transfer_record.state = TransferState::Failed;
                transfer_record.error_message = Some(format!("Chunk {} SHA-256 verification failed", chunk.index));
                transfer_record.updated_at = Utc::now();
                let _ = db.upsert_transfer(&transfer_record);

                return Err(TeleCloudError::Integrity {
                    operation: "verify_chunk_sha256".to_string(),
                    message: format!("Chunk {} hash mismatch against manifest", chunk.index),
                    recoverable: false,
                });
            }

            // Write chunk to destination file
            dest_file
                .seek(SeekFrom::Start(chunk.offset))
                .await
                .map_err(|e| TeleCloudError::Io {
                    operation: "seek_dest_file".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;

            dest_file
                .write_all(&plaintext)
                .await
                .map_err(|e| TeleCloudError::Io {
                    operation: "write_chunk_dest".to_string(),
                    message: e.to_string(),
                    recoverable: false,
                })?;

            total_transferred += chunk.size;
            tracker.update(total_transferred);

            transfer_record.transferred_bytes = total_transferred;
            transfer_record.speed_bytes_per_sec = tracker.speed_bytes_per_sec();
            transfer_record.eta_seconds = tracker.eta_seconds();
            transfer_record.active_chunk_index = Some(chunk.index + 1);
            transfer_record.updated_at = Utc::now();
            db.upsert_transfer(&transfer_record)?;
        }

        dest_file.flush().await.map_err(|e| TeleCloudError::Io {
            operation: "flush_dest_file".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        drop(dest_file);

        // Whole-file SHA-256 integrity verification
        let is_valid = IntegrityVerifier::verify_file_sha256(&params.dest_path, &manifest.full_sha256).await?;
        let integrity_rec = telecloud_core::IntegrityRecord {
            file_id: file.id,
            last_verified_at: Utc::now(),
            is_valid,
            details: if is_valid {
                Some("Full SHA-256 matched".to_string())
            } else {
                Some("Full SHA-256 mismatch".to_string())
            },
        };
        let _ = db.record_integrity_check(&integrity_rec);

        if !is_valid {
            transfer_record.state = TransferState::Failed;
            transfer_record.error_message = Some("Whole-file SHA-256 verification failed".to_string());
            transfer_record.updated_at = Utc::now();
            let _ = db.upsert_transfer(&transfer_record);

            return Err(TeleCloudError::Integrity {
                operation: "verify_full_file".to_string(),
                message: "Full file hash does not match manifest".to_string(),
                recoverable: false,
            });
        }

        let _ = db.record_recent_access(file.id, RecentAction::Downloaded);

        transfer_record.state = TransferState::Completed;
        transfer_record.transferred_bytes = file.size;
        transfer_record.speed_bytes_per_sec = 0;
        transfer_record.eta_seconds = None;
        transfer_record.updated_at = Utc::now();
        db.upsert_transfer(&transfer_record)?;

        Ok(())
    }
}
