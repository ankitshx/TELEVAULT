//! End-to-end integration tests for TELEVAULT Phase 16.
//!
//! Validates the complete backup -> upload -> verification -> restore cycle:
//! Profile creation -> scanning -> compression -> encryption -> chunking -> upload ->
//! 4-level verification -> point-in-time restore -> bit-for-bit SHA-256 comparison.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use televault_backup::checker::BackupChecker;
use televault_backup::engine::BackupEngine;
use televault_backup::profile::BackupProfile;
use televault_backup::restore::{CollisionPolicy, RestoreEngine, SnapshotRestoreRequest};
use televault_backup::verification::VerificationEngine;
use televault_core::ids::ProfileId;
use televault_core::models::{BackupStatus, CompressionAlgorithm};
use televault_crypto::key::SecretKey;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::Database;
use televault_integrity::types::{VerificationLevel, VerificationOptions};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

struct E2eTestContext {
    db: Arc<Database>,
    mock_provider: Arc<MockStorageProvider>,
    temp_manager: Arc<TempPayloadManager>,
    backup_engine: Arc<BackupEngine>,
    restore_engine: Arc<RestoreEngine>,
    verification_engine: Arc<VerificationEngine>,
    checker: Arc<BackupChecker>,
    test_root: PathBuf,
}

impl E2eTestContext {
    fn new(test_name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!("televault_e2e_{test_name}_{nanos}"));
        fs::create_dir_all(&test_root).unwrap();

        let temp_dir = test_root.join("temp_staging");
        fs::create_dir_all(&temp_dir).unwrap();

        let db = Arc::new(Database::open_in_memory().expect("open in-memory db"));
        let mock_provider = Arc::new(MockStorageProvider::new());
        let transfer_config = TransferEngineConfig::default();
        let transfer_engine = Arc::new(TransferEngine::new(
            Arc::clone(&mock_provider) as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            transfer_config,
            None,
        ));
        let temp_manager = Arc::new(TempPayloadManager::new(&temp_dir));

        let backup_engine = Arc::new(BackupEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        ));

        let restore_engine = Arc::new(RestoreEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        ));

        let verification_engine = Arc::new(VerificationEngine::new(
            Arc::clone(&db),
            Arc::clone(&mock_provider) as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            Arc::clone(&temp_manager),
        ));

        let checker = Arc::new(BackupChecker::new(Arc::clone(&db)));

        Self {
            db,
            mock_provider,
            temp_manager,
            backup_engine,
            restore_engine,
            verification_engine,
            checker,
            test_root,
        }
    }

    fn write_source_file(&self, rel_path: &str, content: &[u8]) -> PathBuf {
        let full_path = self.test_root.join("source").join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();
        full_path
    }

    fn sha256_of_file(path: &Path) -> String {
        let data = fs::read(path).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&data);
        format!("{:x}", hasher.finalize())
    }
}

impl Drop for E2eTestContext {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.test_root);
    }
}

#[test]
fn test_e2e_backup_verify_and_restore_cycle() {
    let ctx = E2eTestContext::new("full_cycle");
    let source_dir = ctx.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    // 1. Create original files of various structures
    let f1_content = b"TELEVAULT production hardening end-to-end test payload alpha";
    let f2_content = vec![0xAB; 256 * 1024]; // 256 KiB binary payload
    let f3_content =
        b"Nested document inside subfolder with UTF-8: \xE2\x9C\x94 TELEVAULT \xE2\x9C\xA8";

    ctx.write_source_file("documents/notes.txt", f1_content);
    ctx.write_source_file("media/binary.bin", &f2_content);
    ctx.write_source_file("documents/sub/utf8.md", f3_content);

    // Compute original hashes
    let f1_hash = E2eTestContext::sha256_of_file(&source_dir.join("documents/notes.txt"));
    let f2_hash = E2eTestContext::sha256_of_file(&source_dir.join("media/binary.bin"));
    let f3_hash = E2eTestContext::sha256_of_file(&source_dir.join("documents/sub/utf8.md"));

    // 2. Configure Profile with Zstd compression and AES-256-GCM encryption
    let profile_id = ProfileId::new("prof-e2e-01").unwrap();
    let mut profile =
        BackupProfile::new(profile_id.clone(), "E2E Secure Profile", source_dir.clone());
    profile.compression_algorithm = CompressionAlgorithm::Zstd;
    let enc_key = SecretKey::from_bytes([0x42; 32]);
    let enc_policy = EncryptionPolicy::Enabled(enc_key.clone());
    profile.encryption_policy = enc_policy.clone();

    ctx.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();

    // 3. Execute Backup
    let backup_summary = ctx
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .expect("backup must succeed");

    assert_eq!(backup_summary.status, BackupStatus::Completed);
    assert_eq!(backup_summary.new_files, 3);
    assert_eq!(backup_summary.transferred_chunks, 3);
    assert!(ctx.mock_provider.stored_objects_count() >= 3);

    let snapshot_id = backup_summary.snapshot_id;

    // 4. Remote Verification Across All 4 Levels
    for level in [
        VerificationLevel::MetadataOnly,
        VerificationLevel::RemoteAvailability,
        VerificationLevel::RemoteIntegrity,
        VerificationLevel::RestoreReadiness,
    ] {
        let options = VerificationOptions {
            level,
            full_hash_check: true,
            decrypt_check: true,
            timeout_secs: Some(30),
        };

        let verify_result = ctx
            .verification_engine
            .verify_snapshot(&profile_id, &snapshot_id, &options, &cancel)
            .expect("verification execution");

        assert_eq!(
            verify_result.status,
            televault_integrity::types::VerificationStatus::Healthy,
            "Verification level {:?} must be Healthy",
            level
        );
        assert_eq!(verify_result.findings.len(), 0);
    }

    // 5. Restore Entire Snapshot to a Fresh Directory
    let restore_dest = ctx.test_root.join("restored_output");
    fs::create_dir_all(&restore_dest).unwrap();

    let restore_req = SnapshotRestoreRequest {
        snapshot_id: snapshot_id.clone(),
        destination_dir: restore_dest.clone(),
        collision_policy: CollisionPolicy::Overwrite,
        verify_integrity: true,
        encryption_policy: enc_policy.clone(),
    };

    let restore_summary = ctx
        .restore_engine
        .restore_snapshot(&restore_req, &cancel)
        .expect("snapshot restore must succeed");

    assert_eq!(restore_summary.restored_files, 3);
    assert_eq!(restore_summary.failed_files, 0);

    // 6. Verify Exact Byte-for-Byte SHA-256 Match for All Restored Files
    let r1_path = restore_dest.join("documents/notes.txt");
    let r2_path = restore_dest.join("media/binary.bin");
    let r3_path = restore_dest.join("documents/sub/utf8.md");

    assert!(r1_path.exists(), "Restored notes.txt must exist");
    assert!(r2_path.exists(), "Restored binary.bin must exist");
    assert!(r3_path.exists(), "Restored utf8.md must exist");

    assert_eq!(
        E2eTestContext::sha256_of_file(&r1_path),
        f1_hash,
        "notes.txt SHA-256 match"
    );
    assert_eq!(
        E2eTestContext::sha256_of_file(&r2_path),
        f2_hash,
        "binary.bin SHA-256 match"
    );
    assert_eq!(
        E2eTestContext::sha256_of_file(&r3_path),
        f3_hash,
        "utf8.md SHA-256 match"
    );

    // 7. Verify Temporary Staging Directory is Completely Empty
    let temp_files = fs::read_dir(ctx.temp_manager.temp_dir()).unwrap().count();
    assert_eq!(
        temp_files, 0,
        "Temporary staging directory must be clean after backup and restore"
    );

    // 8. Test Collision Policies
    // a) Skip: Restoring again with Skip should not overwrite
    let skip_req = SnapshotRestoreRequest {
        snapshot_id: snapshot_id.clone(),
        destination_dir: restore_dest.clone(),
        collision_policy: CollisionPolicy::Skip,
        verify_integrity: true,
        encryption_policy: enc_policy.clone(),
    };
    let skip_summary = ctx
        .restore_engine
        .restore_snapshot(&skip_req, &cancel)
        .expect("skip restore");
    assert_eq!(skip_summary.skipped_files, 3);

    // b) KeepBoth: Restoring again with KeepBoth should create (1) suffixed files
    let keep_both_req = SnapshotRestoreRequest {
        snapshot_id: snapshot_id.clone(),
        destination_dir: restore_dest.clone(),
        collision_policy: CollisionPolicy::KeepBoth,
        verify_integrity: true,
        encryption_policy: enc_policy.clone(),
    };
    let keep_both_summary = ctx
        .restore_engine
        .restore_snapshot(&keep_both_req, &cancel)
        .expect("keep both restore");
    assert_eq!(keep_both_summary.kept_both_files, 3);

    let r1_collision = restore_dest.join("documents/notes (1).txt");
    assert!(
        r1_collision.exists(),
        "KeepBoth must produce notes (1).txt on collision"
    );
    assert_eq!(
        E2eTestContext::sha256_of_file(&r1_collision),
        f1_hash,
        "Collided file content must match original"
    );
}

#[test]
fn test_e2e_multi_generation_incremental_backup_and_point_in_time_recovery() {
    let ctx = E2eTestContext::new("incremental_pit");
    let source_dir = ctx.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let f1_path = ctx.write_source_file("file1.txt", b"Gen 1 original content for file 1");
    let f2_path = ctx.write_source_file("file2.txt", b"Gen 1 doomed file 2 to be deleted");
    let f3_path = ctx.write_source_file("file3.txt", b"Gen 1 steady file 3 unchanged");

    let f1_gen1_hash = E2eTestContext::sha256_of_file(&f1_path);
    let f2_gen1_hash = E2eTestContext::sha256_of_file(&f2_path);
    let f3_gen1_hash = E2eTestContext::sha256_of_file(&f3_path);

    let profile_id = ProfileId::new("prof-pit-01").unwrap();
    let profile = BackupProfile::new(profile_id.clone(), "PIT Profile", source_dir.clone());
    ctx.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();

    // Generation 1: Full Backup
    let gen1_summary = ctx
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .expect("gen 1 backup");
    assert_eq!(gen1_summary.new_files, 3);
    let gen1_snap_id = gen1_summary.snapshot_id;

    // Checker asserts no changes yet
    assert!(
        !ctx.checker.is_incremental_backup_needed(&profile).unwrap(),
        "No changes after initial backup"
    );

    // Generation 2: Mutate Filesystem
    // - file1 modified
    // - file2 deleted
    // - file3 untouched
    // - file4 added
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(&f1_path, b"Gen 2 modified file 1 with new content").unwrap();
    fs::remove_file(&f2_path).unwrap();
    let f4_path = ctx.write_source_file("file4.txt", b"Gen 2 brand new file 4");

    let f1_gen2_hash = E2eTestContext::sha256_of_file(&f1_path);
    let f4_gen2_hash = E2eTestContext::sha256_of_file(&f4_path);

    assert!(
        ctx.checker.is_incremental_backup_needed(&profile).unwrap(),
        "Checker detects changes"
    );

    // Generation 2 Backup
    let gen2_summary = ctx
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .expect("gen 2 backup");
    assert_eq!(gen2_summary.new_files, 1, "1 new file (file4)");
    assert_eq!(gen2_summary.modified_files, 1, "1 modified file (file1)");
    assert_eq!(gen2_summary.unchanged_files, 1, "1 unchanged file (file3)");
    assert_eq!(gen2_summary.deleted_files, 1, "1 deleted file (file2)");
    let gen2_snap_id = gen2_summary.snapshot_id;

    // Point-in-Time Restore of Generation 1
    let dest_gen1 = ctx.test_root.join("restore_gen1");
    let res1 = ctx
        .restore_engine
        .restore_snapshot(
            &SnapshotRestoreRequest {
                snapshot_id: gen1_snap_id.clone(),
                destination_dir: dest_gen1.clone(),
                collision_policy: CollisionPolicy::Overwrite,
                verify_integrity: true,
                encryption_policy: EncryptionPolicy::Disabled,
            },
            &cancel,
        )
        .expect("restore gen 1");

    assert_eq!(res1.restored_files, 3);
    assert_eq!(
        E2eTestContext::sha256_of_file(&dest_gen1.join("file1.txt")),
        f1_gen1_hash,
        "Gen 1 file 1 must match Gen 1 content"
    );
    assert_eq!(
        E2eTestContext::sha256_of_file(&dest_gen1.join("file2.txt")),
        f2_gen1_hash,
        "Gen 1 file 2 must exist in Gen 1 restore"
    );
    assert_eq!(
        E2eTestContext::sha256_of_file(&dest_gen1.join("file3.txt")),
        f3_gen1_hash,
        "Gen 1 file 3 must match"
    );
    assert!(
        !dest_gen1.join("file4.txt").exists(),
        "file4 must NOT exist in Gen 1 restore"
    );

    // Point-in-Time Restore of Generation 2
    let dest_gen2 = ctx.test_root.join("restore_gen2");
    let res2 = ctx
        .restore_engine
        .restore_snapshot(
            &SnapshotRestoreRequest {
                snapshot_id: gen2_snap_id.clone(),
                destination_dir: dest_gen2.clone(),
                collision_policy: CollisionPolicy::Overwrite,
                verify_integrity: true,
                encryption_policy: EncryptionPolicy::Disabled,
            },
            &cancel,
        )
        .expect("restore gen 2");

    assert_eq!(res2.restored_files, 3, "Gen 2 has file1, file3, file4");
    assert_eq!(
        E2eTestContext::sha256_of_file(&dest_gen2.join("file1.txt")),
        f1_gen2_hash,
        "Gen 2 file 1 must match modified content"
    );
    assert!(
        !dest_gen2.join("file2.txt").exists(),
        "file2 was deleted in Gen 2 so must NOT exist"
    );
    assert_eq!(
        E2eTestContext::sha256_of_file(&dest_gen2.join("file3.txt")),
        f3_gen1_hash,
        "Gen 2 file 3 unchanged content"
    );
    assert_eq!(
        E2eTestContext::sha256_of_file(&dest_gen2.join("file4.txt")),
        f4_gen2_hash,
        "Gen 2 file 4 must match newly added content"
    );

    // Database integrity check passes
    let integrity = ctx.db.full_integrity_check().expect("integrity check");
    assert_eq!(integrity, vec!["ok".to_string()]);
    let fk = ctx.db.foreign_key_check().expect("fk check");
    assert!(fk.is_empty(), "0 FK violations");
}
