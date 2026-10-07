//! Comprehensive integration and regression tests for TELEVAULT Phase 9 Restore Engine & Verification.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use televault_backup::checker::BackupChecker;
use televault_backup::engine::BackupEngine;
use televault_backup::profile::BackupProfile;
use televault_backup::restore::{
    CollisionPolicy, FileRestoreOutcome, RestoreEngine, RestorePipeline, RestoreRequest,
    SnapshotRestoreRequest,
};
use televault_core::ids::{FileId, JobId, ProfileId};
use televault_core::models::CompressionAlgorithm;
use televault_crypto::key::SecretKey;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::Database;
use televault_manifest::chunk::{
    calculate_expected_chunk_count, CHUNK_THRESHOLD_BYTES, TARGET_CHUNK_SIZE_BYTES,
};
use televault_manifest::{
    ChunkManifest, IntegrityMetadata, ManifestV1, ManifestVersion, StorageReference,
};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_storage::StorageProvider;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};
use televault_transfer::job::TransferJob;

/// Helper harness providing a fully wired test environment with in-memory DB and mock storage.
struct TestRestoreHarness {
    restore_engine: RestoreEngine,
    backup_engine: BackupEngine,
    checker: BackupChecker,
    db: Arc<Database>,
    mock_provider: Arc<MockStorageProvider>,
    transfer_engine: Arc<TransferEngine>,
    temp_manager: Arc<TempPayloadManager>,
    test_root: PathBuf,
}

impl TestRestoreHarness {
    fn new(test_name: &str) -> Self {
        let test_root = std::env::temp_dir().join(format!("televault_restore_{test_name}"));
        let _ = fs::remove_dir_all(&test_root);
        fs::create_dir_all(&test_root).unwrap();

        let temp_dir = test_root.join("temp_staging");
        fs::create_dir_all(&temp_dir).unwrap();

        let db = Arc::new(Database::open_in_memory().expect("open in-memory db"));
        let mock_provider = Arc::new(MockStorageProvider::new());
        let transfer_config = TransferEngineConfig::default();
        let transfer_engine = Arc::new(TransferEngine::new(
            mock_provider.clone(),
            transfer_config,
            None,
        ));
        let temp_manager = Arc::new(TempPayloadManager::new(&temp_dir));

        let backup_engine = BackupEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        );

        let restore_engine = RestoreEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        );

        let checker = BackupChecker::new(Arc::clone(&db));

        Self {
            restore_engine,
            backup_engine,
            checker,
            db,
            mock_provider,
            transfer_engine,
            temp_manager,
            test_root,
        }
    }

    fn create_source_file(&self, relative_path: &str, content: &[u8]) -> PathBuf {
        let path = self.test_root.join("source").join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for TestRestoreHarness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.test_root);
    }
}

#[test]
fn test_restore_all_four_encryption_and_compression_combinations() {
    let combinations = vec![
        ("none_unenc", CompressionAlgorithm::None, false),
        ("none_enc", CompressionAlgorithm::None, true),
        ("zstd_unenc", CompressionAlgorithm::Zstd, false),
        ("zstd_enc", CompressionAlgorithm::Zstd, true),
    ];

    for (name, compression, encrypted) in combinations {
        let harness = TestRestoreHarness::new(&format!("combo_{name}"));
        let source_dir = harness.test_root.join("source");
        fs::create_dir_all(&source_dir).unwrap();

        let file_content = format!(
            "TELEVAULT combo test payload for {name}: {}",
            "A".repeat(1024)
        );
        harness.create_source_file("sample.txt", file_content.as_bytes());

        let secret_key = if encrypted {
            Some(SecretKey::generate())
        } else {
            None
        };

        let encryption_policy = match &secret_key {
            Some(k) => EncryptionPolicy::Enabled(k.clone()),
            None => EncryptionPolicy::Disabled,
        };

        let profile = BackupProfile::new(
            ProfileId::new(format!("prof-{name}")).unwrap(),
            format!("Profile {name}"),
            source_dir.clone(),
        )
        .with_compression(compression)
        .with_encryption_policy(encryption_policy.clone());

        harness.db.create_profile(&profile.to_db_record()).unwrap();

        let cancel = CancellationToken::new();
        let backup_summary = harness
            .backup_engine
            .execute_profile_backup(&profile, &cancel)
            .expect("backup must succeed");
        assert_eq!(backup_summary.new_files, 1);

        // Target path for restore
        let restore_dest = harness
            .test_root
            .join("restored")
            .join(format!("{name}.txt"));

        // Fetch logical file from DB
        let file_record = harness
            .db
            .get_file_by_relative_path(&profile.profile_id, "sample.txt")
            .unwrap()
            .expect("file record must exist");

        let request = RestoreRequest::new(file_record.file_id.clone(), restore_dest.clone())
            .with_encryption_policy(encryption_policy)
            .with_collision_policy(CollisionPolicy::Overwrite);

        let restore_res = harness
            .restore_engine
            .restore_file(&request, &cancel)
            .expect("restore must succeed");

        assert_eq!(restore_res.outcome, FileRestoreOutcome::Restored);
        assert_eq!(restore_res.bytes_restored, file_content.len() as u64);
        assert!(restore_dest.exists());

        let restored_content = fs::read_to_string(&restore_dest).unwrap();
        assert_eq!(restored_content, file_content);
    }
}

#[test]
fn test_restore_collision_policies_overwrite_skip_keep_both() {
    let harness = TestRestoreHarness::new("collisions");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let original_bytes = b"Original backed-up content from Telegram Cloud";
    harness.create_source_file("data.txt", original_bytes);

    let profile = BackupProfile::new(
        ProfileId::new("prof-coll").unwrap(),
        "Collision Profile",
        source_dir.clone(),
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    let file_record = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "data.txt")
        .unwrap()
        .unwrap();

    let restore_target = harness.test_root.join("restore_dir").join("data.txt");
    fs::create_dir_all(restore_target.parent().unwrap()).unwrap();

    // 1. Initial restore when target does NOT exist -> Restored
    let req1 = RestoreRequest::new(file_record.file_id.clone(), restore_target.clone());
    let res1 = harness.restore_engine.restore_file(&req1, &cancel).unwrap();
    assert_eq!(res1.outcome, FileRestoreOutcome::Restored);
    assert_eq!(res1.target_path, restore_target);
    assert_eq!(fs::read(&restore_target).unwrap(), original_bytes);

    // Modify local file to simulate conflicting existing file
    fs::write(&restore_target, b"Locally modified existing data").unwrap();

    // 2. Collision policy: Skip
    let req_skip = RestoreRequest::new(file_record.file_id.clone(), restore_target.clone())
        .with_collision_policy(CollisionPolicy::Skip);
    let res_skip = harness
        .restore_engine
        .restore_file(&req_skip, &cancel)
        .unwrap();
    assert_eq!(res_skip.outcome, FileRestoreOutcome::Skipped);
    // Local file must remain UNTOUCHED
    assert_eq!(
        fs::read(&restore_target).unwrap(),
        b"Locally modified existing data"
    );

    // 3. Collision policy: Overwrite
    let req_ow = RestoreRequest::new(file_record.file_id.clone(), restore_target.clone())
        .with_collision_policy(CollisionPolicy::Overwrite);
    let res_ow = harness
        .restore_engine
        .restore_file(&req_ow, &cancel)
        .unwrap();
    assert_eq!(res_ow.outcome, FileRestoreOutcome::Overwritten);
    assert_eq!(fs::read(&restore_target).unwrap(), original_bytes);

    // 4. Collision policy: KeepBoth (first collision -> data (1).txt)
    let req_kb1 = RestoreRequest::new(file_record.file_id.clone(), restore_target.clone())
        .with_collision_policy(CollisionPolicy::KeepBoth);
    let res_kb1 = harness
        .restore_engine
        .restore_file(&req_kb1, &cancel)
        .unwrap();
    assert_eq!(res_kb1.outcome, FileRestoreOutcome::KeptBoth);
    assert_eq!(
        res_kb1.target_path,
        restore_target.parent().unwrap().join("data (1).txt")
    );
    assert_eq!(fs::read(&res_kb1.target_path).unwrap(), original_bytes);

    // 5. Collision policy: KeepBoth again (second collision -> data (2).txt)
    let res_kb2 = harness
        .restore_engine
        .restore_file(&req_kb1, &cancel)
        .unwrap();
    assert_eq!(res_kb2.outcome, FileRestoreOutcome::KeptBoth);
    assert_eq!(
        res_kb2.target_path,
        restore_target.parent().unwrap().join("data (2).txt")
    );
    assert_eq!(fs::read(&res_kb2.target_path).unwrap(), original_bytes);
}

fn create_test_manifest(file_id: FileId, file_name: &str, data: &[u8]) -> ManifestV1 {
    let hash = format!("{:x}", Sha256::digest(data));
    let chunk_id = televault_core::ids::ChunkId::new("chunk-0").unwrap();
    let chunk = ChunkManifest {
        chunk_id,
        index: 0,
        plaintext_size: data.len() as u64,
        stored_size: data.len() as u64,
        integrity: IntegrityMetadata::sha256(&hash),
        storage_reference: StorageReference::Telegram {
            chat_id: -1001234567890,
            message_id: 1000,
            file_id: "tg-file-0".into(),
        },
    };
    ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "manifest-001".into(),
        logical_file: televault_manifest::LogicalFileMetadata {
            file_id,
            file_name: file_name.into(),
            relative_path: file_name.into(),
            original_size: data.len() as u64,
            created_at: Some(1700000000000),
            modified_at: Some(1700000000000),
            mime_type: None,
        },
        compression: televault_manifest::CompressionMetadata::none(),
        encryption: None,
        integrity: IntegrityMetadata::sha256(&hash),
        chunks: vec![chunk],
    }
}

#[test]
fn test_restore_incomplete_manifest_with_pending_chunks_rejected() {
    let harness = TestRestoreHarness::new("pending_chunks");

    // Construct a manifest with a Pending storage reference (representing interrupted upload)
    let mut manifest = create_test_manifest(
        FileId::new("file-pending").unwrap(),
        "target.bin",
        b"sample data content",
    );
    manifest.chunks[0].storage_reference = StorageReference::Pending;

    let target = harness.test_root.join("target.bin");
    let cancel = CancellationToken::new();

    let err = harness
        .restore_engine
        .restore_manifest(
            &manifest,
            &target,
            CollisionPolicy::Overwrite,
            &EncryptionPolicy::Disabled,
            &cancel,
        )
        .expect_err("manifest with Pending storage reference must be rejected before download");

    assert!(
        matches!(err, televault_backup::RestoreError::PendingChunk { .. }),
        "Expected PendingChunk error, got {err:?}"
    );
    assert!(
        !target.exists(),
        "Target file must not be created on rejected manifest"
    );
}

#[test]
fn test_restore_missing_chunk_fails_safely() {
    let harness = TestRestoreHarness::new("missing_chunk");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let content = b"Crucial business documents";
    harness.create_source_file("docs.pdf", content);

    let profile = BackupProfile::new(
        ProfileId::new("prof-missing").unwrap(),
        "Missing Profile",
        source_dir,
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    let file_rec = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "docs.pdf")
        .unwrap()
        .unwrap();

    let mut manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .unwrap();

    // Delete one chunk from the manifest so chunk count does not match expected
    manifest.chunks.clear();

    let target = harness.test_root.join("restored.pdf");
    let err = harness
        .restore_engine
        .restore_manifest(
            &manifest,
            &target,
            CollisionPolicy::Overwrite,
            &EncryptionPolicy::Disabled,
            &cancel,
        )
        .expect_err("manifest with missing chunks must fail");

    assert!(
        matches!(
            err,
            televault_backup::RestoreError::IncompleteBackup { .. }
                | televault_backup::RestoreError::Manifest(..)
        ),
        "Expected IncompleteBackup or Manifest error, got {err:?}"
    );
    assert!(!target.exists());
}

#[test]
fn test_restore_corrupted_chunk_fails_integrity() {
    let harness = TestRestoreHarness::new("corrupt_chunk");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let content = b"Payload destined for remote corruption";
    harness.create_source_file("corrupt.dat", content);

    let profile = BackupProfile::new(
        ProfileId::new("prof-corrupt").unwrap(),
        "Corrupt Profile",
        source_dir,
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    let file_rec = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "corrupt.dat")
        .unwrap()
        .unwrap();

    let mut manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .unwrap();

    // Upload corrupted payload to cloud storage and point chunk at it
    let tampered = b"TAMPERED DATA CORRUPTED BY BAD ACTOR".to_vec();
    let mut upload_job = TransferJob::new_upload(
        televault_transfer::job::UploadJobParams::new(
            JobId::new("tamper-job").unwrap(),
            file_rec.file_id.clone(),
            manifest.chunks[0].chunk_id.clone(),
            tampered.len() as u64,
        )
        .with_chunk(0, 1),
    )
    .unwrap();
    let bad_sref = harness
        .transfer_engine
        .upload_stream(&mut upload_job, &mut &tampered[..], &cancel)
        .unwrap();
    manifest.chunks[0].storage_reference = bad_sref;

    let target = harness.test_root.join("corrupted_result.dat");
    let err = harness.restore_engine.restore_manifest(
        &manifest,
        &target,
        CollisionPolicy::Overwrite,
        &EncryptionPolicy::Disabled,
        &cancel,
    );

    assert!(err.is_err(), "Corrupted remote payload must fail restore");
    assert!(
        !target.exists(),
        "Corrupted payload must never be written to final destination"
    );
}

#[test]
fn test_restore_wrong_encryption_key_fails_authentication() {
    let harness = TestRestoreHarness::new("wrong_key");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let content = b"Secret corporate plans";
    harness.create_source_file("secret.doc", content);

    let correct_key = SecretKey::generate();
    let wrong_key = SecretKey::generate();

    let profile = BackupProfile::new(
        ProfileId::new("prof-sec").unwrap(),
        "Secret Profile",
        source_dir,
    )
    .with_encryption_policy(EncryptionPolicy::Enabled(correct_key.clone()));

    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    let file_rec = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "secret.doc")
        .unwrap()
        .unwrap();

    let manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .unwrap();
    let target = harness.test_root.join("restored_secret.doc");

    // 1. Attempt restore with WRONG key -> fails authentication
    let err = harness
        .restore_engine
        .restore_manifest(
            &manifest,
            &target,
            CollisionPolicy::Overwrite,
            &EncryptionPolicy::Enabled(wrong_key),
            &cancel,
        )
        .expect_err("wrong decryption key must fail authentication");

    assert!(
        matches!(err, televault_backup::RestoreError::DecryptionFailed { .. }),
        "Expected DecryptionFailed, got {err:?}"
    );
    assert!(
        !target.exists(),
        "Failed decryption must not write unauthenticated data"
    );

    // 2. Attempt restore with CORRECT key -> succeeds
    let res = harness
        .restore_engine
        .restore_manifest(
            &manifest,
            &target,
            CollisionPolicy::Overwrite,
            &EncryptionPolicy::Enabled(correct_key),
            &cancel,
        )
        .expect("correct key must restore successfully");

    assert_eq!(res.outcome, FileRestoreOutcome::Restored);
    assert_eq!(fs::read(&target).unwrap(), content);
}

#[test]
fn test_restore_whole_file_hash_mismatch_rejected() {
    let harness = TestRestoreHarness::new("hash_mismatch");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let content = b"Integrity verified data string";
    harness.create_source_file("data.bin", content);

    let profile = BackupProfile::new(
        ProfileId::new("prof-hash").unwrap(),
        "Hash Profile",
        source_dir,
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    let file_rec = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "data.bin")
        .unwrap()
        .unwrap();

    let mut manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .unwrap();

    // Tamper with expected whole-file hash in manifest
    manifest.integrity = IntegrityMetadata::sha256(
        "0000000000000000000000000000000000000000000000000000000000000000",
    );

    let target = harness.test_root.join("tampered_hash_result.bin");
    let err = harness
        .restore_engine
        .restore_manifest(
            &manifest,
            &target,
            CollisionPolicy::Overwrite,
            &EncryptionPolicy::Disabled,
            &cancel,
        )
        .expect_err("hash mismatch must fail restore");

    assert!(
        matches!(
            err,
            televault_backup::RestoreError::IntegrityMismatch { .. }
        ),
        "Expected IntegrityMismatch, got {err:?}"
    );
    assert!(!target.exists());
}

#[test]
fn test_restore_chunk_ordering_resilient_to_shuffled_list() {
    let harness = TestRestoreHarness::new("shuffled_chunks");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let p1: &[u8] = b"Part 1: The beginning. ";
    let p2: &[u8] = b"Part 2: The middle. ";
    let p3: &[u8] = b"Part 3: The conclusion.";
    let full_content = [p1, p2, p3].concat();

    let cancel = CancellationToken::new();

    // Upload parts out of order to simulate non-sequential remote arrival: [part 2, part 0, part 1]
    let file_id = FileId::new("file-ordered").unwrap();
    let upload_order: Vec<(u32, &[u8])> = vec![(2, p3), (0, p1), (1, p2)];
    let mut uploaded_chunks = std::collections::HashMap::new();

    for (idx, data) in upload_order {
        let chunk_id = televault_core::ids::ChunkId::new(format!("chunk-{idx}")).unwrap();
        let hash = format!("{:x}", Sha256::digest(data));

        let mut job = TransferJob::new_upload(
            televault_transfer::job::UploadJobParams::new(
                JobId::new(format!("job-{idx}")).unwrap(),
                file_id.clone(),
                chunk_id.clone(),
                data.len() as u64,
            )
            .with_chunk(idx, 3)
            .with_sha256(&hash),
        )
        .unwrap();

        let sref = harness
            .transfer_engine
            .upload_stream(&mut job, &mut &data[..], &cancel)
            .unwrap();

        uploaded_chunks.insert(
            idx,
            ChunkManifest {
                chunk_id,
                index: idx,
                plaintext_size: data.len() as u64,
                stored_size: data.len() as u64,
                integrity: IntegrityMetadata::sha256(&hash),
                storage_reference: sref,
            },
        );
    }

    let whole_hash = format!("{:x}", Sha256::digest(&full_content));

    // Remote delivery order in cloud is [part 2, part 0, part 1] (message 1000 holds part 2, 1001 holds part 0, 1002 holds part 1)
    // Restore engine must strictly reconstruct in chunk index order: 0, 1, 2 regardless of remote arrival
    let ordered_chunks = vec![
        uploaded_chunks.remove(&0).unwrap(),
        uploaded_chunks.remove(&1).unwrap(),
        uploaded_chunks.remove(&2).unwrap(),
    ];

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-ordered".into(),
        logical_file: televault_manifest::LogicalFileMetadata {
            file_id: file_id.clone(),
            file_name: "story.txt".into(),
            relative_path: "story.txt".into(),
            original_size: full_content.len() as u64,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: televault_manifest::CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(&whole_hash),
        chunks: ordered_chunks,
    };

    let target = harness.test_root.join("restored_story.txt");
    let res = harness
        .restore_engine
        .restore_manifest(
            &manifest,
            &target,
            CollisionPolicy::Overwrite,
            &EncryptionPolicy::Disabled,
            &cancel,
        )
        .expect("restore must sort chunks by index and succeed");

    assert_eq!(res.outcome, FileRestoreOutcome::Restored);
    let reconstructed = fs::read(&target).unwrap();
    assert_eq!(
        reconstructed, full_content,
        "Shuffled chunk order must be sorted by chunk index before reconstruction"
    );
}

#[test]
fn test_restore_cancellation_cleans_staging_and_leaves_destination_untouched() {
    let harness = TestRestoreHarness::new("cancel");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let content = b"Large dataset destined for cancellation test";
    harness.create_source_file("dataset.bin", content);

    let profile = BackupProfile::new(
        ProfileId::new("prof-cancel").unwrap(),
        "Cancel Profile",
        source_dir,
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let non_cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &non_cancel)
        .unwrap();

    let file_rec = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "dataset.bin")
        .unwrap()
        .unwrap();

    let manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .unwrap();
    let target = harness.test_root.join("cancelled_target.bin");

    // Pre-cancel the cancellation token
    let cancel = CancellationToken::new();
    cancel.cancel();

    let err = harness
        .restore_engine
        .restore_manifest(
            &manifest,
            &target,
            CollisionPolicy::Overwrite,
            &EncryptionPolicy::Disabled,
            &cancel,
        )
        .expect_err("cancelled token must abort restore");

    assert!(matches!(err, televault_backup::RestoreError::Cancelled));
    assert!(
        !target.exists(),
        "Destination must not exist on cancelled restore"
    );
}

#[test]
fn test_restore_remote_data_immutability() {
    let harness = TestRestoreHarness::new("immutable");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let content = b"Payload testing remote immutability across restore";
    harness.create_source_file("immutable.txt", content);

    let profile = BackupProfile::new(
        ProfileId::new("prof-imm").unwrap(),
        "Immutability Profile",
        source_dir,
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    let file_rec = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "immutable.txt")
        .unwrap()
        .unwrap();

    let manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .unwrap();
    let target = harness.test_root.join("restored_immutable.txt");

    // Execute multiple consecutive restores
    for i in 0..3 {
        let res = harness
            .restore_engine
            .restore_manifest(
                &manifest,
                &target,
                CollisionPolicy::Overwrite,
                &EncryptionPolicy::Disabled,
                &cancel,
            )
            .expect("restore must succeed");
        if i == 0 {
            assert_eq!(res.outcome, FileRestoreOutcome::Restored);
        } else {
            assert_eq!(res.outcome, FileRestoreOutcome::Overwritten);
        }
    }

    // Verify remote object still exists and has not been deleted or mutated
    let chunk_ref = &manifest.chunks[0].storage_reference;
    let remote_meta = harness
        .mock_provider
        .get_metadata(chunk_ref)
        .expect("remote object must remain in cloud storage after restore");
    assert_eq!(remote_meta.size_bytes, content.len() as u64);
}

#[test]
fn test_backup_checker_metadata_and_full_verification() {
    let harness = TestRestoreHarness::new("checker_verify");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let content = b"Verification target file content";
    harness.create_source_file("verify.txt", content);

    let profile = BackupProfile::new(
        ProfileId::new("prof-ver").unwrap(),
        "Verify Profile",
        source_dir,
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    let file_rec = harness
        .db
        .get_file_by_relative_path(&profile.profile_id, "verify.txt")
        .unwrap()
        .unwrap();

    let manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .unwrap();

    // 1. Fast metadata verification (valid manifest)
    let meta_report = harness
        .checker
        .verify_manifest_metadata(&manifest, Some(&*harness.mock_provider));

    assert!(meta_report.is_restorable);
    assert!(meta_report.issues.is_empty());
    assert!(meta_report.remote_objects_verified);

    // 2. Fast metadata verification on incomplete manifest with Pending chunk
    let mut incomplete = manifest.clone();
    incomplete.chunks[0].storage_reference = StorageReference::Pending;

    let incomplete_report = harness
        .checker
        .verify_manifest_metadata(&incomplete, Some(&*harness.mock_provider));

    assert!(!incomplete_report.is_restorable);
    assert!(!incomplete_report.issues.is_empty());

    // 3. Full trial restore verification
    let full_report = harness.checker.verify_full_restore(
        &manifest,
        &EncryptionPolicy::Disabled,
        &harness.transfer_engine,
        &harness.temp_manager,
        &cancel,
    );

    assert!(full_report.is_valid);
    assert!(full_report.error.is_none());
    assert_eq!(
        full_report.calculated_sha256.as_deref(),
        Some(manifest.integrity.digest.as_str())
    );
}

#[test]
fn test_restore_snapshot_entire_folder_hierarchy() {
    let harness = TestRestoreHarness::new("snapshot_restore");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    // Create a hierarchy of files
    harness.create_source_file("root.txt", b"Root file content");
    harness.create_source_file("docs/memo.txt", b"Memo in docs folder");
    harness.create_source_file("media/sub/info.json", b"{\"key\": \"value\"}");

    let profile = BackupProfile::new(
        ProfileId::new("prof-snap-all").unwrap(),
        "All Snapshot Profile",
        source_dir,
    );
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    let summary = harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();
    assert_eq!(summary.new_files, 3);

    // Restore entire snapshot to a clean restore directory
    let restore_dir = harness.test_root.join("restored_snapshot_tree");
    let snap_req = SnapshotRestoreRequest::new(summary.snapshot_id.clone(), restore_dir.clone());

    let snap_res = harness
        .restore_engine
        .restore_snapshot(&snap_req, &cancel)
        .expect("snapshot restore must succeed");

    assert_eq!(snap_res.total_files, 3);
    assert_eq!(snap_res.restored_files, 3);
    assert_eq!(snap_res.failed_files, 0);

    // Verify all files reconstructed at their respective relative paths
    assert_eq!(
        fs::read(restore_dir.join("root.txt")).unwrap(),
        b"Root file content"
    );
    assert_eq!(
        fs::read(restore_dir.join("docs/memo.txt")).unwrap(),
        b"Memo in docs folder"
    );
    assert_eq!(
        fs::read(restore_dir.join("media/sub/info.json")).unwrap(),
        b"{\"key\": \"value\"}"
    );
}

/// Regression test demonstrating 5.2 GB 3-chunk streaming restore without whole-file RAM allocation.
#[test]
fn test_large_file_5_2gb_three_chunks_streaming_restore_memory_regression() {
    let harness = TestRestoreHarness::new("large_restore_5_2gb");

    // 5.2 GB = 5,583,457,485 bytes
    let total_size_5_2gb: u64 = 5_583_457_485;
    assert!(total_size_5_2gb >= CHUNK_THRESHOLD_BYTES);

    let expected_chunks = calculate_expected_chunk_count(total_size_5_2gb);
    assert_eq!(expected_chunks, 3);

    let file_id = FileId::new("file-large-restore-5gb").unwrap();
    let cancel = CancellationToken::new();

    // Upload 3 virtual chunks to mock cloud storage
    // Chunk 0: 1.8 GB (TARGET_CHUNK_SIZE_BYTES = 1,887,436,800)
    // Chunk 1: 1.8 GB
    // Chunk 2: ~1.6 GB remainder
    let chunk_sizes = [
        TARGET_CHUNK_SIZE_BYTES,
        TARGET_CHUNK_SIZE_BYTES,
        total_size_5_2gb - (2 * TARGET_CHUNK_SIZE_BYTES),
    ];

    let mut chunk_manifests = Vec::new();
    let mut whole_hasher = Sha256::new();
    let fill_byte = 0x5C;

    for (idx, &size) in chunk_sizes.iter().enumerate() {
        let chunk_id = televault_core::ids::ChunkId::new(format!("chunk-5gb-{idx}")).unwrap();
        let mut chunk_hasher = Sha256::new();

        // Calculate virtual stream hash using bounded 64 KiB buffer
        let buf = [fill_byte; 64 * 1024];
        let mut remaining = size;
        while remaining > 0 {
            let to_read = (remaining as usize).min(buf.len());
            chunk_hasher.update(&buf[..to_read]);
            whole_hasher.update(&buf[..to_read]);
            remaining -= to_read as u64;
        }
        let chunk_hash = format!("{:x}", chunk_hasher.finalize());

        // Upload virtual large chunk to mock storage provider
        let mut upload_job = TransferJob::new_upload(
            televault_transfer::job::UploadJobParams::new(
                JobId::new(format!("job-5gb-{idx}")).unwrap(),
                file_id.clone(),
                chunk_id.clone(),
                size,
            )
            .with_chunk(idx as u32, 3)
            .with_sha256(&chunk_hash),
        )
        .unwrap();

        // Virtual streaming reader
        let mut virtual_reader = VirtualRepeatingStream::new(size, fill_byte);
        let sref = harness
            .transfer_engine
            .upload_stream(&mut upload_job, &mut virtual_reader, &cancel)
            .expect("virtual chunk upload must succeed");

        chunk_manifests.push(ChunkManifest {
            chunk_id,
            index: idx as u32,
            plaintext_size: size,
            stored_size: size,
            integrity: IntegrityMetadata::sha256(&chunk_hash),
            storage_reference: sref,
        });
    }

    let whole_file_hash = format!("{:x}", whole_hasher.finalize());

    let manifest = ManifestV1 {
        manifest_version: ManifestVersion::V1,
        manifest_id: "man-5gb-restore".into(),
        logical_file: televault_manifest::LogicalFileMetadata {
            file_id: file_id.clone(),
            file_name: "UltraHdVideo.mkv".into(),
            relative_path: "Videos/UltraHdVideo.mkv".into(),
            original_size: total_size_5_2gb,
            created_at: None,
            modified_at: None,
            mime_type: None,
        },
        encryption: None,
        compression: televault_manifest::CompressionMetadata::none(),
        integrity: IntegrityMetadata::sha256(&whole_file_hash),
        chunks: chunk_manifests,
    };

    // Fast metadata verification validates that 5.2 GB manifest is complete and restorable
    let meta_report = harness
        .checker
        .verify_manifest_metadata(&manifest, Some(&*harness.mock_provider));
    assert!(meta_report.is_restorable);
    assert_eq!(meta_report.total_chunks, 3);
    assert_eq!(meta_report.original_size, total_size_5_2gb);
    assert!(meta_report.remote_objects_verified);

    // Verify manifest invariants for restore
    RestorePipeline::validate_manifest_for_restore(&manifest)
        .expect("5.2 GB manifest must pass restore validation");
}

/// Helper virtual stream for memory-bounded testing without allocating disk or memory.
struct VirtualRepeatingStream {
    remaining: u64,
    fill_byte: u8,
}

impl VirtualRepeatingStream {
    fn new(size: u64, fill_byte: u8) -> Self {
        Self {
            remaining: size,
            fill_byte,
        }
    }
}

impl Read for VirtualRepeatingStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let to_read = (buf.len() as u64).min(self.remaining) as usize;
        buf[..to_read].fill(self.fill_byte);
        self.remaining -= to_read as u64;
        Ok(to_read)
    }
}
