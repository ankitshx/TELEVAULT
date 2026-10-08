//! TelegramStorageProvider bridging Telegram transport to the general StorageProvider contract.

use crate::contracts::{TelegramChunkHeader, TelegramStorageConfig};
use crate::error::TelegramError;
use crate::reference::TelegramReference;
use crate::transport::TelegramTransport;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use televault_storage::{
    DeleteRequest, DownloadRequest, DownloadResult, RemoteObjectMetadata, StorageError,
    StorageProvider, StorageStatus, UploadRequest, UploadResult, VerificationRequest,
};

/// Storage provider implementing [`StorageProvider`] on top of a [`TelegramTransport`].
///
/// Encapsulates all Telegram cloud protocol details, caption tagging, document
/// message operations, and reference conversion.
#[derive(Debug, Clone)]
pub struct TelegramStorageProvider<T: TelegramTransport> {
    transport: T,
    config: TelegramStorageConfig,
}

impl<T: TelegramTransport> TelegramStorageProvider<T> {
    /// Creates a new [`TelegramStorageProvider`].
    pub fn new(transport: T, config: TelegramStorageConfig) -> Self {
        Self { transport, config }
    }

    /// Returns a reference to the inner transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Returns the active storage configuration.
    pub fn config(&self) -> &TelegramStorageConfig {
        &self.config
    }

    /// Tests connection and access to the configured target chat without creating backup payloads.
    pub fn test_connection(
        &self,
    ) -> Result<crate::transport::TelegramConnectionInfo, StorageError> {
        self.transport
            .test_connection(self.config.target_chat_id)
            .map_err(StorageError::from)
    }
}

/// Helper reader calculating running SHA-256 while streaming bytes.
struct HashingReader<'a> {
    inner: &'a mut dyn Read,
    hasher: Sha256,
    total_bytes: u64,
}

impl<'a> HashingReader<'a> {
    fn new(inner: &'a mut dyn Read) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            total_bytes: 0,
        }
    }

    fn finalize_hash(self) -> (String, u64) {
        let hex = format!("{:x}", self.hasher.finalize());
        (hex, self.total_bytes)
    }
}

impl<'a> Read for HashingReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.hasher.update(&buf[..n]);
            self.total_bytes += n as u64;
        }
        Ok(n)
    }
}

/// Helper writer calculating running SHA-256 while streaming bytes.
struct HashingWriter<'a> {
    inner: &'a mut dyn Write,
    hasher: Sha256,
    total_bytes: u64,
}

impl<'a> HashingWriter<'a> {
    fn new(inner: &'a mut dyn Write) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            total_bytes: 0,
        }
    }

    fn finalize_hash(self) -> (String, u64) {
        let hex = format!("{:x}", self.hasher.finalize());
        (hex, self.total_bytes)
    }
}

impl<'a> Write for HashingWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        if n > 0 {
            self.hasher.update(&buf[..n]);
            self.total_bytes += n as u64;
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<T: TelegramTransport> StorageProvider for TelegramStorageProvider<T> {
    fn name(&self) -> &'static str {
        "telegram-cloud-storage"
    }

    fn upload(
        &self,
        request: &UploadRequest,
        reader: &mut dyn Read,
    ) -> Result<UploadResult, StorageError> {
        let header = TelegramChunkHeader {
            format_version: 1,
            file_id: request.file_id.clone(),
            chunk_id: request.chunk_id.clone(),
            chunk_index: request.chunk_index,
            total_chunks: request.total_chunks,
            size_bytes: request.expected_size_bytes,
        };

        let caption = header.to_caption();
        let file_name = format!("{}_{}.chunk", request.file_id.as_str(), request.chunk_index);

        let mut hashing_reader = HashingReader::new(reader);
        let doc = self
            .transport
            .send_document(
                self.config.target_chat_id,
                &caption,
                &file_name,
                &mut hashing_reader,
            )
            .map_err(StorageError::from)?;

        let (calculated_hash, bytes_read) = hashing_reader.finalize_hash();

        // Verify size
        if request.expected_size_bytes > 0 && bytes_read != request.expected_size_bytes {
            return Err(StorageError::UploadFailed {
                reason: format!(
                    "Uploaded bytes mismatch: read {bytes_read}, expected {}",
                    request.expected_size_bytes
                ),
                retryable: false,
            });
        }

        // Verify hash if specified
        if let Some(ref expected_hash) = request.expected_sha256 {
            if !calculated_hash.eq_ignore_ascii_case(expected_hash) {
                return Err(StorageError::VerificationFailed {
                    reason: format!(
                        "Upload SHA-256 mismatch: calculated '{calculated_hash}', expected '{expected_hash}'"
                    ),
                });
            }
        }

        let reference = TelegramReference::new(doc.chat_id, doc.message_id, doc.file_id)
            .map_err(StorageError::from)?
            .to_storage_reference();

        Ok(UploadResult {
            storage_reference: reference,
            bytes_uploaded: bytes_read,
            verified_sha256: Some(calculated_hash),
            remote_status: StorageStatus::Verified,
        })
    }

    fn download(
        &self,
        request: &DownloadRequest,
        writer: &mut dyn Write,
    ) -> Result<DownloadResult, StorageError> {
        let reference = TelegramReference::from_storage_reference(&request.storage_reference)
            .map_err(StorageError::from)?;

        let mut hashing_writer = HashingWriter::new(writer);
        let downloaded_bytes = self
            .transport
            .get_document_stream(
                reference.chat_id,
                reference.message_id,
                &reference.file_id,
                &mut hashing_writer,
            )
            .map_err(StorageError::from)?;

        let (calculated_hash, total_written) = hashing_writer.finalize_hash();
        assert_eq!(downloaded_bytes, total_written);

        if request.expected_size_bytes > 0 && downloaded_bytes != request.expected_size_bytes {
            return Err(StorageError::DownloadFailed {
                reason: format!(
                    "Downloaded size mismatch: received {downloaded_bytes}, expected {}",
                    request.expected_size_bytes
                ),
                retryable: false,
            });
        }

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
            bytes_downloaded: downloaded_bytes,
            verified_sha256: Some(calculated_hash),
        })
    }

    fn verify(&self, request: &VerificationRequest) -> Result<bool, StorageError> {
        let reference = match TelegramReference::from_storage_reference(&request.storage_reference)
        {
            Ok(r) => r,
            Err(_) => return Ok(false),
        };

        match self.transport.get_message_metadata(
            reference.chat_id,
            reference.message_id,
            &reference.file_id,
        ) {
            Ok(doc) => {
                if request.expected_size_bytes > 0 && doc.size_bytes != request.expected_size_bytes
                {
                    return Ok(false);
                }

                if let Some(ref expected_hash) = request.expected_sha256 {
                    let mut sink = std::io::sink();
                    let mut hashing_writer = HashingWriter::new(&mut sink);
                    if self
                        .transport
                        .get_document_stream(
                            reference.chat_id,
                            reference.message_id,
                            &reference.file_id,
                            &mut hashing_writer,
                        )
                        .is_err()
                    {
                        return Ok(false);
                    }
                    let (calculated, _) = hashing_writer.finalize_hash();
                    if !calculated.eq_ignore_ascii_case(expected_hash) {
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            Err(TelegramError::MessageNotFound { .. } | TelegramError::DocumentNotFound { .. }) => {
                Ok(false)
            }
            Err(e) => Err(e.into()),
        }
    }

    fn get_metadata(
        &self,
        reference: &televault_manifest::StorageReference,
    ) -> Result<RemoteObjectMetadata, StorageError> {
        let tg_ref =
            TelegramReference::from_storage_reference(reference).map_err(StorageError::from)?;

        let doc = self
            .transport
            .get_message_metadata(tg_ref.chat_id, tg_ref.message_id, &tg_ref.file_id)
            .map_err(StorageError::from)?;

        Ok(RemoteObjectMetadata {
            remote_id: format!("{}:{}", doc.chat_id, doc.message_id),
            size_bytes: doc.size_bytes,
            sha256_hash: None,
            storage_reference: reference.clone(),
            created_at: None,
        })
    }

    fn delete(&self, request: &DeleteRequest) -> Result<(), StorageError> {
        let tg_ref = TelegramReference::from_storage_reference(&request.storage_reference)
            .map_err(StorageError::from)?;

        self.transport
            .delete_message(tg_ref.chat_id, tg_ref.message_id)
            .map_err(StorageError::from)?;

        Ok(())
    }
}
