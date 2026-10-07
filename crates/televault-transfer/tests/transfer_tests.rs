//! Comprehensive integration and resource-efficiency tests for televault-transfer.

use std::fs;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use televault_core::ids::{ChunkId, FileId, JobId};
use televault_core::TransferDirection;
use televault_manifest::StorageReference;
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};
use televault_transfer::error::TransferError;
use televault_transfer::job::{DownloadJobParams, TransferJob, UploadJobParams};
use televault_transfer::progress::{ProgressCallback, TransferProgress};
use televault_transfer::retry::RetryPolicy;
use televault_transfer::state::TransferState;

/// Virtual zero-RAM generator stream representing multi-gigabyte files.
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

/// Bounded counting writer that does not retain bytes in memory.
struct NullCountingWriter {
    total_written: u64,
}

impl NullCountingWriter {
    fn new() -> Self {
        Self { total_written: 0 }
    }
}

impl Write for NullCountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.total_written += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_upload_success_and_progress_tracking() {
    let mock_provider = Arc::new(MockStorageProvider::new());
    let progress_events = Arc::new(AtomicUsize::new(0));
    let progress_events_clone = Arc::clone(&progress_events);

    let progress_cb: ProgressCallback = Arc::new(move |p: TransferProgress| {
        progress_events_clone.fetch_add(1, Ordering::SeqCst);
        if p.state == TransferState::Completed {
            assert_eq!(p.percentage(), 100.0);
        }
    });

    let config = TransferEngineConfig {
        max_concurrent_transfers: 2,
        retry_policy: RetryPolicy::default(),
    };
    let engine = TransferEngine::new(mock_provider, config, Some(progress_cb));

    let payload = b"Hello, TELEVAULT Transfer Engine upload!";
    let mut reader = &payload[..];

    let mut job = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("job-up-1").unwrap(),
        FileId::new("file-up-1").unwrap(),
        ChunkId::new("chunk-up-1").unwrap(),
        payload.len() as u64,
    ))
    .unwrap();

    let cancel = CancellationToken::new();
    let storage_ref = engine
        .upload_stream(&mut job, &mut reader, &cancel)
        .expect("upload must succeed");

    assert_eq!(job.state, TransferState::Completed);
    assert_eq!(job.bytes_transferred, payload.len() as u64);
    assert!(matches!(storage_ref, StorageReference::Telegram { .. }));
    assert!(progress_events.load(Ordering::SeqCst) > 0);
}

#[test]
fn test_download_success_and_integrity_verification() {
    let mock_provider: Arc<dyn televault_storage::StorageProvider + Send + Sync> =
        Arc::new(MockStorageProvider::new());
    let config = TransferEngineConfig::default();
    let engine = TransferEngine::new(mock_provider, config, None);

    // 1. Initial upload to create remote object
    let payload = b"Payload destined for download roundtrip";
    let mut reader = &payload[..];
    let mut up_job = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("job-up-dl").unwrap(),
        FileId::new("file-up-dl").unwrap(),
        ChunkId::new("chunk-up-dl").unwrap(),
        payload.len() as u64,
    ))
    .unwrap();

    let cancel = CancellationToken::new();
    let storage_ref = engine
        .upload_stream(&mut up_job, &mut reader, &cancel)
        .expect("upload");

    // 2. Download from remote reference
    let mut dl_job = TransferJob::new_download(DownloadJobParams::new(
        JobId::new("job-dl-1").unwrap(),
        FileId::new("file-up-dl").unwrap(),
        ChunkId::new("chunk-up-dl").unwrap(),
        storage_ref,
        payload.len() as u64,
    ))
    .unwrap();

    let mut downloaded = Vec::new();
    let bytes_dl = engine
        .download_stream(&mut dl_job, &mut downloaded, &cancel)
        .expect("download");

    assert_eq!(bytes_dl, payload.len() as u64);
    assert_eq!(dl_job.state, TransferState::Completed);
    assert_eq!(downloaded, payload);
}

#[test]
fn test_upload_retry_and_eventual_success() {
    let mock = MockStorageProvider::new();
    let mock_provider: Arc<dyn televault_storage::StorageProvider + Send + Sync> =
        Arc::new(mock.clone());
    let config = TransferEngineConfig {
        max_concurrent_transfers: 1,
        retry_policy: RetryPolicy::new(3, 10, 50, 2.0),
    };
    let engine = TransferEngine::new(mock_provider, config, None);

    let payload = b"Resilient retry payload";

    let mut job = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("job-retry-ok").unwrap(),
        FileId::new("file-retry-ok").unwrap(),
        ChunkId::new("chunk-retry-ok").unwrap(),
        payload.len() as u64,
    ))
    .unwrap();

    let cancel = CancellationToken::new();

    // 1. Simulate transient failure on first try
    mock.set_fail_uploads(true);
    let mut reader1 = &payload[..];
    let err1 = engine.upload_stream(&mut job, &mut reader1, &cancel);
    assert!(err1.is_err());
    assert_eq!(job.state, TransferState::Retrying);
    assert_eq!(job.retry_count, 1);

    // 2. Clear failure -> retry succeeds
    mock.set_fail_uploads(false);
    let mut reader2 = &payload[..];
    let ok2 = engine.upload_stream(&mut job, &mut reader2, &cancel);
    assert!(ok2.is_ok());
    assert_eq!(job.state, TransferState::Completed);
}

#[test]
fn test_upload_retry_exhaustion_leads_to_failed_state() {
    let mock = MockStorageProvider::new();
    mock.set_fail_uploads(true);
    let mock_provider: Arc<dyn televault_storage::StorageProvider + Send + Sync> = Arc::new(mock);
    let config = TransferEngineConfig {
        max_concurrent_transfers: 1,
        retry_policy: RetryPolicy::new(2, 5, 20, 2.0),
    };
    let engine = TransferEngine::new(mock_provider, config, None);

    let payload = b"Always failing payload";
    let mut job = TransferJob::new_upload(
        UploadJobParams::new(
            JobId::new("job-exhaust").unwrap(),
            FileId::new("file-exhaust").unwrap(),
            ChunkId::new("chunk-exhaust").unwrap(),
            payload.len() as u64,
        )
        .with_max_retries(2),
    )
    .unwrap();

    let cancel = CancellationToken::new();

    // Attempt 1: retryable failure -> Retrying (retry_count 1)
    let mut reader1 = &payload[..];
    assert!(engine
        .upload_stream(&mut job, &mut reader1, &cancel)
        .is_err());
    assert_eq!(job.state, TransferState::Retrying);

    // Attempt 2: retryable failure -> Retrying (retry_count 2)
    let mut reader2 = &payload[..];
    assert!(engine
        .upload_stream(&mut job, &mut reader2, &cancel)
        .is_err());
    assert_eq!(job.state, TransferState::Retrying);

    // Attempt 3: retries exhausted -> Failed
    let mut reader3 = &payload[..];
    let res3 = engine.upload_stream(&mut job, &mut reader3, &cancel);
    assert!(res3.is_err());
    assert_eq!(job.state, TransferState::Failed);
}

#[test]
fn test_upload_cancellation_stops_transfer() {
    let mock_provider = Arc::new(MockStorageProvider::new());
    let config = TransferEngineConfig::default();
    let engine = TransferEngine::new(mock_provider, config, None);

    let payload = b"Data to be cancelled before upload";
    let mut reader = &payload[..];

    let mut job = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("job-cancel-up").unwrap(),
        FileId::new("file-cancel-up").unwrap(),
        ChunkId::new("chunk-cancel-up").unwrap(),
        payload.len() as u64,
    ))
    .unwrap();

    let cancel = CancellationToken::new();
    cancel.cancel(); // Pre-cancelled

    let err = engine.upload_stream(&mut job, &mut reader, &cancel);
    assert!(matches!(err, Err(TransferError::Cancelled)));
    assert_eq!(job.state, TransferState::Cancelled);
}

#[test]
fn test_queue_lifecycle_and_bounded_concurrency() {
    let mock_provider = Arc::new(MockStorageProvider::new());
    let config = TransferEngineConfig {
        max_concurrent_transfers: 2,
        retry_policy: RetryPolicy::default(),
    };
    let engine = TransferEngine::new(mock_provider, config, None);

    let j1 = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("q-job-1").unwrap(),
        FileId::new("q-file-1").unwrap(),
        ChunkId::new("q-chunk-1").unwrap(),
        100,
    ))
    .unwrap();

    let j2 = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("q-job-2").unwrap(),
        FileId::new("q-file-2").unwrap(),
        ChunkId::new("q-chunk-2").unwrap(),
        200,
    ))
    .unwrap();

    let j3 = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("q-job-3").unwrap(),
        FileId::new("q-file-3").unwrap(),
        ChunkId::new("q-chunk-3").unwrap(),
        300,
    ))
    .unwrap();

    engine.submit(j1).unwrap();
    engine.submit(j2).unwrap();
    engine.submit(j3).unwrap();

    let queue = engine.queue();
    assert_eq!(queue.pending_count(), 3);
    assert_eq!(queue.active_count(), 0);

    // Slot 1
    let acq1 = queue.acquire_next_job();
    assert!(acq1.is_some());
    assert_eq!(queue.active_count(), 1);

    // Slot 2
    let acq2 = queue.acquire_next_job();
    assert!(acq2.is_some());
    assert_eq!(queue.active_count(), 2);

    // Slot 3 blocked by max_concurrent_transfers = 2
    let acq3 = queue.acquire_next_job();
    assert!(acq3.is_none());

    // Release slot 1
    let (mut done_job, _) = acq1.unwrap();
    done_job.state = TransferState::Completed;
    queue.finish_job(done_job);

    // Now slot 3 is available
    let acq3_retry = queue.acquire_next_job();
    assert!(acq3_retry.is_some());
}

#[test]
fn test_large_chunk_1_8gb_streaming_bounded_memory() {
    let mock_provider: Arc<dyn televault_storage::StorageProvider + Send + Sync> =
        Arc::new(MockStorageProvider::new());
    let config = TransferEngineConfig::default();
    let engine = TransferEngine::new(mock_provider, config, None);

    // 1.8 GB chunk size (1,887,436,800 bytes)
    let chunk_size_1_8_gb: u64 = 1_887_436_800;
    let mut virtual_stream = VirtualRepeatingStream::new(chunk_size_1_8_gb, 0x7E);

    let mut job = TransferJob::new_upload(
        UploadJobParams::new(
            JobId::new("job-1-8gb").unwrap(),
            FileId::new("file-video-large").unwrap(),
            ChunkId::new("chunk-0").unwrap(),
            chunk_size_1_8_gb,
        )
        .with_chunk(0, 3),
    )
    .unwrap();

    let cancel = CancellationToken::new();

    // Must stream in bounded 64 KiB chunks without allocating 1.8 GB of RAM
    let storage_ref = engine
        .upload_stream(&mut job, &mut virtual_stream, &cancel)
        .expect("1.8 GB upload streaming must succeed");

    assert_eq!(job.bytes_transferred, chunk_size_1_8_gb);
    assert_eq!(job.state, TransferState::Completed);

    // Download stream through NullCountingWriter
    let mut dl_job = TransferJob::new_download(
        DownloadJobParams::new(
            JobId::new("job-dl-1-8gb").unwrap(),
            FileId::new("file-video-large").unwrap(),
            ChunkId::new("chunk-0").unwrap(),
            storage_ref,
            chunk_size_1_8_gb,
        )
        .with_chunk(0, 3),
    )
    .unwrap();

    let mut counting_writer = NullCountingWriter::new();
    let downloaded_bytes = engine
        .download_stream(&mut dl_job, &mut counting_writer, &cancel)
        .expect("1.8 GB download streaming must succeed");

    assert_eq!(downloaded_bytes, chunk_size_1_8_gb);
    assert_eq!(counting_writer.total_written, chunk_size_1_8_gb);
    assert_eq!(dl_job.state, TransferState::Completed);
}

#[test]
fn test_large_logical_file_5_2gb_three_chunks() {
    let mock_provider = Arc::new(MockStorageProvider::new());
    let config = TransferEngineConfig::default();
    let engine = TransferEngine::new(mock_provider, config, None);
    let cancel = CancellationToken::new();

    let file_id = FileId::new("file-movie-5-2gb").unwrap();
    let total_chunks = 3;

    // 5.2 GB = Chunk 0 (1.8 GB) + Chunk 1 (1.8 GB) + Chunk 2 (1.6 GB)
    let chunk_sizes = [
        1_887_436_800u64, // Chunk 0
        1_887_436_800u64, // Chunk 1
        1_808_583_885u64, // Chunk 2
    ];

    let mut storage_refs = Vec::new();

    for (idx, &size) in chunk_sizes.iter().enumerate() {
        let mut stream = VirtualRepeatingStream::new(size, (idx as u8) + 1);
        let mut job = TransferJob::new_upload(
            UploadJobParams::new(
                JobId::new(format!("job-chunk-{idx}")).unwrap(),
                file_id.clone(),
                ChunkId::new(format!("chunk-{idx}")).unwrap(),
                size,
            )
            .with_chunk(idx as u32, total_chunks),
        )
        .unwrap();

        let sref = engine
            .upload_stream(&mut job, &mut stream, &cancel)
            .unwrap_or_else(|e| panic!("Chunk {idx} upload failed: {e}"));

        assert_eq!(job.bytes_transferred, size);
        assert_eq!(job.state, TransferState::Completed);
        storage_refs.push(sref);
    }

    assert_eq!(storage_refs.len(), 3);
}

#[test]
fn test_staged_upload_lifecycle_cleans_temporary_file() {
    let mock_provider = Arc::new(MockStorageProvider::new());
    let config = TransferEngineConfig::default();
    let engine = TransferEngine::new(mock_provider, config, None);

    let temp_dir = std::env::temp_dir().join("televault_test_transfer_staged_cleanup");
    let manager = TempPayloadManager::new(&temp_dir);

    // 1. Create temporary staging file
    let staging_file = manager
        .create_staging_file("upload")
        .expect("create staging");
    let staged_path = staging_file.path().to_path_buf();
    assert!(staged_path.exists());

    // 2. Write payload into staging file
    let payload = b"Staged chunk waiting for cloud upload and cleanup";
    fs::write(&staged_path, payload).expect("write staged data");
    assert_eq!(
        fs::metadata(&staged_path).unwrap().len(),
        payload.len() as u64
    );

    let mut job = TransferJob::new_upload(
        UploadJobParams::new(
            JobId::new("job-staged-clean").unwrap(),
            FileId::new("file-staged-clean").unwrap(),
            ChunkId::new("chunk-staged-clean").unwrap(),
            payload.len() as u64,
        )
        .with_source_path(staged_path.clone()),
    )
    .unwrap();

    let cancel = CancellationToken::new();

    // 3. Execute staged upload -> remote upload + remote verify + cleanup temporary staging payload
    let storage_ref = engine
        .execute_staged_upload(&mut job, staging_file, &cancel)
        .expect("staged upload");

    assert!(matches!(storage_ref, StorageReference::Telegram { .. }));
    assert_eq!(job.state, TransferState::Completed);
    assert!(
        !staged_path.exists(),
        "Temporary staging file must be deleted upon verified remote upload"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_staged_upload_failure_raii_drop_cleans_temporary_file() {
    let mock = MockStorageProvider::new();
    mock.set_fail_uploads(true);
    let mock_provider: Arc<dyn televault_storage::StorageProvider + Send + Sync> = Arc::new(mock);

    let config = TransferEngineConfig {
        max_concurrent_transfers: 1,
        retry_policy: RetryPolicy::new(0, 10, 50, 1.0), // No retries
    };
    let engine = TransferEngine::new(mock_provider, config, None);

    let temp_dir = std::env::temp_dir().join("televault_test_transfer_staged_drop");
    let manager = TempPayloadManager::new(&temp_dir);

    let staging_file = manager.create_staging_file("fail").expect("create staging");
    let staged_path = staging_file.path().to_path_buf();
    fs::write(&staged_path, b"Doomed payload").expect("write payload");
    assert!(staged_path.exists());

    let mut job = TransferJob::new_upload(
        UploadJobParams::new(
            JobId::new("job-staged-fail").unwrap(),
            FileId::new("file-staged-fail").unwrap(),
            ChunkId::new("chunk-staged-fail").unwrap(),
            14,
        )
        .with_source_path(staged_path.clone())
        .with_max_retries(0),
    )
    .unwrap();

    let cancel = CancellationToken::new();
    let res = engine.execute_staged_upload(&mut job, staging_file, &cancel);
    assert!(res.is_err());
    assert_eq!(job.state, TransferState::Failed);

    // Staging file must have been deleted by RAII Drop on staging_file scope exit
    assert!(
        !staged_path.exists(),
        "Temporary staging file must be cleaned by RAII Drop on failure"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_database_transfer_job_sync() {
    let mock_provider: Arc<dyn televault_storage::StorageProvider + Send + Sync> =
        Arc::new(MockStorageProvider::new());
    let config = TransferEngineConfig::default();
    let engine = TransferEngine::new(mock_provider, config, None);

    // Initialize in-memory SQLite database from televault-db
    let db = televault_db::Database::open_in_memory().expect("open in-memory db");

    // Insert requisite foreign key parent profile & file records
    let profile = televault_db::models::ProfileRecord {
        profile_id: televault_core::ids::ProfileId::new("prof-sync").unwrap(),
        name: "Sync Profile".into(),
        description: None,
        source_path: "C:\\Users\\Test\\Data".into(),
        enabled: true,
        created_at: "2026-10-07T12:00:00Z".into(),
        updated_at: "2026-10-07T12:00:00Z".into(),
    };
    db.create_profile(&profile).unwrap();

    let file = televault_db::models::FileRecord {
        file_id: FileId::new("file-sync-1").unwrap(),
        profile_id: Some(profile.profile_id.clone()),
        file_name: "doc.pdf".into(),
        relative_path: "doc.pdf".into(),
        original_size: 1024,
        mime_type: None,
        status: "active".into(),
        logical_file_hash: None,
        created_at: "2026-10-07T12:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-07T12:00:00Z".into(),
        updated_timestamp: "2026-10-07T12:00:00Z".into(),
    };
    db.create_file(&file).unwrap();

    let manifest = televault_db::models::ManifestRecord {
        manifest_id: "man-sync-1".into(),
        file_id: file.file_id.clone(),
        manifest_version: "1.0.0".into(),
        serialized_manifest: "{}".into(),
        created_at: "2026-10-07T12:00:00Z".into(),
        updated_at: "2026-10-07T12:00:00Z".into(),
    };
    db.create_manifest(&manifest).unwrap();

    let chunk = televault_db::models::ChunkRecord {
        chunk_id: ChunkId::new("chunk-sync-1").unwrap(),
        file_id: file.file_id.clone(),
        manifest_id: "man-sync-1".into(),
        chunk_index: 0,
        plaintext_size: 1024,
        stored_size: 1024,
        integrity_hash: "hash".into(),
        storage_reference: "ref".into(),
        status: "pending".into(),
        created_at: "2026-10-07T12:00:00Z".into(),
        updated_at: "2026-10-07T12:00:00Z".into(),
    };
    db.create_chunk(&chunk).unwrap();

    // Register job in engine
    let job = TransferJob::new_upload(UploadJobParams::new(
        JobId::new("job-sync-1").unwrap(),
        file.file_id.clone(),
        ChunkId::new("chunk-sync-1").unwrap(),
        1024,
    ))
    .unwrap();

    engine.submit(job.clone()).unwrap();

    // Sync to DB
    engine
        .sync_job_to_db(&db, &job.job_id)
        .expect("sync job to db");

    // Read back from DB
    let record = db
        .get_transfer_job(&job.job_id)
        .expect("query db")
        .expect("job exists in db");
    assert_eq!(record.job_id, job.job_id);
    assert_eq!(record.file_id, file.file_id);
    assert_eq!(record.direction, TransferDirection::Upload);
}
