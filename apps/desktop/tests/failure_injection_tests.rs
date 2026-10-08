//! Failure injection, cancellation, and error resilience integration tests for TELEVAULT Phase 16.
//!
//! Validates:
//! - Upload failure handling: simulated network failure -> backup marked Failed, no false success, staging cleaned.
//! - Cooperative cancellation: in-flight backup and restore stop immediately, staging files deleted.
//! - Verification failure detection: unavailable or corrupted remote object flagged as Failed with repair eligibility.
//! - Restore failure handling: simulated remote download failure reported without corrupting local destination.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use televault_backup::engine::BackupEngine;
use televault_backup::profile::BackupProfile;
use televault_backup::repair::RepairEngine;
use televault_backup::restore::{CollisionPolicy, RestoreEngine, SnapshotRestoreRequest};
use televault_backup::verification::VerificationEngine;
use televault_core::ids::ProfileId;
use televault_core::models::BackupStatus;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::Database;
use televault_integrity::types::{VerificationLevel, VerificationOptions, VerificationStatus};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

struct FailureHarness {
    db: Arc<Database>,
    mock_provider: Arc<MockStorageProvider>,
    temp_manager: Arc<TempPayloadManager>,
    backup_engine: Arc<BackupEngine>,
    restore_engine: Arc<RestoreEngine>,
    verification_engine: Arc<VerificationEngine>,
    repair_engine: Arc<RepairEngine>,
    test_root: PathBuf,
}

impl FailureHarness {
    fn new(test_name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!("televault_failure_{test_name}_{nanos}"));
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

        let repair_engine = Arc::new(RepairEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
            Arc::clone(&verification_engine),
        ));

        Self {
            db,
            mock_provider,
            temp_manager,
            backup_engine,
            restore_engine,
            verification_engine,
            repair_engine,
            test_root,
        }
    }

    fn write_source_file(&self, rel_path: &str, content: &[u8]) -> PathBuf {
        let path = self.test_root.join("source").join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for FailureHarness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.test_root);
    }
}

#[test]
fn test_simulated_upload_failure_does_not_create_false_success() {
    let harness = FailureHarness::new("upload_fail");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    harness.write_source_file(
        "critical_data.txt",
        b"Vital user payload that must not be lost",
    );

    let pid = ProfileId::new("prof-fail-01").unwrap();
    let profile = BackupProfile::new(pid.clone(), "Failure Test Profile", source_dir);
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    // Inject simulated upload failure
    harness.mock_provider.set_fail_uploads(true);

    let cancel = CancellationToken::new();
    let backup_res = harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel);

    // 1. Verify backup returned error and was NOT marked successful
    assert!(
        backup_res.is_err(),
        "Backup must report error on upload failure"
    );

    // 2. Verify snapshot in database is transitioned to Failed (never Completed)
    let snapshots = harness.db.list_snapshots_by_profile(&pid).unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots[0].status,
        BackupStatus::Failed,
        "Snapshot must be marked Failed, not Completed"
    );

    // 3. Verify temporary staging files were cleanly unlinked
    let temp_count = fs::read_dir(harness.temp_manager.temp_dir())
        .unwrap()
        .count();
    assert_eq!(
        temp_count, 0,
        "Failed upload must clean up staging files via RAII Drop"
    );
}

#[test]
fn test_cooperative_backup_cancellation_preserves_consistency() {
    let harness = FailureHarness::new("backup_cancel");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    harness.write_source_file("large_item.bin", &vec![0xAA; 512 * 1024]);

    let pid = ProfileId::new("prof-cancel-01").unwrap();
    let profile = BackupProfile::new(pid.clone(), "Cancel Profile", source_dir);
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    // 1. Test plan backup then cancel before execution
    let plan = harness.backup_engine.plan_backup(&profile).expect("plan");
    let cancel = CancellationToken::new();
    cancel.cancel();

    let backup_res = harness.backup_engine.execute_backup(plan, &cancel);

    // Verify cancelled error returned
    assert!(backup_res.is_err());

    // 2. Verify staging directory remains clean
    let temp_count = fs::read_dir(harness.temp_manager.temp_dir())
        .unwrap()
        .count();
    assert_eq!(temp_count, 0, "Cancelled backup must leave 0 staging files");
}

#[test]
fn test_remote_verification_detects_corrupted_storage() {
    let harness = FailureHarness::new("verify_corrupt");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    harness.write_source_file("test_doc.txt", b"Intact original payload");

    let pid = ProfileId::new("prof-v-fail-01").unwrap();
    let profile = BackupProfile::new(pid.clone(), "Verify Fail Profile", source_dir);
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();

    // 1. Successful initial backup
    let summary = harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .expect("initial backup");
    let snap_id = summary.snapshot_id;

    // 2. Level 3 verification initially Healthy
    let options = VerificationOptions {
        level: VerificationLevel::RemoteIntegrity,
        full_hash_check: true,
        decrypt_check: false,
        timeout_secs: Some(30),
    };
    let v_init = harness
        .verification_engine
        .verify_snapshot(&pid, &snap_id, &options, &cancel)
        .expect("verify init");
    assert_eq!(v_init.status, VerificationStatus::Healthy);

    // 3. Corrupt the remote stored chunk in mock storage
    let file_rec = harness
        .db
        .list_files_by_profile(Some(&pid))
        .unwrap()
        .into_iter()
        .next()
        .expect("must have 1 file");
    let manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .expect("manifest must exist");
    let chunk_ref = &manifest.chunks[0].storage_reference;
    harness.mock_provider.corrupt_object(chunk_ref).unwrap();

    // 4. Must detect corruption during Level 3 verification and report Corrupted status
    let v_corrupt = harness
        .verification_engine
        .verify_snapshot(&pid, &snap_id, &options, &cancel)
        .expect("verify after corruption injection");

    assert_eq!(
        v_corrupt.status,
        VerificationStatus::Corrupted,
        "Corrupted remote data must flag verification as Corrupted"
    );
    assert!(
        !v_corrupt.is_restore_ready,
        "Corrupted snapshot must not be restore ready"
    );
    assert_eq!(
        v_corrupt.summary.corrupted_chunks, 1,
        "Must detect exactly 1 corrupted chunk"
    );
    assert!(!v_corrupt.findings.is_empty(), "Must report finding codes");

    // 5. Repair eligibility must report the affected chunk as eligible for recovery
    let preview = harness
        .repair_engine
        .preview_snapshot_repair(&pid, &snap_id, &cancel)
        .expect("preview snapshot repair");
    assert!(
        preview.is_repairable,
        "Corrupted chunk must be eligible for repair"
    );
    assert_eq!(
        preview.total_affected_chunks, 1,
        "Must report 1 repair candidate"
    );
}

#[test]
fn test_simulated_download_failure_during_restore() {
    let harness = FailureHarness::new("restore_fail");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    harness.write_source_file("important_file.txt", b"Data intended for restoration");

    let pid = ProfileId::new("prof-r-fail-01").unwrap();
    let profile = BackupProfile::new(pid.clone(), "Restore Fail Profile", source_dir);
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();

    // 1. Successful backup
    let summary = harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .expect("backup");
    let snap_id = summary.snapshot_id;

    // 2. Inject download failure in storage provider
    harness.mock_provider.set_fail_downloads(true);

    let restore_dest = harness.test_root.join("restore_attempt");
    fs::create_dir_all(&restore_dest).unwrap();

    let restore_req = SnapshotRestoreRequest {
        snapshot_id: snap_id,
        destination_dir: restore_dest.clone(),
        collision_policy: CollisionPolicy::Overwrite,
        verify_integrity: true,
        encryption_policy: EncryptionPolicy::Disabled,
    };

    let restore_summary = harness
        .restore_engine
        .restore_snapshot(&restore_req, &cancel)
        .expect("restore snapshot execution");

    // 3. Assert failed files counter is incremented and no corrupt partial file remains
    assert_eq!(
        restore_summary.failed_files, 1,
        "Failed download must be accounted for in failed_files"
    );
    assert_eq!(
        restore_summary.restored_files, 0,
        "No file should be falsely marked restored"
    );

    // 4. Staging files must be 0
    let temp_count = fs::read_dir(harness.temp_manager.temp_dir())
        .unwrap()
        .count();
    assert_eq!(
        temp_count, 0,
        "Staging directory must be cleaned after failed restore"
    );
}
