//! Comprehensive integration and regression tests for televault-backup.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use televault_backup::checker::BackupChecker;
use televault_backup::engine::BackupEngine;
use televault_backup::pipeline::{PayloadPipeline, ProcessingOptions};
use televault_backup::profile::BackupProfile;
use televault_core::ids::{FileId, ProfileId};
use televault_core::models::{BackupStatus, CompressionAlgorithm};
use televault_crypto::key::SecretKey;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::Database;
use televault_manifest::chunk::{
    calculate_expected_chunk_count, CHUNK_THRESHOLD_BYTES, TARGET_CHUNK_SIZE_BYTES,
};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

/// Creates a test backup engine backed by an in-memory database and mock storage provider.
fn setup_test_engine(temp_root: &Path) -> (BackupEngine, Arc<Database>, Arc<TempPayloadManager>) {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory db"));
    let mock_provider = Arc::new(MockStorageProvider::new());
    let transfer_config = TransferEngineConfig::default();
    let transfer_engine = Arc::new(TransferEngine::new(mock_provider, transfer_config, None));
    let temp_manager = Arc::new(TempPayloadManager::new(temp_root));

    let backup_engine = BackupEngine::new(
        Arc::clone(&db),
        Arc::clone(&transfer_engine),
        Arc::clone(&temp_manager),
    );

    (backup_engine, db, temp_manager)
}

#[test]
fn test_incremental_backup_regression_scenario() {
    let test_dir = std::env::temp_dir().join("televault_test_incremental_regression");
    let source_dir = test_dir.join("source");
    let temp_dir = test_dir.join("temp");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&source_dir).unwrap();

    let (engine, db, _temp_mgr) = setup_test_engine(&temp_dir);

    // Initial files: file1.txt, file2.txt, file3.txt
    let f1_path = source_dir.join("file1.txt");
    let f2_path = source_dir.join("file2.txt");
    let f3_path = source_dir.join("file3.txt");
    fs::write(&f1_path, b"File 1 permanent content").unwrap();
    fs::write(&f2_path, b"File 2 original content").unwrap();
    fs::write(&f3_path, b"File 3 destined for deletion").unwrap();

    let profile = BackupProfile::new(
        ProfileId::new("prof-inc").unwrap(),
        "Incremental Profile",
        source_dir.clone(),
    );
    db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();

    // 1. Initial full backup execution
    let summary1 = engine
        .execute_profile_backup(&profile, &cancel)
        .expect("initial backup must succeed");

    assert_eq!(summary1.status, BackupStatus::Completed);
    assert_eq!(summary1.new_files, 3);
    assert_eq!(summary1.modified_files, 0);
    assert_eq!(summary1.unchanged_files, 0);
    assert_eq!(summary1.deleted_files, 0);
    assert_eq!(summary1.transferred_chunks, 3);

    // Verify checker reports backup is not needed right now
    let checker = BackupChecker::new(Arc::clone(&db));
    assert!(
        !checker.is_incremental_backup_needed(&profile).unwrap(),
        "No backup needed when nothing changed"
    );

    // 2. Mutate filesystem state:
    // - file1.txt remains untouched
    // - file2.txt modified
    // - file3.txt deleted
    // - file4.txt added
    std::thread::sleep(std::time::Duration::from_millis(50)); // Ensure modified timestamp ticks
    fs::write(&f2_path, b"File 2 MODIFIED content with more bytes!").unwrap();
    fs::remove_file(&f3_path).unwrap();
    let f4_path = source_dir.join("file4.txt");
    fs::write(&f4_path, b"File 4 newly created").unwrap();

    assert!(
        checker.is_incremental_backup_needed(&profile).unwrap(),
        "Backup must be needed after modifications"
    );

    // 3. Incremental backup execution
    let summary2 = engine
        .execute_profile_backup(&profile, &cancel)
        .expect("incremental backup must succeed");

    assert_eq!(summary2.status, BackupStatus::Completed);
    assert_eq!(summary2.new_files, 1, "Only file4 is new");
    assert_eq!(summary2.modified_files, 1, "Only file2 is modified");
    assert_eq!(summary2.unchanged_files, 1, "file1 is unchanged");
    assert_eq!(summary2.deleted_files, 1, "file3 was deleted locally");
    assert_eq!(
        summary2.transferred_chunks, 2,
        "Transferred chunks must only be file2 and file4"
    );
    assert!(
        summary2.reused_bytes > 0,
        "file1 payload bytes were reused without upload"
    );

    // 4. Verify file3 remote backup record was NOT destroyed
    let f3_status = checker
        .check_file_status(&profile.profile_id, "file3.txt")
        .unwrap();
    assert!(f3_status.is_tracked, "file3 must remain tracked in history");
    assert!(
        f3_status.manifest_available,
        "file3 remote manifest must remain available for retention"
    );

    let _ = fs::remove_dir_all(&test_dir);
}

/// Virtual stream representing multi-gigabyte files without heap allocation.
struct VirtualZeroAllocStream {
    remaining_bytes: u64,
    fill_byte: u8,
}

impl VirtualZeroAllocStream {
    fn new(size_bytes: u64, fill_byte: u8) -> Self {
        Self {
            remaining_bytes: size_bytes,
            fill_byte,
        }
    }
}

impl Read for VirtualZeroAllocStream {
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

#[test]
fn test_large_logical_file_5_2gb_three_chunks_pipeline() {
    let temp_dir = std::env::temp_dir().join("televault_test_large_file_pipeline");
    let manager = TempPayloadManager::new(&temp_dir);

    // 5.2 GB = 5,583,457,485 bytes
    let total_size_5_2gb: u64 = 5_583_457_485;
    assert!(total_size_5_2gb >= CHUNK_THRESHOLD_BYTES);

    let expected_chunks = calculate_expected_chunk_count(total_size_5_2gb);
    assert_eq!(
        expected_chunks, 3,
        "5.2 GB must partition into exactly 3 chunks"
    );

    // Virtual repeating stream does not allocate 5.2 GB in memory
    let virtual_stream = VirtualZeroAllocStream::new(total_size_5_2gb, 0xAB);

    let file_id = FileId::new("file-movie-large").unwrap();
    let options = ProcessingOptions::default();

    let processed = PayloadPipeline::process_file(
        &file_id,
        "Movies/BigVideo.mp4",
        virtual_stream,
        total_size_5_2gb,
        &manager,
        &options,
    )
    .expect("5.2 GB streaming pipeline processing must succeed");

    assert_eq!(processed.chunks.len(), 3);
    assert_eq!(processed.total_size, total_size_5_2gb);

    // Chunk 0: 1.8 GB (TARGET_CHUNK_SIZE_BYTES = 1,887,436,800)
    assert_eq!(processed.chunks[0].chunk_index, 0);
    assert_eq!(processed.chunks[0].total_chunks, 3);
    assert_eq!(processed.chunks[0].plaintext_size, TARGET_CHUNK_SIZE_BYTES);
    assert_eq!(processed.chunks[0].stored_size, TARGET_CHUNK_SIZE_BYTES);

    // Chunk 1: 1.8 GB
    assert_eq!(processed.chunks[1].chunk_index, 1);
    assert_eq!(processed.chunks[1].total_chunks, 3);
    assert_eq!(processed.chunks[1].plaintext_size, TARGET_CHUNK_SIZE_BYTES);

    // Chunk 2: Remainder = 1,808,583,885 bytes (~1.68 GB)
    assert_eq!(processed.chunks[2].chunk_index, 2);
    assert_eq!(processed.chunks[2].total_chunks, 3);
    assert_eq!(
        processed.chunks[2].plaintext_size,
        total_size_5_2gb - (2 * TARGET_CHUNK_SIZE_BYTES)
    );

    // Verify manifest
    assert_eq!(processed.manifest.chunks.len(), 3);
    assert_eq!(processed.manifest.logical_file.file_name, "BigVideo.mp4");
    assert_eq!(
        processed.manifest.logical_file.relative_path,
        "Movies/BigVideo.mp4"
    );
    assert_eq!(
        processed.manifest.logical_file.original_size,
        total_size_5_2gb
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_all_four_encryption_and_compression_combinations() {
    let temp_dir = std::env::temp_dir().join("televault_test_four_combinations");
    let manager = TempPayloadManager::new(&temp_dir);
    let payload = b"Universal test payload for encryption and compression matrix";
    let key = SecretKey::generate();

    // 1. Compression OFF + Encryption OFF
    let opt1 = ProcessingOptions {
        encryption_policy: EncryptionPolicy::Disabled,
        compression_algorithm: CompressionAlgorithm::None,
    };
    let res1 = PayloadPipeline::process_file(
        &FileId::new("f-combo-1").unwrap(),
        "c1.txt",
        &payload[..],
        payload.len() as u64,
        &manager,
        &opt1,
    )
    .unwrap();
    assert!(res1.manifest.encryption.is_none());
    assert_eq!(
        res1.manifest.compression.algorithm,
        CompressionAlgorithm::None
    );

    // 2. Compression OFF + Encryption ON
    let opt2 = ProcessingOptions {
        encryption_policy: EncryptionPolicy::Enabled(key.clone()),
        compression_algorithm: CompressionAlgorithm::None,
    };
    let res2 = PayloadPipeline::process_file(
        &FileId::new("f-combo-2").unwrap(),
        "c2.txt",
        &payload[..],
        payload.len() as u64,
        &manager,
        &opt2,
    )
    .unwrap();
    assert!(res2.manifest.encryption.is_some());
    assert_eq!(
        res2.manifest.compression.algorithm,
        CompressionAlgorithm::None
    );

    // 3. Compression ON + Encryption OFF
    let opt3 = ProcessingOptions {
        encryption_policy: EncryptionPolicy::Disabled,
        compression_algorithm: CompressionAlgorithm::Zstd,
    };
    let res3 = PayloadPipeline::process_file(
        &FileId::new("f-combo-3").unwrap(),
        "c3.txt",
        &payload[..],
        payload.len() as u64,
        &manager,
        &opt3,
    )
    .unwrap();
    assert!(res3.manifest.encryption.is_none());
    assert_eq!(
        res3.manifest.compression.algorithm,
        CompressionAlgorithm::Zstd
    );

    // 4. Compression ON + Encryption ON
    let opt4 = ProcessingOptions {
        encryption_policy: EncryptionPolicy::Enabled(key),
        compression_algorithm: CompressionAlgorithm::Zstd,
    };
    let res4 = PayloadPipeline::process_file(
        &FileId::new("f-combo-4").unwrap(),
        "c4.txt",
        &payload[..],
        payload.len() as u64,
        &manager,
        &opt4,
    )
    .unwrap();
    assert!(res4.manifest.encryption.is_some());
    assert_eq!(
        res4.manifest.compression.algorithm,
        CompressionAlgorithm::Zstd
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_empty_and_special_file_boundary_cases() {
    let temp_dir = std::env::temp_dir().join("televault_test_boundary_cases");
    let manager = TempPayloadManager::new(&temp_dir);

    // 1. Empty file (0 bytes)
    let empty_payload: &[u8] = b"";
    let options = ProcessingOptions::default();
    let res = PayloadPipeline::process_file(
        &FileId::new("f-empty").unwrap(),
        "empty.txt",
        empty_payload,
        0,
        &manager,
        &options,
    )
    .expect("empty file must process cleanly into 1 chunk of 0 bytes");

    assert_eq!(res.chunks.len(), 1);
    assert_eq!(res.chunks[0].plaintext_size, 0);
    assert_eq!(res.total_size, 0);
    assert_eq!(
        res.chunks[0].plaintext_sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_backup_cancellation_aborts_cooperatively() {
    let test_dir = std::env::temp_dir().join("televault_test_cancellation");
    let source_dir = test_dir.join("source");
    let temp_dir = test_dir.join("temp");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&source_dir).unwrap();

    let (engine, db, _temp_mgr) = setup_test_engine(&temp_dir);

    fs::write(source_dir.join("work.dat"), b"Important work payload").unwrap();

    let profile = BackupProfile::new(
        ProfileId::new("prof-cancel").unwrap(),
        "Cancel Profile",
        source_dir,
    );
    db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    cancel.cancel(); // Pre-cancelled token

    let result = engine.execute_profile_backup(&profile, &cancel);
    assert!(result.is_err());

    let _ = fs::remove_dir_all(&test_dir);
}
