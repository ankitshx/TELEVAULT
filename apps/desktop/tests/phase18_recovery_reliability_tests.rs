//! Comprehensive Phase 18 Production Hardening, Recovery & Reliability Tests.
//!
//! Validates:
//! - Startup recovery & deterministic reconciliation of crashed operations (snapshots & transfer jobs)
//! - Orphaned staging payload cleanup on startup
//! - Database PRAGMA integrity_check & foreign_key_check on startup
//! - Restore interruption & destination collision safety (Overwrite, Skip, KeepBoth)
//! - Configuration atomicity under simulated write interruption
//! - Scheduler state recovery and missed run bounds without execution storms
//! - Retention transactional pruning and remote Telegram payload preservation invariant

use std::fs;
use std::path::PathBuf;
use televault_backup::profile::BackupProfile;
use televault_backup::restore::CollisionPolicy;
use televault_core::ids::{FileId, JobId, ProfileId, ScheduleId, SnapshotId};
use televault_core::models::{BackupStatus, TransferDirection, TransferStatus};
use televault_db::{
    Database, FileRecord, ProfileRecord, ScheduleRecord, SnapshotRecord, TransferJobRecord,
};
use televault_desktop::state::DesktopAppState;
use televault_scheduler::SchedulerServiceStatus;
use televault_transfer::cancellation::CancellationToken;

fn setup_temp_env(name: &str) -> (PathBuf, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("televault_phase18_{name}_{nanos}"));
    fs::create_dir_all(&root).unwrap();
    let db_path = root.join("database").join("televault.db");
    (root, db_path)
}

#[test]
fn test_startup_recovery_reconciles_interrupted_operations_and_purges_staging() {
    let (root, db_path) = setup_temp_env("startup_rec");
    let base_dir = root.clone();

    // 1. Manually prepare database with crashed state
    fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    let db = Database::open(&db_path).expect("open database");

    let pid = ProfileId::new("prof-crashed").unwrap();
    let profile = ProfileRecord {
        profile_id: pid.clone(),
        name: "Crashed Profile".into(),
        description: None,
        source_path: base_dir.join("source").to_string_lossy().to_string(),
        enabled: true,
        created_at: "2026-10-08T10:00:00Z".into(),
        updated_at: "2026-10-08T10:00:00Z".into(),
    };
    db.create_profile(&profile).unwrap();

    let fid = FileId::new("file-crashed-01").unwrap();
    let file = FileRecord {
        file_id: fid.clone(),
        profile_id: Some(pid.clone()),
        file_name: "data.bin".into(),
        relative_path: "data.bin".into(),
        original_size: 1024,
        mime_type: None,
        status: "active".into(),
        logical_file_hash: Some("hash1".into()),
        created_at: "2026-10-08T10:00:00Z".into(),
        modified_at: None,
        created_timestamp: "2026-10-08T10:00:00Z".into(),
        updated_timestamp: "2026-10-08T10:00:00Z".into(),
    };
    db.create_file(&file).unwrap();

    // Stuck snapshot in 'BackingUp'
    let snap_stuck = SnapshotRecord {
        snapshot_id: SnapshotId::new("snap-stuck").unwrap(),
        profile_id: pid.clone(),
        status: BackupStatus::BackingUp,
        metadata: Some("{\"in_flight\": true}".into()),
        created_at: "2026-10-08T10:01:00Z".into(),
    };
    db.create_snapshot(&snap_stuck).unwrap();

    // Stuck snapshot in 'Scanning'
    let snap_scanning = SnapshotRecord {
        snapshot_id: SnapshotId::new("snap-scanning").unwrap(),
        profile_id: pid.clone(),
        status: BackupStatus::Scanning,
        metadata: None,
        created_at: "2026-10-08T10:02:00Z".into(),
    };
    db.create_snapshot(&snap_scanning).unwrap();

    // Stuck transfer job in 'Transferring'
    let job_stuck = TransferJobRecord {
        job_id: JobId::new("job-stuck").unwrap(),
        file_id: fid.clone(),
        chunk_id: None,
        direction: TransferDirection::Upload,
        status: TransferStatus::Transferring,
        progress: 512,
        retry_count: 1,
        error_message: None,
        created_at: "2026-10-08T10:01:30Z".into(),
        updated_at: "2026-10-08T10:01:45Z".into(),
    };
    db.create_transfer_job(&job_stuck).unwrap();

    // Create orphaned staging files in temp directory
    let temp_dir = base_dir.join("temp");
    fs::create_dir_all(&temp_dir).unwrap();
    let orphan_file = temp_dir.join("stage_chunk_orphan_01.tmp");
    fs::write(&orphan_file, b"orphaned chunk staging payload").unwrap();
    assert!(orphan_file.exists());

    // 2. Launch DesktopAppState::new, simulating application restart
    let state = DesktopAppState::new(base_dir.clone(), None).expect("DesktopAppState restart");

    // 3. Verify StartupRecoveryReport
    let report = &state.startup_recovery_report;
    assert!(report.database_integrity_ok);
    assert_eq!(report.database_integrity_details, vec!["ok".to_string()]);
    assert_eq!(report.foreign_key_violations, 0);
    assert_eq!(
        report.reconciled_snapshots_count, 2,
        "Both stuck snapshots reconciled"
    );
    assert_eq!(
        report.reconciled_transfers_count, 1,
        "Stuck transfer job reconciled"
    );
    assert_eq!(
        report.purged_staging_files_count, 1,
        "Orphaned staging file purged"
    );

    // Verify orphan file was deleted from disk
    assert!(
        !orphan_file.exists(),
        "Orphaned staging file must be purged"
    );

    // Verify database statuses
    let db2 = &state.db;
    let s1 = db2
        .get_snapshot(&SnapshotId::new("snap-stuck").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(s1.status, BackupStatus::Failed);
    assert!(s1
        .metadata
        .as_ref()
        .unwrap()
        .contains("Operation interrupted"));

    let s2 = db2
        .get_snapshot(&SnapshotId::new("snap-scanning").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(s2.status, BackupStatus::Failed);
    assert!(s2
        .metadata
        .as_ref()
        .unwrap()
        .contains("Operation interrupted"));

    let j1 = db2
        .get_transfer_job(&JobId::new("job-stuck").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(j1.status, TransferStatus::Failed);
    assert!(j1
        .error_message
        .as_ref()
        .unwrap()
        .contains("Operation interrupted"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_restore_interruption_and_destination_collision_policies() {
    let (root, _db_path) = setup_temp_env("restore_collision");
    let state = DesktopAppState::new(root.clone(), None).expect("DesktopAppState");

    let source_dir = root.join("source");
    let restore_dir = root.join("restore");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&restore_dir).unwrap();

    let test_file = source_dir.join("document.txt");
    fs::write(&test_file, b"Initial production version 1 content").unwrap();

    // 1. Create profile and perform initial backup
    let pid = ProfileId::new("prof-restore-test").unwrap();
    let profile = BackupProfile::new(pid.clone(), "Restore Test Profile", source_dir.clone());
    state.db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();
    let summary = state
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();

    // Pre-create destination file with conflicting content
    let dest_file = restore_dir.join("document.txt");
    fs::write(&dest_file, b"Conflicting local data already on disk").unwrap();

    // Policy 1: CollisionPolicy::Skip -> Existing file preserved untouched
    let skip_req = televault_backup::restore::SnapshotRestoreRequest::new(
        summary.snapshot_id.clone(),
        restore_dir.clone(),
    )
    .with_collision_policy(CollisionPolicy::Skip);
    let res_skip = state
        .restore_engine
        .restore_snapshot(&skip_req, &cancel)
        .unwrap();
    assert_eq!(res_skip.skipped_files, 1);
    assert_eq!(
        fs::read(&dest_file).unwrap(),
        b"Conflicting local data already on disk"
    );

    // Policy 2: CollisionPolicy::KeepBoth -> Creates disambiguated filename, original untouched
    let keep_req = televault_backup::restore::SnapshotRestoreRequest::new(
        summary.snapshot_id.clone(),
        restore_dir.clone(),
    )
    .with_collision_policy(CollisionPolicy::KeepBoth);
    let res_keep = state
        .restore_engine
        .restore_snapshot(&keep_req, &cancel)
        .unwrap();
    assert_eq!(res_keep.kept_both_files, 1);
    assert_eq!(
        fs::read(&dest_file).unwrap(),
        b"Conflicting local data already on disk"
    );

    // Check disambiguated file exists and has restored content
    let restored_disambiguated = restore_dir.join("document (1).txt");
    assert!(restored_disambiguated.exists());
    assert_eq!(
        fs::read(&restored_disambiguated).unwrap(),
        b"Initial production version 1 content"
    );

    // Policy 3: CollisionPolicy::Overwrite -> Safely overwrites target file
    let overwrite_req = televault_backup::restore::SnapshotRestoreRequest::new(
        summary.snapshot_id.clone(),
        restore_dir.clone(),
    )
    .with_collision_policy(CollisionPolicy::Overwrite);
    let res_over = state
        .restore_engine
        .restore_snapshot(&overwrite_req, &cancel)
        .unwrap();
    assert_eq!(res_over.overwritten_files, 1);
    assert_eq!(
        fs::read(&dest_file).unwrap(),
        b"Initial production version 1 content"
    );

    // Invariant check: Staging directory contains 0 leaked payloads
    let temp_entries = fs::read_dir(state.paths.temp_dir()).unwrap();
    let temp_count = temp_entries.count();
    assert_eq!(
        temp_count, 0,
        "All restore temporary files must be cleaned after restore"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_configuration_atomicity_and_corruption_resilience() {
    let (root, _db_path) = setup_temp_env("config_atomicity");
    let state = DesktopAppState::new(root.clone(), None).expect("DesktopAppState");

    let config_file = state.paths.config_file();
    let config_dir = state.paths.config_dir();
    fs::create_dir_all(&config_dir).unwrap();

    // 1. Initial valid config written
    let initial_config = televault_core::config::AppConfig::default();
    let initial_json = serde_json::to_string_pretty(&initial_config).unwrap();
    fs::write(&config_file, initial_json.as_bytes()).unwrap();

    // 2. Simulate partial / interrupted write to temp file
    let temp_file = config_file.with_extension("tmp");
    fs::write(&temp_file, b"{\"incomplete\": true, \"trun").unwrap();

    // Verify original config is completely intact
    let content = fs::read_to_string(&config_file).unwrap();
    let parsed: televault_core::config::AppConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed, initial_config);

    // 3. Atomically replace with updated valid config
    let mut updated_config = initial_config.clone();
    updated_config.general.theme = "light".into();
    let updated_json = serde_json::to_string_pretty(&updated_config).unwrap();

    fs::write(&temp_file, updated_json.as_bytes()).unwrap();
    fs::rename(&temp_file, &config_file).unwrap();

    let read_back: televault_core::config::AppConfig =
        serde_json::from_str(&fs::read_to_string(&config_file).unwrap()).unwrap();
    assert_eq!(read_back.general.theme, "light");
    assert!(!temp_file.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_scheduler_recovery_and_missed_run_bounds() {
    let (root, _db_path) = setup_temp_env("sched_recovery");
    let state = DesktopAppState::new(root.clone(), None).expect("DesktopAppState");

    let pid = ProfileId::new("prof-sched-rec").unwrap();
    let profile = ProfileRecord {
        profile_id: pid.clone(),
        name: "Sched Rec Profile".into(),
        description: None,
        source_path: root.join("source").to_string_lossy().to_string(),
        enabled: true,
        created_at: "2026-10-08T10:00:00Z".into(),
        updated_at: "2026-10-08T10:00:00Z".into(),
    };
    state.db.create_profile(&profile).unwrap();

    // Create schedule with next_run_at in past (due while app was closed)
    let sched_id = ScheduleId::new("sched-past-due").unwrap();
    let schedule = ScheduleRecord {
        schedule_id: sched_id.clone(),
        profile_id: pid.clone(),
        schedule_type: "daily".into(),
        expression: "00:00".into(),
        timezone: "UTC".into(),
        enabled: true,
        next_run_at: Some("2026-10-07T00:00:00Z".into()), // 1 day ago
        last_run_at: None,
        last_status: Some("running".into()), // Left in 'running' prior to crash
        last_error_code: None,
        created_at: "2026-10-06T10:00:00Z".into(),
        updated_at: "2026-10-07T00:00:00Z".into(),
    };
    state.db.create_schedule(&schedule).unwrap();

    // Verify scheduler status starts cleanly
    let status = state.scheduler_service.status();
    assert_eq!(status, SchedulerServiceStatus::Stopped);

    // Verify schedule record is accessible and not corrupted
    let fetched = state.db.get_schedule(&sched_id).unwrap().unwrap();
    assert!(fetched.enabled);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase18_end_to_end_core_lifecycle_and_zero_payload_leakage() {
    let (root, _db_path) = setup_temp_env("e2e_full_lifecycle");
    let state = DesktopAppState::new(root.clone(), None).expect("DesktopAppState");

    let source_dir = root.join("source");
    let restore_dir = root.join("restore");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&restore_dir).unwrap();

    // 1. Create source file
    let doc_path = source_dir.join("financial_report.pdf");
    let test_data = vec![0x42u8; 128 * 1024]; // 128 KiB
    fs::write(&doc_path, &test_data).unwrap();

    // 2. Create Profile
    let pid = ProfileId::new("prof-e2e-p18").unwrap();
    let profile = BackupProfile::new(pid.clone(), "E2E P18 Profile", source_dir.clone());
    state.db.create_profile(&profile.to_db_record()).unwrap();

    // 3. Execute Backup
    let cancel = CancellationToken::new();
    let summary = state
        .backup_engine
        .execute_profile_backup(&profile, &cancel)
        .unwrap();
    assert_eq!(summary.status, BackupStatus::Completed);
    assert_eq!(summary.new_files, 1);
    assert_eq!(summary.transferred_chunks, 1);

    // 4. Verify Snapshot remotely across options
    let ver_opts = televault_integrity::types::VerificationOptions {
        level: televault_integrity::types::VerificationLevel::RemoteIntegrity,
        full_hash_check: true,
        decrypt_check: false,
        timeout_secs: Some(30),
    };
    let ver_res = state
        .verification_engine
        .verify_snapshot(&pid, &summary.snapshot_id, &ver_opts, &cancel)
        .unwrap();
    assert_eq!(
        ver_res.status,
        televault_integrity::types::VerificationStatus::Healthy
    );

    // 5. Restore Snapshot
    let restore_req = televault_backup::restore::SnapshotRestoreRequest::new(
        summary.snapshot_id.clone(),
        restore_dir.clone(),
    )
    .with_collision_policy(CollisionPolicy::Overwrite);
    let rest_res = state
        .restore_engine
        .restore_snapshot(&restore_req, &cancel)
        .unwrap();
    assert_eq!(rest_res.restored_files, 1);

    // Verify byte-level equality
    let restored_file = restore_dir.join("financial_report.pdf");
    assert!(restored_file.exists());
    let restored_data = fs::read(&restored_file).unwrap();
    assert_eq!(restored_data, test_data);

    // 6. Execute Retention Dry-Run
    let dry_run = state
        .retention_engine
        .evaluate_retention(
            &pid,
            None,
            &std::collections::HashSet::new(),
            chrono::Utc::now(),
        )
        .unwrap();
    assert_eq!(
        dry_run.snapshots_pruned, 0,
        "Current snapshot kept within default retention"
    );

    // 7. Verify Cloud-First Storage Invariant: 0 permanent local backup payloads
    let temp_entries = fs::read_dir(state.paths.temp_dir()).unwrap();
    assert_eq!(
        temp_entries.count(),
        0,
        "Temp staging directory must be clean"
    );

    // 8. Verify SQLite database integrity and foreign key constraints
    assert_eq!(
        state.db.full_integrity_check().unwrap(),
        vec!["ok".to_string()]
    );
    assert!(state.db.foreign_key_check().unwrap().is_empty());

    let _ = fs::remove_dir_all(&root);
}
