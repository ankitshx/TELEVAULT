//! End-to-end Verification + Repair lifecycle integration tests.
//!
//! Validates:
//! - Full workflow: Backup -> Verification (Healthy) -> Bitrot/Damage Injection ->
//!   Verification Failure (Corrupted) -> Repair Preview -> Execution of Repair ->
//!   Re-verification (Healthy) -> Restore Readiness -> Byte-for-byte Restore
//! - Strict Profile Ownership Isolation during Repair (cross-profile repair rejection)
//! - No leakage of temporary staging files during repair

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use televault_backup::engine::BackupEngine;
use televault_backup::profile::BackupProfile;
use televault_backup::repair::RepairEngine;
use televault_backup::restore::{CollisionPolicy, RestoreEngine, SnapshotRestoreRequest};
use televault_backup::verification::VerificationEngine;
use televault_core::ids::ProfileId;
use televault_crypto::policy::EncryptionPolicy;
use televault_db::Database;
use televault_integrity::types::{VerificationLevel, VerificationOptions, VerificationStatus};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

struct VerificationRepairHarness {
    db: Arc<Database>,
    mock_provider: Arc<MockStorageProvider>,
    _temp_manager: Arc<TempPayloadManager>,
    backup_engine: Arc<BackupEngine>,
    restore_engine: Arc<RestoreEngine>,
    verification_engine: Arc<VerificationEngine>,
    repair_engine: Arc<RepairEngine>,
    test_root: PathBuf,
    staging_dir: PathBuf,
}

impl VerificationRepairHarness {
    fn new(test_name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_root =
            std::env::temp_dir().join(format!("televault_v_repair_{test_name}_{nanos}"));
        fs::create_dir_all(&test_root).unwrap();

        let staging_dir = test_root.join("temp_staging");
        fs::create_dir_all(&staging_dir).unwrap();

        let db = Arc::new(Database::open_in_memory().expect("open in-memory db"));
        let mock_provider = Arc::new(MockStorageProvider::new());
        let transfer_config = TransferEngineConfig::default();
        let transfer_engine = Arc::new(TransferEngine::new(
            Arc::clone(&mock_provider) as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            transfer_config,
            None,
        ));
        let temp_manager = Arc::new(TempPayloadManager::new(&staging_dir));

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
            _temp_manager: temp_manager,
            backup_engine,
            restore_engine,
            verification_engine,
            repair_engine,
            test_root,
            staging_dir,
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
}

impl Drop for VerificationRepairHarness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.test_root);
    }
}

#[test]
fn test_e2e_verification_repair_reverification_and_restore_cycle() {
    let harness = VerificationRepairHarness::new("full_cycle");
    let source_dir = harness.test_root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let doc_content =
        b"CRITICAL USER PAYLOAD: Must survive simulated damage, repair, and restoration exactly.";
    harness.write_source_file("critical_document.pdf", doc_content);

    let pid = ProfileId::new("prof-repair-cycle").unwrap();
    let profile = BackupProfile::new(pid.clone(), "Repair Cycle Profile", source_dir.clone());
    harness.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();

    // 1. Initial Backup
    let summary = harness
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .expect("initial backup");
    let snap_id = summary.snapshot_id;
    assert_eq!(summary.new_files, 1);

    // 2. Initial Level 3 Verification -> Healthy & Restore Ready
    let options = VerificationOptions {
        level: VerificationLevel::RemoteIntegrity,
        full_hash_check: true,
        decrypt_check: false,
        timeout_secs: Some(30),
    };

    let v_init = harness
        .verification_engine
        .verify_snapshot(&pid, &snap_id, &options, &cancel)
        .expect("verify snapshot initial");
    assert_eq!(v_init.status, VerificationStatus::Healthy);
    assert!(v_init.is_restore_ready);

    // 3. Inject Bitrot / Remote Damage by corrupting the chunk payload in mock storage
    let file_rec = harness
        .db
        .list_files_by_profile(Some(&pid))
        .unwrap()
        .into_iter()
        .next()
        .expect("file record");
    let manifest = harness
        .db
        .get_manifest_by_file_id(&file_rec.file_id)
        .unwrap()
        .expect("manifest");
    let chunk_ref = &manifest.chunks[0].storage_reference;
    harness.mock_provider.corrupt_object(chunk_ref).unwrap();

    // 4. Verification must detect corruption -> Corrupted status & NOT restore ready
    let v_damaged = harness
        .verification_engine
        .verify_snapshot(&pid, &snap_id, &options, &cancel)
        .expect("verify after damage injection");
    assert_eq!(v_damaged.status, VerificationStatus::Corrupted);
    assert!(!v_damaged.is_restore_ready);
    assert_eq!(v_damaged.summary.corrupted_chunks, 1);

    // 5. Repair Preview: Must confirm repair eligibility and identify 1 candidate
    let preview = harness
        .repair_engine
        .preview_snapshot_repair(&pid, &snap_id, &cancel)
        .expect("preview snapshot repair");
    assert!(preview.is_repairable);
    assert_eq!(preview.total_affected_chunks, 1);
    assert_eq!(preview.eligible_candidates.len(), 1);

    // 6. Execute Repair: repair_snapshot
    let repair_result = harness
        .repair_engine
        .repair_snapshot(&pid, &snap_id, false, &cancel)
        .expect("execute repair");
    assert_eq!(repair_result.repaired_chunks, 1);
    assert_eq!(repair_result.failed_chunks, 0);

    // 7. Re-verification: Must now be Healthy and Restore Ready!
    let v_post_repair = harness
        .verification_engine
        .verify_snapshot(&pid, &snap_id, &options, &cancel)
        .expect("verify after repair");
    assert_eq!(
        v_post_repair.status,
        VerificationStatus::Healthy,
        "Snapshot must be Healthy after successful repair"
    );
    assert!(
        v_post_repair.is_restore_ready,
        "Snapshot must be Restore Ready after successful repair"
    );
    assert_eq!(v_post_repair.summary.corrupted_chunks, 0);

    // 8. Restore the repaired snapshot and verify byte-for-byte exact content
    let restore_dest = harness.test_root.join("restored_output");
    let restore_req = SnapshotRestoreRequest {
        snapshot_id: snap_id.clone(),
        destination_dir: restore_dest.clone(),
        collision_policy: CollisionPolicy::Overwrite,
        verify_integrity: true,
        encryption_policy: EncryptionPolicy::Disabled,
    };

    let restore_result = harness
        .restore_engine
        .restore_snapshot(&restore_req, &cancel)
        .expect("restore repaired snapshot");
    assert_eq!(restore_result.restored_files, 1);
    assert_eq!(restore_result.failed_files, 0);

    let restored_file = restore_dest.join("critical_document.pdf");
    assert!(restored_file.exists());
    let restored_bytes = fs::read(&restored_file).unwrap();
    assert_eq!(restored_bytes, doc_content);

    // 9. Staging directory must be clean (0 leaked temp files)
    let leaked_temp = fs::read_dir(&harness.staging_dir).unwrap().count();
    assert_eq!(
        leaked_temp, 0,
        "No staging files leaked during repair & restore"
    );
}

#[test]
fn test_repair_strict_ownership_isolation() {
    let harness = VerificationRepairHarness::new("ownership_isolation");
    let source_dir_a = harness.test_root.join("source_a");
    let source_dir_b = harness.test_root.join("source_b");
    fs::create_dir_all(&source_dir_a).unwrap();
    fs::create_dir_all(&source_dir_b).unwrap();

    harness.write_source_file("source_a/confidential.txt", b"Profile A Confidential Data");

    let pid_a = ProfileId::new("prof-owner-a").unwrap();
    let profile_a = BackupProfile::new(pid_a.clone(), "Profile A", source_dir_a);
    harness
        .db
        .create_profile(&profile_a.to_db_record())
        .unwrap();

    let pid_b = ProfileId::new("prof-owner-b").unwrap();
    let profile_b = BackupProfile::new(pid_b.clone(), "Profile B", source_dir_b);
    harness
        .db
        .create_profile(&profile_b.to_db_record())
        .unwrap();

    let cancel = CancellationToken::new();

    // Backup Profile A
    let summary_a = harness
        .backup_engine
        .execute_profile_backup(&profile_a, &cancel)
        .expect("backup A");
    let snap_id_a = summary_a.snapshot_id;

    // Profile B attempts to preview or repair Profile A's snapshot: MUST FAIL
    let preview_res = harness
        .repair_engine
        .preview_snapshot_repair(&pid_b, &snap_id_a, &cancel);
    assert!(
        preview_res.is_err(),
        "Cross-profile repair preview must be rejected"
    );

    let repair_res = harness
        .repair_engine
        .repair_snapshot(&pid_b, &snap_id_a, false, &cancel);
    assert!(
        repair_res.is_err(),
        "Cross-profile repair execution must be rejected"
    );
}
