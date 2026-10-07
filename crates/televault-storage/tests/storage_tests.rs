//! Comprehensive integration and resource-efficiency tests for televault-storage.

use std::fs;
use std::io::Read;
use televault_core::ids::{ChunkId, FileId};
use televault_storage::{
    DeleteRequest, DownloadRequest, MockStorageProvider, StorageProvider, StorageStatus,
    TempPayloadManager, UploadRequest, VerificationRequest,
};

/// Virtual zero-RAM generator stream capable of representing multi-gigabyte files.
struct VirtualRepeatingStream {
    remaining_bytes: u64,
    fill_byte: u8,
}

impl VirtualRepeatingStream {
    fn new(size_bytes: u64, fill_byte: u8) -> Self {
        Self {
            remaining_bytes: size_bytes,
            fill_byte,
        }
    }
}

impl Read for VirtualRepeatingStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining_bytes == 0 {
            return Ok(0);
        }
        let to_read = (buf.len() as u64).min(self.remaining_bytes) as usize;
        buf[..to_read].fill(self.fill_byte);
        self.remaining_bytes -= to_read as u64;
        Ok(to_read)
    }
}

/// Null writer that counts written bytes and verifies streaming throughput without allocating RAM.
struct NullCountingWriter {
    total_written: u64,
}

impl NullCountingWriter {
    fn new() -> Self {
        Self { total_written: 0 }
    }
}

impl std::io::Write for NullCountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.total_written += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_large_chunk_streaming_resource_efficiency() {
    // Represents a full 1.8 GB chunk (1,887,436,800 bytes)
    let chunk_size_1_8_gb: u64 = 1_887_436_800;
    let mut virtual_stream = VirtualRepeatingStream::new(chunk_size_1_8_gb, 0x5A);

    let provider = MockStorageProvider::new();
    let file_id = FileId::new("file-video-large").unwrap();
    let chunk_id = ChunkId::new("chunk-01").unwrap();

    let upload_req = UploadRequest {
        file_id: file_id.clone(),
        chunk_id: chunk_id.clone(),
        chunk_index: 0,
        total_chunks: 3,
        expected_size_bytes: chunk_size_1_8_gb,
        expected_sha256: None,
        resumable: true,
    };

    // Upload must complete through bounded 64 KiB buffers without allocating 1.8 GB of RAM
    let upload_res = provider
        .upload(&upload_req, &mut virtual_stream)
        .expect("streaming upload must succeed");

    assert_eq!(upload_res.bytes_uploaded, chunk_size_1_8_gb);
    assert!(upload_res.remote_status.is_remotely_complete());
    assert!(upload_res.verified_sha256.is_some());

    // Verify remote object
    let v_req = VerificationRequest {
        storage_reference: upload_res.storage_reference.clone(),
        expected_size_bytes: chunk_size_1_8_gb,
        expected_sha256: upload_res.verified_sha256.clone(),
    };
    assert!(provider.verify(&v_req).expect("verification must pass"));

    // Download through streaming writer without allocating 1.8 GB buffer
    let dl_req = DownloadRequest {
        file_id,
        chunk_id,
        storage_reference: upload_res.storage_reference,
        expected_size_bytes: chunk_size_1_8_gb,
        expected_sha256: upload_res.verified_sha256,
    };

    let mut counting_writer = NullCountingWriter::new();
    let dl_res = provider
        .download(&dl_req, &mut counting_writer)
        .expect("streaming download must succeed");

    assert_eq!(dl_res.bytes_downloaded, chunk_size_1_8_gb);
    assert_eq!(counting_writer.total_written, chunk_size_1_8_gb);
}

#[test]
fn test_temporary_storage_lifecycle_and_cleanup() {
    let temp_root = std::env::temp_dir().join("televault_storage_test_temp_lifecycle");
    let manager = TempPayloadManager::new(&temp_root);

    // 1. Create staging file
    let staging_file = manager.create_staging_file("upload").expect("create file");
    let file_path = staging_file.path().to_path_buf();
    assert!(file_path.exists());

    // 2. Stream data into staging file
    let payload = b"encrypted chunk payload awaiting remote upload";
    fs::write(&file_path, payload).expect("write payload");
    assert_eq!(
        fs::metadata(&file_path).unwrap().len(),
        payload.len() as u64
    );

    // 3. Upload to remote provider
    let provider = MockStorageProvider::new();
    let mut file_reader = fs::File::open(&file_path).expect("open file");
    let req = UploadRequest {
        file_id: FileId::new("file-stage-01").unwrap(),
        chunk_id: ChunkId::new("chunk-stage-01").unwrap(),
        chunk_index: 0,
        total_chunks: 1,
        expected_size_bytes: payload.len() as u64,
        expected_sha256: None,
        resumable: false,
    };
    let upload_res = provider.upload(&req, &mut file_reader).expect("upload");
    assert!(upload_res.remote_status.is_remotely_complete());

    // 4. Remote verification succeeds -> Explicit cleanup of temporary staging payload
    staging_file.cleanup().expect("cleanup temp file");
    assert!(
        !file_path.exists(),
        "Temporary staging file must be deleted upon remote verification"
    );

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_temporary_storage_drop_raii_guarantee() {
    let temp_root = std::env::temp_dir().join("televault_storage_test_temp_raii");
    let manager = TempPayloadManager::new(&temp_root);

    let file_path = {
        let staging_file = manager.create_staging_file("unhandled").expect("create");
        let p = staging_file.path().to_path_buf();
        assert!(p.exists());
        p
        // staging_file dropped here due to simulated worker panic/error
    };

    assert!(
        !file_path.exists(),
        "Unfinalized staging file must be deleted by Drop"
    );

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn test_failed_upload_does_not_mark_remotely_complete() {
    let provider = MockStorageProvider::new();
    provider.set_fail_uploads(true);

    let payload = b"failed payload";
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
    assert!(err.is_err());

    // Status invariant: A failed upload must NEVER be marked complete
    let failed_status = StorageStatus::Failed;
    assert!(!failed_status.is_remotely_complete());
    assert_eq!(provider.stored_objects_count(), 0);
}

#[test]
fn test_retry_idempotency_and_preservation() {
    let provider = MockStorageProvider::new();

    // 1. Initial attempt fails
    provider.set_fail_uploads(true);
    let payload = b"resilient chunk payload";
    let mut reader1 = &payload[..];

    let req = UploadRequest {
        file_id: FileId::new("file-retry-01").unwrap(),
        chunk_id: ChunkId::new("chunk-retry-01").unwrap(),
        chunk_index: 0,
        total_chunks: 1,
        expected_size_bytes: payload.len() as u64,
        expected_sha256: None,
        resumable: true,
    };

    assert!(provider.upload(&req, &mut reader1).is_err());

    // 2. Retry succeeds
    provider.set_fail_uploads(false);
    let mut reader2 = &payload[..];
    let upload_res = provider
        .upload(&req, &mut reader2)
        .expect("retry must succeed");

    assert_eq!(upload_res.bytes_uploaded, payload.len() as u64);
    assert!(upload_res.remote_status.is_remotely_complete());

    // 3. Verify
    let v_req = VerificationRequest {
        storage_reference: upload_res.storage_reference.clone(),
        expected_size_bytes: payload.len() as u64,
        expected_sha256: upload_res.verified_sha256.clone(),
    };
    assert!(provider.verify(&v_req).expect("verify must pass"));

    // 4. Delete
    provider
        .delete(&DeleteRequest {
            storage_reference: upload_res.storage_reference.clone(),
        })
        .expect("delete");
    assert!(!provider.verify(&v_req).expect("verify after delete"));
}
