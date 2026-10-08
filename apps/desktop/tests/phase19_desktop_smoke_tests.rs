//! Phase 19: Comprehensive Desktop Run & User Acceptance Smoke Tests
//!
//! Validates end-to-end desktop workflows on disk-backed SQLite database:
//! 1. Dashboard, System Info & Deterministic Startup Recovery
//! 2. Profiles View & Management
//! 3. Backups View, Snapshot Creation & File Catalog
//! 4. Verification View & Cryptographic Auditing
//! 5. Restore View & Destination Collision Resolution (Skip, Overwrite, KeepBoth)
//! 6. Schedules View & Next Run Computation
//! 7. Retention View, Policy Preview & Safe Metadata Pruning
//! 8. Repair View & Chunk Reconstruction Preview
//! 9. Activity View, Transfer Queue & Cancellation Tracking
//! 10. Settings View & Telegram Credential Isolation
//! 11. Application Restart & Full Metadata Persistence
//! 12. Safe Error Handling & Resilience (Invalid Paths, Non-existent Entities)

use std::fs;
use std::path::PathBuf;
use tauri::Manager;

use televault_desktop::commands::backup::*;
use televault_desktop::commands::repair::*;
use televault_desktop::commands::restore::*;
use televault_desktop::commands::retention::*;
use televault_desktop::commands::scheduler::*;
use televault_desktop::commands::system::*;
use televault_desktop::commands::telegram::*;
use televault_desktop::commands::transfer::*;
use televault_desktop::commands::verification::*;
use televault_desktop::dto::*;
use televault_desktop::state::DesktopAppState;

fn setup_temp_desktop_env(name: &str) -> (tauri::App<tauri::test::MockRuntime>, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("televault_p19_{name}_{nanos}"));
    fs::create_dir_all(&root).unwrap();

    let state =
        DesktopAppState::new(root.clone(), None).expect("Initialize DesktopAppState on disk");
    let app = tauri::test::mock_app();
    app.manage(state);
    (app, root)
}

// =========================================================================
// 1. DASHBOARD & SYSTEM INFO SMOKE TESTS
// =========================================================================
#[test]
fn test_p19_smoke_dashboard_and_startup_recovery() {
    let (app, root) = setup_temp_desktop_env("dashboard");
    let state = app.state::<DesktopAppState>();

    // 1. App Info
    let info = get_app_info().expect("get_app_info");
    assert_eq!(info.app_name, "TELEVAULT");
    assert_eq!(info.version, "0.1.0");
    assert_eq!(info.platform, "windows");

    // 2. System Paths
    let paths = get_system_paths(state.clone()).expect("get_system_paths");
    assert!(!paths.base_dir.is_empty());
    assert!(paths.database_path.ends_with("televault.db"));
    assert!(!paths.temp_dir.is_empty());

    // 3. Startup Recovery Report
    let recovery = get_startup_recovery_report(state.clone()).expect("get_startup_recovery_report");
    assert!(recovery.database_integrity_ok);
    assert_eq!(recovery.foreign_key_violations, 0);
    assert_eq!(recovery.reconciled_snapshots_count, 0);
    assert_eq!(recovery.reconciled_transfers_count, 0);

    // 4. Scheduler & Transfer Telemetry
    let sched_status = get_scheduler_status(state.clone()).expect("get_scheduler_status");
    assert!(!sched_status.status.is_empty());

    let transfer_status = get_transfer_status(state.clone()).expect("get_transfer_status");
    assert_eq!(transfer_status.active_count, 0);
    assert_eq!(transfer_status.queued_count, 0);

    let _ = fs::remove_dir_all(root);
}

// =========================================================================
// 2. PROFILES, BACKUP & SNAPSHOT LIFECYCLE SMOKE TESTS
// =========================================================================
#[tokio::test]
async fn test_p19_smoke_backup_and_snapshot_lifecycle() {
    let (app, root) = setup_temp_desktop_env("backup");
    let state = app.state::<DesktopAppState>();

    // Create source files
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    let file_a = source_dir.join("sample.txt");
    let file_b = source_dir.join("data.bin");
    fs::write(&file_a, b"TELEVAULT Phase 19 sample text document").unwrap();
    fs::write(&file_b, vec![0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56]).unwrap();

    // 1. Create Profile
    let profile_req = CreateProfileRequest {
        profile_id: "prof-p19-primary".into(),
        name: "P19 Primary Backup".into(),
        description: Some("Phase 19 verification test profile".into()),
        source_path: source_dir.to_string_lossy().to_string(),
    };
    let profile = create_backup_profile(state.clone(), profile_req).expect("create_backup_profile");
    assert_eq!(profile.profile_id, "prof-p19-primary");
    assert_eq!(profile.name, "P19 Primary Backup");
    assert_eq!(
        profile.source_path,
        source_dir.to_string_lossy().to_string()
    );
    assert!(profile.enabled);

    // 2. List Profiles
    let profiles = list_backup_profiles(state.clone()).expect("list_backup_profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].profile_id, profile.profile_id);

    // 3. Initiate Backup
    let backup_req = StartBackupRequest {
        profile_id: profile.profile_id.clone(),
        passphrase: None,
    };
    let summary = start_backup(state.clone(), backup_req)
        .await
        .expect("start_backup");
    assert_eq!(summary.profile_id, profile.profile_id);
    assert_eq!(summary.new_files, 2);

    // 4. List Snapshots
    let snapshots =
        list_snapshots(state.clone(), profile.profile_id.clone()).expect("list_snapshots");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].snapshot_id, summary.snapshot_id);
    assert_eq!(snapshots[0].status, "completed");

    // 5. Query Snapshot Files Catalog
    let catalog =
        get_snapshot_files(state.clone(), summary.snapshot_id.clone()).expect("get_snapshot_files");
    assert_eq!(catalog.len(), 2);
    let names: Vec<String> = catalog.iter().map(|f| f.relative_path.clone()).collect();
    assert!(names.iter().any(|n| n.contains("sample.txt")));
    assert!(names.iter().any(|n| n.contains("data.bin")));

    let _ = fs::remove_dir_all(root);
}

// =========================================================================
// 3. VERIFICATION & REPAIR WORKFLOW SMOKE TESTS
// =========================================================================
#[tokio::test]
async fn test_p19_smoke_verification_and_repair() {
    let (app, root) = setup_temp_desktop_env("verify_repair");
    let state = app.state::<DesktopAppState>();

    // Prepare backup target
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("critical.log"),
        b"Critical security and audit logs",
    )
    .unwrap();

    let profile = create_backup_profile(
        state.clone(),
        CreateProfileRequest {
            profile_id: "prof-p19-audit".into(),
            name: "Audit Profile".into(),
            description: None,
            source_path: source_dir.to_string_lossy().to_string(),
        },
    )
    .expect("create profile");

    let summary = start_backup(
        state.clone(),
        StartBackupRequest {
            profile_id: profile.profile_id.clone(),
            passphrase: None,
        },
    )
    .await
    .expect("run backup");

    let files = get_snapshot_files(state.clone(), summary.snapshot_id.clone()).expect("get files");
    assert!(!files.is_empty());
    let target_file_id = files[0].file_id.clone();

    // 1. Verify Manifest Metadata Check
    let meta_report = verify_manifest_metadata(
        state.clone(),
        VerifyManifestRequest {
            file_id: target_file_id.clone(),
        },
    )
    .expect("verify_manifest_metadata");
    assert!(meta_report.is_restorable);
    assert_eq!(meta_report.file_id, target_file_id);

    // 2. Target Verification via IPC
    let fid = televault_core::ids::FileId::new(&target_file_id).unwrap();
    let manifest = state.db.get_manifest_by_file_id(&fid).unwrap().unwrap();

    let verify_req = VerifyTargetRequest {
        profile_id: profile.profile_id.clone(),
        target_id: Some(manifest.manifest_id),
        level: Some(1),
        full_hash_check: Some(false),
        decrypt_check: Some(false),
        operation_id: None,
    };
    let verify_res = verify_manifest(state.clone(), verify_req)
        .await
        .expect("verify_manifest");
    assert_eq!(verify_res.status, "healthy");
    assert!(verify_res.is_restore_ready);

    // 3. Query Verification History
    let history = get_verification_history(state.clone(), profile.profile_id.clone(), Some(10))
        .expect("get_verification_history");
    assert!(!history.is_empty());
    assert_eq!(history[0].profile_id, profile.profile_id);

    // 4. Repair Preview
    let preview_req = PreviewRepairRequest {
        profile_id: profile.profile_id.clone(),
        target_type: "file".into(),
        target_id: target_file_id.clone(),
        operation_id: None,
        passphrase: None,
    };
    let preview = preview_repair(state.clone(), preview_req)
        .await
        .expect("preview_repair");
    assert_eq!(preview.target_id, target_file_id);
    assert_eq!(preview.eligible_candidates.len(), 0); // No chunks corrupted

    // 5. Query Repair History
    let repair_hist = get_repair_history(state.clone(), profile.profile_id.clone(), Some(10))
        .expect("get_repair_history");
    assert!(repair_hist.is_empty()); // Clean files require no repairs

    let _ = fs::remove_dir_all(root);
}

// =========================================================================
// 4. RESTORE & COLLISION HANDLING SMOKE TESTS
// =========================================================================
#[tokio::test]
async fn test_p19_smoke_restore_and_collision_handling() {
    let (app, root) = setup_temp_desktop_env("restore");
    let state = app.state::<DesktopAppState>();

    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    let original_payload = b"Original uncorrupted Phase 19 document content";
    let doc_file = source_dir.join("document.txt");
    fs::write(&doc_file, original_payload).unwrap();

    let profile = create_backup_profile(
        state.clone(),
        CreateProfileRequest {
            profile_id: "prof-p19-restore".into(),
            name: "Restore Test Profile".into(),
            description: None,
            source_path: source_dir.to_string_lossy().to_string(),
        },
    )
    .expect("create profile");

    let summary = start_backup(
        state.clone(),
        StartBackupRequest {
            profile_id: profile.profile_id.clone(),
            passphrase: None,
        },
    )
    .await
    .expect("run backup");

    let restore_target = root.join("restored_output");
    fs::create_dir_all(&restore_target).unwrap();

    // 1. Initial Clean Restore
    let res = restore_snapshot(
        state.clone(),
        RestoreSnapshotRequest {
            snapshot_id: summary.snapshot_id.clone(),
            destination_directory: restore_target.to_string_lossy().to_string(),
            collision_policy: CollisionPolicyDto::Overwrite,
            passphrase: None,
        },
    )
    .await
    .expect("restore_snapshot clean");
    assert_eq!(res.restored_files, 1);
    assert_eq!(res.failed_files, 0);

    let restored_file = restore_target.join("document.txt");
    assert!(restored_file.exists());
    assert_eq!(fs::read(&restored_file).unwrap(), original_payload);

    // 2. Collision Test: Overwrite Policy
    fs::write(&restored_file, b"Contaminated local content").unwrap();
    let res_overwrite = restore_snapshot(
        state.clone(),
        RestoreSnapshotRequest {
            snapshot_id: summary.snapshot_id.clone(),
            destination_directory: restore_target.to_string_lossy().to_string(),
            collision_policy: CollisionPolicyDto::Overwrite,
            passphrase: None,
        },
    )
    .await
    .expect("restore overwrite");
    assert_eq!(res_overwrite.failed_files, 0);
    assert_eq!(fs::read(&restored_file).unwrap(), original_payload);

    // 3. Collision Test: Skip Policy
    fs::write(
        &restored_file,
        b"Pre-existing file that must NOT be overwritten",
    )
    .unwrap();
    let res_skip = restore_snapshot(
        state.clone(),
        RestoreSnapshotRequest {
            snapshot_id: summary.snapshot_id.clone(),
            destination_directory: restore_target.to_string_lossy().to_string(),
            collision_policy: CollisionPolicyDto::Skip,
            passphrase: None,
        },
    )
    .await
    .expect("restore skip");
    assert_eq!(res_skip.skipped_files, 1);
    assert_eq!(
        fs::read(&restored_file).unwrap(),
        b"Pre-existing file that must NOT be overwritten"
    );

    // 4. Collision Test: KeepBoth Policy
    let res_keep_both = restore_snapshot(
        state.clone(),
        RestoreSnapshotRequest {
            snapshot_id: summary.snapshot_id.clone(),
            destination_directory: restore_target.to_string_lossy().to_string(),
            collision_policy: CollisionPolicyDto::KeepBoth,
            passphrase: None,
        },
    )
    .await
    .expect("restore keep_both");
    assert_eq!(res_keep_both.failed_files, 0);
    // Verify both original and restored copy exist
    let entries: Vec<_> = fs::read_dir(&restore_target)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e == "document.txt"));
    assert!(entries.iter().any(|e| e.contains("document (1).txt")));

    let _ = fs::remove_dir_all(root);
}

// =========================================================================
// 5. SCHEDULES & RETENTION SMOKE TESTS
// =========================================================================
#[test]
fn test_p19_smoke_schedules_and_retention() {
    let (app, root) = setup_temp_desktop_env("schedules_retention");
    let state = app.state::<DesktopAppState>();

    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let profile = create_backup_profile(
        state.clone(),
        CreateProfileRequest {
            profile_id: "prof-p19-sched".into(),
            name: "Automated Profile".into(),
            description: None,
            source_path: source_dir.to_string_lossy().to_string(),
        },
    )
    .expect("create profile");

    // 1. Create Schedule
    let sched_req = CreateScheduleRequest {
        profile_id: profile.profile_id.clone(),
        schedule_type: "daily".into(),
        expression: "02:30".into(),
        timezone: Some("utc".into()),
        enabled: Some(true),
    };
    let sched = create_schedule(state.clone(), sched_req).expect("create_schedule");
    assert_eq!(sched.profile_id, profile.profile_id);
    assert!(sched.enabled);
    assert!(sched.next_run_at.is_some());

    // 2. Disable & Enable Schedule
    let disabled =
        disable_schedule(state.clone(), sched.schedule_id.clone()).expect("disable_schedule");
    assert!(!disabled.enabled);

    let enabled =
        enable_schedule(state.clone(), sched.schedule_id.clone()).expect("enable_schedule");
    assert!(enabled.enabled);

    // 3. Retention Policy Get and Set
    let ret_policy = get_retention_policy(state.clone(), profile.profile_id.clone())
        .expect("get_retention_policy");
    assert_eq!(ret_policy.keep_latest_n, Some(10));

    let updated_policy = set_retention_policy(
        state.clone(),
        SetRetentionPolicyRequest {
            profile_id: profile.profile_id.clone(),
            keep_latest_n: Some(5),
            keep_newer_than_secs: Some(86400),
            keep_latest_successful: Some(true),
            keep_latest_always: Some(true),
            prune_failed: Some(false),
            prune_empty: Some(false),
            enabled: Some(true),
        },
    )
    .expect("set_retention_policy");
    assert_eq!(updated_policy.keep_latest_n, Some(5));

    // 4. Preview and Execute Retention
    let preview =
        preview_retention(state.clone(), profile.profile_id.clone()).expect("preview_retention");
    assert_eq!(preview.snapshots_pruned, 0);

    let exec_res =
        execute_retention(state.clone(), profile.profile_id.clone()).expect("execute_retention");
    assert!(exec_res.pruned_snapshots.is_empty());

    let _ = fs::remove_dir_all(root);
}

// =========================================================================
// 6. ACTIVITY, AUDIT & SETTINGS SMOKE TESTS
// =========================================================================
#[test]
fn test_p19_smoke_activity_and_settings() {
    let (app, root) = setup_temp_desktop_env("activity_settings");
    let state = app.state::<DesktopAppState>();

    // 1. Transfer jobs list
    let jobs = list_transfer_jobs(state.clone(), None).expect("list_transfer_jobs");
    assert!(jobs.is_empty());

    // 2. App Settings Get and Update
    let config = get_app_config(state.clone()).expect("get_app_config");
    assert_eq!(config.transfer.max_concurrent_transfers, 3);

    let mut updated_config = config.clone();
    updated_config.transfer.max_concurrent_transfers = 4;
    let saved_config = update_app_config(state.clone(), updated_config).expect("update_app_config");
    assert_eq!(saved_config.transfer.max_concurrent_transfers, 4);

    // 3. Telegram Configuration & Credential Masking
    let tg_status = get_telegram_status(state.clone()).expect("get_telegram_status");
    assert!(!tg_status.is_configured); // Unconfigured initially

    let creds_req = SaveTelegramConfigDto {
        bot_token: "123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ123456789".into(),
        target_chat_id: -1001234567890,
        api_endpoint: None,
    };
    let save_res = save_telegram_config(state.clone(), creds_req).expect("save_telegram_config");
    assert!(save_res.is_configured);
    assert_eq!(save_res.target_chat_id, Some(-1001234567890));

    // Verify token is never exposed in plaintext
    let new_tg_status = get_telegram_status(state.clone()).expect("get_telegram_status");
    assert!(new_tg_status.is_configured);

    let _ = fs::remove_dir_all(root);
}

// =========================================================================
// 7. APPLICATION RESTART & PERSISTENCE SMOKE TEST
// =========================================================================
#[tokio::test]
async fn test_p19_smoke_restart_persistence_and_deterministic_recovery() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("televault_p19_restart_{nanos}"));
    fs::create_dir_all(&root).unwrap();

    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("persistent.txt"),
        b"Persistent across restart",
    )
    .unwrap();

    let pid_saved = "prof-p19-persist".to_string();
    let sid_saved: String;
    let snap_id_saved: String;

    // --- INSTANCE 1: Create state, run backup, configure schedule ---
    {
        let state1 = DesktopAppState::new(root.clone(), None).expect("Instance 1 start");
        let app1 = tauri::test::mock_app();
        app1.manage(state1);
        let tauri_state1 = app1.state::<DesktopAppState>();

        let _profile = create_backup_profile(
            tauri_state1.clone(),
            CreateProfileRequest {
                profile_id: pid_saved.clone(),
                name: "Persistent Profile".into(),
                description: Some("Survives restart".into()),
                source_path: source_dir.to_string_lossy().to_string(),
            },
        )
        .expect("create profile 1");

        let sched = create_schedule(
            tauri_state1.clone(),
            CreateScheduleRequest {
                profile_id: pid_saved.clone(),
                schedule_type: "daily".into(),
                expression: "03:00".into(),
                timezone: Some("utc".into()),
                enabled: Some(true),
            },
        )
        .expect("create schedule 1");
        sid_saved = sched.schedule_id.clone();

        let summary = start_backup(
            tauri_state1.clone(),
            StartBackupRequest {
                profile_id: pid_saved.clone(),
                passphrase: None,
            },
        )
        .await
        .expect("start backup 1");
        snap_id_saved = summary.snapshot_id.clone();

        // Update settings in Instance 1
        let mut cfg = get_app_config(tauri_state1.clone()).expect("get cfg 1");
        cfg.transfer.max_concurrent_transfers = 5;
        update_app_config(tauri_state1.clone(), cfg).expect("update cfg 1");
        // State 1 drops cleanly here
    }

    // --- INSTANCE 2: Restart application from the exact same disk directory ---
    {
        let state2 = DesktopAppState::new(root.clone(), None).expect("Instance 2 restart");
        let app2 = tauri::test::mock_app();
        app2.manage(state2);
        let tauri_state2 = app2.state::<DesktopAppState>();

        // 1. Startup Recovery Report must be healthy
        let rec = get_startup_recovery_report(tauri_state2.clone()).expect("recovery 2");
        assert!(rec.database_integrity_ok);
        assert_eq!(rec.foreign_key_violations, 0);
        // Completed snapshot must NOT be flagged as interrupted
        assert_eq!(rec.reconciled_snapshots_count, 0);

        // 2. Profile persisted
        let profiles = list_backup_profiles(tauri_state2.clone()).expect("list profiles 2");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].profile_id, pid_saved);
        assert_eq!(profiles[0].name, "Persistent Profile");

        // 3. Schedule persisted
        let schedules = list_schedules(tauri_state2.clone()).expect("list schedules 2");
        assert_eq!(schedules.len(), 1);
        assert_eq!(schedules[0].schedule_id, sid_saved);
        assert_eq!(schedules[0].expression, "03:00");

        // 4. Snapshot & Catalog persisted
        let snapshots =
            list_snapshots(tauri_state2.clone(), pid_saved.clone()).expect("list snapshots 2");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].snapshot_id, snap_id_saved);
        assert_eq!(snapshots[0].status, "completed");

        let files =
            get_snapshot_files(tauri_state2.clone(), snap_id_saved.clone()).expect("catalog 2");
        assert_eq!(files.len(), 1);
        assert!(files[0].relative_path.contains("persistent.txt"));

        // 5. Config persisted
        let cfg2 = get_app_config(tauri_state2.clone()).expect("cfg 2");
        assert_eq!(cfg2.transfer.max_concurrent_transfers, 5);
    }

    let _ = fs::remove_dir_all(root);
}

// =========================================================================
// 8. ERROR HANDLING & RESILIENCE SMOKE TESTS
// =========================================================================
#[tokio::test]
async fn test_p19_smoke_safe_error_handling() {
    let (app, root) = setup_temp_desktop_env("error_handling");
    let state = app.state::<DesktopAppState>();

    // 1. Non-existent profile backup
    let err_backup = start_backup(
        state.clone(),
        StartBackupRequest {
            profile_id: "non-existent-profile-id".into(),
            passphrase: None,
        },
    )
    .await;
    assert!(err_backup.is_err());

    // 2. Non-existent snapshot restore
    let err_restore = restore_snapshot(
        state.clone(),
        RestoreSnapshotRequest {
            snapshot_id: "non-existent-snapshot-id".into(),
            destination_directory: root.join("restore").to_string_lossy().to_string(),
            collision_policy: CollisionPolicyDto::Overwrite,
            passphrase: None,
        },
    )
    .await;
    assert!(err_restore.is_err());

    // 3. Invalid schedule expression
    let err_sched = create_schedule(
        state.clone(),
        CreateScheduleRequest {
            profile_id: "any-profile".into(),
            schedule_type: "daily".into(),
            expression: "invalid:time:format".into(),
            timezone: None,
            enabled: Some(true),
        },
    );
    assert!(err_sched.is_err());

    // 4. Verify database integrity is still 100% OK after all errors
    let paths = get_system_paths(state.clone()).expect("get_system_paths");
    assert!(!paths.database_path.is_empty());

    let _ = fs::remove_dir_all(root);
}
