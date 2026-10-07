//! Lightweight mock storage provider for testing and contract verification.

use crate::error::{Result, StorageError};
use crate::provider::StorageProvider;
use crate::types::{
    DeleteRequest, DownloadRequest, DownloadResult, RemoteObjectMetadata, StorageStatus,
    UploadRequest, UploadResult, VerificationRequest, STREAM_CHUNK_BUFFER_SIZE,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use televault_manifest::StorageReference;

/// Internal payload representation allowing bounded memory for large mock files.
#[derive(Debug, Clone)]
enum MockPayload {
    InMemory(Vec<u8>),
    VirtualLarge { fill_byte: u8 },
}

/// Stored mock object payload and metadata.
#[derive(Debug, Clone)]
struct MockStoredObject {
    payload: MockPayload,
    sha256_hex: String,
    size_bytes: u64,
    created_at: String,
}

/// Thread-safe in-memory mock storage provider.
///
/// Implements [`StorageProvider`] for automated testing without requiring network access
/// or Telegram credentials. Fully supports streaming I/O, failure simulation, and integrity checks.
#[derive(Debug, Clone)]
pub struct MockStorageProvider {
    objects: Arc<Mutex<HashMap<String, MockStoredObject>>>,
    next_message_id: Arc<AtomicI64>,
    target_chat_id: i64,
    fail_uploads: Arc<AtomicBool>,
    fail_downloads: Arc<AtomicBool>,
    fail_verifications: Arc<AtomicBool>,
}

impl MockStorageProvider {
    /// Creates a new [`MockStorageProvider`].
    pub fn new() -> Self {
        Self {
            objects: Arc::new(Mutex::new(HashMap::new())),
            next_message_id: Arc::new(AtomicI64::new(1000)),
            target_chat_id: -1001234567890,
            fail_uploads: Arc::new(AtomicBool::new(false)),
            fail_downloads: Arc::new(AtomicBool::new(false)),
            fail_verifications: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Sets whether subsequent uploads should simulate a failure.
    pub fn set_fail_uploads(&self, fail: bool) {
        self.fail_uploads.store(fail, Ordering::SeqCst);
    }

    /// Sets whether subsequent downloads should simulate a failure.
    pub fn set_fail_downloads(&self, fail: bool) {
        self.fail_downloads.store(fail, Ordering::SeqCst);
    }

    /// Sets whether subsequent verifications should simulate a failure.
    pub fn set_fail_verifications(&self, fail: bool) {
        self.fail_verifications.store(fail, Ordering::SeqCst);
    }

    /// Returns the total number of objects currently held in mock cloud storage.
    pub fn stored_objects_count(&self) -> usize {
        self.objects.lock().unwrap().len()
    }

    /// Inserts a virtual mock object with specified fill byte and precomputed hash.
    /// Used for testing gigabyte-scale payloads (e.g. 1.8 GB chunks) with zero RAM allocation.
    pub fn insert_virtual_object(
        &self,
        reference: StorageReference,
        size_bytes: u64,
        fill_byte: u8,
        sha256_hex: String,
    ) -> Result<()> {
        let key = Self::reference_key(&reference)?;
        let stored_obj = MockStoredObject {
            payload: MockPayload::VirtualLarge { fill_byte },
            sha256_hex,
            size_bytes,
            created_at: "2026-10-07T12:00:00Z".into(),
        };
        self.objects.lock().unwrap().insert(key, stored_obj);
        Ok(())
    }

    /// Inserts an in-memory mock object with payload bytes and calculated hash.
    pub fn insert_in_memory_object(
        &self,
        reference: StorageReference,
        data: Vec<u8>,
    ) -> Result<String> {
        let key = Self::reference_key(&reference)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256_hex = format!("{:x}", hasher.finalize());
        let size_bytes = data.len() as u64;
        let stored_obj = MockStoredObject {
            payload: MockPayload::InMemory(data),
            sha256_hex: sha256_hex.clone(),
            size_bytes,
            created_at: "2026-10-07T12:00:00Z".into(),
        };
        self.objects.lock().unwrap().insert(key, stored_obj);
        Ok(sha256_hex)
    }

    /// Formats a mock storage reference key.
    fn reference_key(reference: &StorageReference) -> Result<String> {
        match reference {
            StorageReference::Telegram {
                chat_id,
                message_id,
                ..
            } => Ok(format!("{chat_id}:{message_id}")),
            StorageReference::LocalStaging { relative_path } => {
                Ok(format!("local:{relative_path}"))
            }
            StorageReference::Pending => Err(StorageError::InvalidReference(
                "Cannot address a pending storage reference".into(),
            )),
        }
    }
}

impl Default for MockStorageProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageProvider for MockStorageProvider {
    fn name(&self) -> &'static str {
        "mock-storage-provider"
    }

    fn upload(&self, request: &UploadRequest, reader: &mut dyn Read) -> Result<UploadResult> {
        if self.fail_uploads.load(Ordering::SeqCst) {
            return Err(StorageError::UploadFailed {
                reason: "Simulated mock upload failure".into(),
                retryable: true,
            });
        }

        let mut buffer = [0u8; STREAM_CHUNK_BUFFER_SIZE];
        let mut hasher = Sha256::new();
        let mut total_read: u64 = 0;
        let is_large_virtual = request.expected_size_bytes > 4 * 1024 * 1024;
        let mut collected_bytes = if is_large_virtual {
            Vec::new()
        } else {
            Vec::with_capacity(request.expected_size_bytes as usize)
        };
        let mut sample_fill_byte = 0u8;

        // Bounded stream processing using STREAM_CHUNK_BUFFER_SIZE stack buffer
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            if total_read == 0 && n > 0 {
                sample_fill_byte = buffer[0];
            }
            hasher.update(&buffer[..n]);
            if !is_large_virtual {
                collected_bytes.extend_from_slice(&buffer[..n]);
            }
            total_read += n as u64;
        }

        // Validate size against request if specified
        if request.expected_size_bytes > 0 && total_read != request.expected_size_bytes {
            return Err(StorageError::UploadFailed {
                reason: format!(
                    "Streamed size mismatch: read {} bytes, expected {} bytes",
                    total_read, request.expected_size_bytes
                ),
                retryable: false,
            });
        }

        let calculated_hash = format!("{:x}", hasher.finalize());

        // Validate hash if expected
        if let Some(ref expected_hash) = request.expected_sha256 {
            if !calculated_hash.eq_ignore_ascii_case(expected_hash) {
                return Err(StorageError::VerificationFailed {
                    reason: format!(
                        "Upload SHA-256 mismatch: calculated '{calculated_hash}', expected '{expected_hash}'"
                    ),
                });
            }
        }

        let msg_id = self.next_message_id.fetch_add(1, Ordering::SeqCst);
        let storage_ref = StorageReference::Telegram {
            chat_id: self.target_chat_id,
            message_id: msg_id,
            file_id: format!("tg_mock_file_{msg_id}"),
        };

        let key = Self::reference_key(&storage_ref)?;
        let payload = if is_large_virtual {
            MockPayload::VirtualLarge {
                fill_byte: sample_fill_byte,
            }
        } else {
            MockPayload::InMemory(collected_bytes)
        };

        let stored_obj = MockStoredObject {
            payload,
            sha256_hex: calculated_hash.clone(),
            size_bytes: total_read,
            created_at: "2026-10-07T12:00:00Z".into(),
        };

        self.objects.lock().unwrap().insert(key, stored_obj);

        Ok(UploadResult {
            storage_reference: storage_ref,
            bytes_uploaded: total_read,
            verified_sha256: Some(calculated_hash),
            remote_status: StorageStatus::Verified,
        })
    }

    fn download(
        &self,
        request: &DownloadRequest,
        writer: &mut dyn Write,
    ) -> Result<DownloadResult> {
        if self.fail_downloads.load(Ordering::SeqCst) {
            return Err(StorageError::DownloadFailed {
                reason: "Simulated mock download failure".into(),
                retryable: true,
            });
        }

        let key = Self::reference_key(&request.storage_reference)?;
        let obj = {
            let guard = self.objects.lock().unwrap();
            guard.get(&key).cloned().ok_or_else(|| {
                StorageError::NotFound(format!("Reference '{key}' not found in mock store"))
            })?
        };

        // Stream bytes to writer in bounded chunks
        let mut total_written: u64 = 0;
        let mut hasher = Sha256::new();

        match obj.payload {
            MockPayload::InMemory(ref bytes) => {
                for chunk in bytes.chunks(STREAM_CHUNK_BUFFER_SIZE) {
                    writer.write_all(chunk)?;
                    hasher.update(chunk);
                    total_written += chunk.len() as u64;
                }
            }
            MockPayload::VirtualLarge { fill_byte } => {
                let chunk_buf = [fill_byte; STREAM_CHUNK_BUFFER_SIZE];
                let mut remaining = obj.size_bytes;
                while remaining > 0 {
                    let to_write = (chunk_buf.len() as u64).min(remaining) as usize;
                    writer.write_all(&chunk_buf[..to_write])?;
                    hasher.update(&chunk_buf[..to_write]);
                    total_written += to_write as u64;
                    remaining -= to_write as u64;
                }
            }
        }

        let calculated_hash = format!("{:x}", hasher.finalize());

        if let Some(ref expected_hash) = request.expected_sha256 {
            if !calculated_hash.eq_ignore_ascii_case(expected_hash) {
                return Err(StorageError::VerificationFailed {
                    reason: format!(
                        "Download SHA-256 mismatch: calculated '{calculated_hash}', expected '{expected_hash}'"
                    ),
                });
            }
        }

        Ok(DownloadResult {
            bytes_downloaded: total_written,
            verified_sha256: Some(calculated_hash),
        })
    }

    fn verify(&self, request: &VerificationRequest) -> Result<bool> {
        if self.fail_verifications.load(Ordering::SeqCst) {
            return Ok(false);
        }

        let key = Self::reference_key(&request.storage_reference)?;
        let guard = self.objects.lock().unwrap();
        if let Some(obj) = guard.get(&key) {
            if request.expected_size_bytes > 0 && obj.size_bytes != request.expected_size_bytes {
                return Ok(false);
            }
            if let Some(ref expected_hash) = request.expected_sha256 {
                if !obj.sha256_hex.eq_ignore_ascii_case(expected_hash) {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn get_metadata(&self, reference: &StorageReference) -> Result<RemoteObjectMetadata> {
        let key = Self::reference_key(reference)?;
        let guard = self.objects.lock().unwrap();
        let obj = guard.get(&key).ok_or_else(|| {
            StorageError::NotFound(format!("Reference '{key}' not found in mock store"))
        })?;

        Ok(RemoteObjectMetadata {
            remote_id: key,
            size_bytes: obj.size_bytes,
            sha256_hash: Some(obj.sha256_hex.clone()),
            storage_reference: reference.clone(),
            created_at: Some(obj.created_at.clone()),
        })
    }

    fn delete(&self, request: &DeleteRequest) -> Result<()> {
        let key = Self::reference_key(&request.storage_reference)?;
        let mut guard = self.objects.lock().unwrap();
        guard.remove(&key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use televault_core::ids::{ChunkId, FileId};

    #[test]
    fn test_mock_storage_upload_and_download_roundtrip() {
        let provider = MockStorageProvider::new();
        let payload = b"Hello TELEVAULT Cloud Storage streaming data!";
        let mut reader = &payload[..];

        let req = UploadRequest {
            file_id: FileId::new("file-01").unwrap(),
            chunk_id: ChunkId::new("chunk-01").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            expected_size_bytes: payload.len() as u64,
            expected_sha256: None,
            resumable: false,
        };

        let upload_res = provider.upload(&req, &mut reader).expect("upload");
        assert_eq!(upload_res.bytes_uploaded, payload.len() as u64);
        assert!(upload_res.remote_status.is_remotely_complete());

        // Verify
        let v_req = VerificationRequest {
            storage_reference: upload_res.storage_reference.clone(),
            expected_size_bytes: payload.len() as u64,
            expected_sha256: upload_res.verified_sha256.clone(),
        };
        assert!(provider.verify(&v_req).expect("verify"));

        // Download
        let dl_req = DownloadRequest {
            file_id: FileId::new("file-01").unwrap(),
            chunk_id: ChunkId::new("chunk-01").unwrap(),
            storage_reference: upload_res.storage_reference.clone(),
            expected_size_bytes: payload.len() as u64,
            expected_sha256: upload_res.verified_sha256,
        };
        let mut downloaded = Vec::new();
        let dl_res = provider
            .download(&dl_req, &mut downloaded)
            .expect("download");
        assert_eq!(dl_res.bytes_downloaded, payload.len() as u64);
        assert_eq!(downloaded, payload);

        // Delete
        provider
            .delete(&DeleteRequest {
                storage_reference: upload_res.storage_reference.clone(),
            })
            .expect("delete");
        assert!(!provider.verify(&v_req).expect("verify after delete"));
    }

    #[test]
    fn test_mock_storage_simulated_failures() {
        let provider = MockStorageProvider::new();
        provider.set_fail_uploads(true);

        let payload = b"test payload";
        let mut reader = &payload[..];
        let req = UploadRequest {
            file_id: FileId::new("file-fail").unwrap(),
            chunk_id: ChunkId::new("chunk-fail").unwrap(),
            chunk_index: 0,
            total_chunks: 1,
            expected_size_bytes: payload.len() as u64,
            expected_sha256: None,
            resumable: false,
        };

        let err = provider.upload(&req, &mut reader);
        assert!(err.is_err(), "Simulated upload failure must fail");
    }
}
