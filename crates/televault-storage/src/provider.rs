//! StorageProvider abstraction trait for remote cloud and staging backends.

use crate::error::Result;
use crate::types::{
    DeleteRequest, DownloadRequest, DownloadResult, RemoteObjectMetadata, UploadRequest,
    UploadResult, VerificationRequest,
};
use std::io::{Read, Write};
use televault_manifest::StorageReference;

/// Abstraction for a remote cloud storage provider (e.g. Telegram Cloud, Mock).
///
/// Decouples core backup and transfer operations from Telegram-specific APIs.
/// All payload transmissions use streaming I/O (`Read` and `Write`) with bounded
/// internal buffers, guaranteeing that multi-gigabyte files or 1.8 GB chunks are
/// NEVER loaded entirely into memory.
pub trait StorageProvider: Send + Sync {
    /// Returns the human-readable identifier of this storage provider.
    fn name(&self) -> &'static str;

    /// Uploads payload bytes from the provided streaming `reader` to remote cloud storage.
    ///
    /// Reads in bounded chunks (e.g. 64 KiB), calculates running SHA-256 integrity,
    /// transmits to the remote provider, verifies remote storage, and returns an [`UploadResult`].
    fn upload(&self, request: &UploadRequest, reader: &mut dyn Read) -> Result<UploadResult>;

    /// Downloads payload bytes from remote cloud storage and writes directly to `writer`.
    ///
    /// Streams directly from the remote provider to the target writer in bounded chunks,
    /// verifying size and SHA-256 integrity as bytes are received.
    fn download(&self, request: &DownloadRequest, writer: &mut dyn Write)
        -> Result<DownloadResult>;

    /// Verifies the presence and integrity of a remote object identified by its storage reference.
    fn verify(&self, request: &VerificationRequest) -> Result<bool>;

    /// Retrieves remote object metadata (e.g. size, hash, creation timestamp).
    fn get_metadata(&self, reference: &StorageReference) -> Result<RemoteObjectMetadata>;

    /// Deletes an object from remote cloud storage.
    fn delete(&self, request: &DeleteRequest) -> Result<()>;
}
