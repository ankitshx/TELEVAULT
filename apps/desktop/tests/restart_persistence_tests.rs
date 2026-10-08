//! Restart and persistence validation tests for TELEVAULT.
//!
//! Validates:
//! - Full metadata persistence across clean application restart on disk-backed SQLite
//! - Profiles, schedules, snapshots, manifests, chunks survive process shutdown
//! - SQLite full integrity check and foreign key check pass with 0 errors
//! - Interrupted / failed backup state recovery across process restart

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use televault_backup::engine::BackupEngine;
use televault_backup::profile::BackupProfile;
use televault_core::ids::{ProfileId, ScheduleId, SnapshotId};
use televault_core::models::BackupStatus;
use televault_db::{Database, ProfileRecord, ScheduleRecord, SnapshotRecord};
use televault_integrity::types::{VerificationOptions, VerificationStatus};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

fn setup_temp_env(name: &str) -> (PathBuf, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("televault_restart_{name}_{nanos}"));
    fs::create_dir_all(&root).unwrap();
    let db_path = root.join("televault.db");
    (root, db_path)
}

#[test]
fn test_metadata_persistence_across_application_restart() {
    let (root, db_path) = setup_temp_env("clean_restart");
    let source_dir = root.join("source");
    let staging_dir = root.join("staging");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&staging_dir).unwrap();

    let test_file = source_dir.join("persistent_doc.txt");
    fs::write(
        &test_file,
        b"TELEVAULT persistent state validation across restart",
    )
    .unwrap();

    let pid = ProfileId::new("prof-restart-01").unwrap();
    let sched_id = ScheduleId::new("sched-restart-01").unwrap();
    let snap_id_saved: SnapshotId;

    // =========================================================================
    // RUNTIME INSTANCE 1: Create state, run backup, verify, shutdown
    // =========================================================================
    {
        let db = Arc::new(Database::open(&db_path).expect("open disk db instance 1"));
        let mock_provider = Arc::new(MockStorageProvider::new());
        let transfer_config = TransferEngineConfig::default();
        let transfer_engine = Arc::new(TransferEngine::new(
            Arc::clone(&mock_provider) as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            transfer_config,
            None,
        ));
        let temp_manager = Arc::new(TempPayloadManager::new(&staging_dir));
        let backup_engine = BackupEngine::new(
            Arc::clone(&db),
            Arc::clone(&transfer_engine),
            Arc::clone(&temp_manager),
        );

        // 1. Create Profile
        let profile = BackupProfile::new(pid.clone(), "Restart Profile", source_dir.clone());
        db.create_profile(&profile.to_db_record()).unwrap();

        // 2. Create Schedule
        let schedule_rec = ScheduleRecord {
            schedule_id: sched_id.clone(),
            profile_id: pid.clone(),
            schedule_type: "daily".into(),
            expression: "02:00".into(),
            timezone: "UTC".into(),
            enabled: true,
            next_run_at: Some("2026-10-09T02:00:00Z".into()),
            last_run_at: None,
            last_status: Some("idle".into()),
            last_error_code: None,
            created_at: "2026-10-08T12:00:00Z".into(),
            updated_at: "2026-10-08T12:00:00Z".into(),
        };
        db.create_schedule(&schedule_rec).unwrap();

        // 3. Run Backup
        let cancel = CancellationToken::new();
        let summary = backup_engine
            .execute_profile_backup(&profile, &cancel)
            .expect("backup in instance 1");
        snap_id_saved = summary.snapshot_id;

        // 4. Run Verification & persist history
        let v_engine = televault_backup::verification::VerificationEngine::new(
            Arc::clone(&db),
            Arc::clone(&mock_provider) as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            Arc::clone(&temp_manager),
        );
        let v_res = v_engine
            .verify_snapshot(
                &pid,
                &snap_id_saved,
                &VerificationOptions::metadata_only(),
                &cancel,
            )
            .expect("verify in instance 1");
        assert_eq!(v_res.status, VerificationStatus::Healthy);

        // Explicitly check DB integrity before closing
        assert_eq!(db.full_integrity_check().unwrap(), vec!["ok".to_string()]);
        assert!(db.foreign_key_check().unwrap().is_empty());

        // Process shuts down: DB, engines, temp manager dropped
    }

    // =========================================================================
    // RUNTIME INSTANCE 2: Reopen from disk, verify all entities survived cleanly
    // =========================================================================
    {
        let db2 = Database::open(&db_path).expect("open disk db instance 2 after restart");

        // 1. Verify Profile reloaded
        let prof = db2
            .get_profile(&pid)
            .unwrap()
            .expect("profile must exist after restart");
        assert_eq!(prof.name, "Restart Profile");
        assert_eq!(Path::new(&prof.source_path), source_dir);

        // 2. Verify Schedule reloaded
        let sched = db2
            .get_schedule(&sched_id)
            .unwrap()
            .expect("schedule must exist after restart");
        assert_eq!(sched.schedule_type, "daily");
        assert_eq!(sched.expression, "02:00");
        assert!(sched.enabled);
        assert_eq!(sched.next_run_at.as_deref(), Some("2026-10-09T02:00:00Z"));

        // 3. Verify Snapshot reloaded
        let snap = db2
            .get_snapshot(&snap_id_saved)
            .unwrap()
            .expect("snapshot must exist after restart");
        assert_eq!(snap.status, BackupStatus::Completed);

        let versions = db2.list_versions_by_snapshot(&snap_id_saved).unwrap();
        assert_eq!(versions.len(), 1);

        // 4. Verify Files & Manifests reloaded
        let files = db2.list_files_by_profile(Some(&pid)).unwrap();
        assert_eq!(files.len(), 1);
        let file_rec = &files[0];
        assert_eq!(file_rec.relative_path, "persistent_doc.txt");

        let manifest = db2
            .get_manifest_by_file_id(&file_rec.file_id)
            .unwrap()
            .expect("manifest must exist after restart");
        assert_eq!(manifest.chunks.len(), 1);
        assert!(manifest.chunks[0].stored_size > 0);
        assert_eq!(manifest.logical_file.file_name, "persistent_doc.txt");

        // 5. Verify Verification History reloaded
        let history = db2.list_verification_history(&pid, 10).unwrap();
        assert!(!history.is_empty(), "verification history must persist");
        assert_eq!(history[0].target_id, snap_id_saved.to_string());
        assert_eq!(history[0].status, "healthy");

        // 6. Verify SQLite integrity and foreign keys on disk
        let integrity_res = db2.full_integrity_check().unwrap();
        assert_eq!(
            integrity_res,
            vec!["ok".to_string()],
            "Integrity check must return ok after restart: {integrity_res:?}"
        );

        let fk_violations = db2.foreign_key_check().unwrap();
        assert!(
            fk_violations.is_empty(),
            "Foreign key check must return 0 violations after restart: {fk_violations:?}"
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_interrupted_backup_recovery_across_restart() {
    let (root, db_path) = setup_temp_env("interrupted_restart");
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let pid = ProfileId::new("prof-interrupted-01").unwrap();
    let snap_id = SnapshotId::new("snap-interrupted-01").unwrap();

    // Simulate crash where snapshot was in 'BackingUp' status
    {
        let db = Database::open(&db_path).expect("open disk db instance 1");
        let prof_rec = ProfileRecord {
            profile_id: pid.clone(),
            name: "Interrupted Profile".into(),
            description: None,
            source_path: source_dir.to_string_lossy().to_string(),
            enabled: true,
            created_at: "2026-10-08T12:00:00Z".into(),
            updated_at: "2026-10-08T12:00:00Z".into(),
        };
        db.create_profile(&prof_rec).unwrap();

        let snap_rec = SnapshotRecord {
            snapshot_id: snap_id.clone(),
            profile_id: pid.clone(),
            status: BackupStatus::BackingUp,
            metadata: Some("in-flight interrupted".into()),
            created_at: "2026-10-08T12:01:00Z".into(),
        };
        db.create_snapshot(&snap_rec).unwrap();

        // Dropped abruptly without marking failed or completed
    }

    // Reopen after restart
    {
        let db2 = Database::open(&db_path).expect("open disk db instance 2");

        let snap = db2
            .get_snapshot(&snap_id)
            .unwrap()
            .expect("snapshot exists");
        assert_eq!(snap.status, BackupStatus::BackingUp);

        // Mark failed cooperatively on startup recovery
        db2.update_snapshot_status_and_metadata(
            &snap_id,
            BackupStatus::Failed,
            Some("Interrupted by system shutdown or restart"),
        )
        .unwrap();

        let recovered_snap = db2
            .get_snapshot(&snap_id)
            .unwrap()
            .expect("snapshot exists");
        assert_eq!(recovered_snap.status, BackupStatus::Failed);

        // Must still pass all integrity and foreign key constraints
        assert_eq!(db2.full_integrity_check().unwrap(), vec!["ok".to_string()]);
        assert!(db2.foreign_key_check().unwrap().is_empty());
    }

    let _ = fs::remove_dir_all(&root);
}
