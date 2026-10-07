//! Comprehensive IPC integration and command suite tests for TELEVAULT Phase 10.

use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use televault_desktop::commands::backup::*;
use televault_desktop::commands::checker::*;
use televault_desktop::commands::restore::*;
use televault_desktop::commands::system::*;
use televault_desktop::commands::transfer::*;
use televault_desktop::dto::*;
use televault_desktop::state::DesktopAppState;

fn setup_test_state() -> (tauri::App<tauri::test::MockRuntime>, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("televault_ipc_test_{nanos}"));
    fs::create_dir_all(&temp_dir).unwrap();

    let state = DesktopAppState::new_in_memory();
    let app = tauri::test::mock_app();
    app.manage(state);
    (app, temp_dir)
}

#[test]
fn test_ipc_builder_initialization() {
    let builder = televault_desktop::builder::create_ipc_builder();
    // Builder initialized without panic
    let _ = builder;
}

#[test]
fn test_system_info_and_paths() {
    let (app, temp_dir) = setup_test_state();
    let tauri_state = app.state::<DesktopAppState>();

    let info = get_app_info().expect("get_app_info");
    assert_eq!(info.app_name, "TELEVAULT");
    assert!(!info.version.is_empty());
    assert_eq!(info.platform, "windows");

    let paths = get_system_paths(tauri_state).expect("get_system_paths");
    assert!(!paths.base_dir.is_empty());
    assert!(!paths.database_path.is_empty());
    assert!(!paths.temp_dir.is_empty());

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_backup_profile_lifecycle() {
    let (app, temp_dir) = setup_test_state();
    let tauri_state = app.state::<DesktopAppState>();

    let src_dir = temp_dir.join("work_docs");
    fs::create_dir_all(&src_dir).unwrap();

    // 1. Create profile
    let create_req = CreateProfileRequest {
        profile_id: "prof-docs-1".into(),
        name: "Work Documents".into(),
        description: Some("My primary files".into()),
        source_path: src_dir.to_string_lossy().to_string(),
    };
    let created = create_backup_profile(tauri_state.clone(), create_req).expect("create profile");
    assert_eq!(created.profile_id, "prof-docs-1");
    assert_eq!(created.name, "Work Documents");
    assert!(created.enabled);

    // 2. List profiles
    let list = list_backup_profiles(tauri_state.clone()).expect("list profiles");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].profile_id, "prof-docs-1");

    // 3. Get profile
    let fetched =
        get_backup_profile(tauri_state.clone(), "prof-docs-1".into()).expect("get profile");
    assert_eq!(fetched.profile_id, "prof-docs-1");
    assert_eq!(fetched.description, Some("My primary files".into()));

    // 4. Update profile
    let update_req = UpdateProfileRequest {
        profile_id: "prof-docs-1".into(),
        name: Some("Updated Docs".into()),
        description: Some("Updated description".into()),
        source_path: None,
        enabled: Some(false),
    };
    let updated = update_backup_profile(tauri_state.clone(), update_req).expect("update profile");
    assert_eq!(updated.name, "Updated Docs");
    assert!(!updated.enabled);

    // 5. Delete profile
    let deleted =
        delete_backup_profile(tauri_state.clone(), "prof-docs-1".into()).expect("delete profile");
    assert!(deleted);

    let list_after = list_backup_profiles(tauri_state).expect("list profiles after delete");
    assert!(list_after.is_empty());

    let _ = fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_backup_execution_and_snapshot_query() {
    let (app, temp_dir) = setup_test_state();
    let tauri_state = app.state::<DesktopAppState>();

    let src_dir = temp_dir.join("src_files");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("notes.txt"), "Important project notes").unwrap();

    let create_req = CreateProfileRequest {
        profile_id: "prof-test".into(),
        name: "Test Profile".into(),
        description: None,
        source_path: src_dir.to_string_lossy().to_string(),
    };
    let _ = create_backup_profile(tauri_state.clone(), create_req).expect("create profile");

    // Start backup
    let start_req = StartBackupRequest {
        profile_id: "prof-test".into(),
        passphrase: None,
    };
    let summary = start_backup(tauri_state.clone(), start_req)
        .await
        .expect("start_backup");

    assert_eq!(summary.profile_id, "prof-test");
    assert_eq!(summary.new_files, 1);
    assert_eq!(summary.transferred_chunks, 1);

    // List snapshots
    let snapshots =
        list_snapshots(tauri_state.clone(), "prof-test".into()).expect("list snapshots");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].profile_id, "prof-test");

    // Get snapshot files
    let files =
        get_snapshot_files(tauri_state, snapshots[0].snapshot_id.clone()).expect("snapshot files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].relative_path, "notes.txt");

    let _ = fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_restore_file_and_verification() {
    let (app, temp_dir) = setup_test_state();
    let tauri_state = app.state::<DesktopAppState>();

    let src_dir = temp_dir.join("src_restore");
    fs::create_dir_all(&src_dir).unwrap();
    let file_content = b"TeleVault Restore Engine Verification Payload 2026";
    fs::write(src_dir.join("data.bin"), file_content).unwrap();

    let create_req = CreateProfileRequest {
        profile_id: "prof-restore".into(),
        name: "Restore Profile".into(),
        description: None,
        source_path: src_dir.to_string_lossy().to_string(),
    };
    let _ = create_backup_profile(tauri_state.clone(), create_req).expect("create profile");

    let start_req = StartBackupRequest {
        profile_id: "prof-restore".into(),
        passphrase: None,
    };
    let summary = start_backup(tauri_state.clone(), start_req)
        .await
        .expect("start_backup");

    let files =
        get_snapshot_files(tauri_state.clone(), summary.snapshot_id).expect("snapshot files");
    let target_file_id = files[0].file_id.clone();

    // Verify metadata fast check
    let meta_report = verify_manifest_metadata(
        tauri_state.clone(),
        VerifyManifestRequest {
            file_id: target_file_id.clone(),
        },
    )
    .expect("verify metadata");
    assert!(meta_report.is_restorable);

    // Full restore verification (trial in staging)
    let full_report = verify_full_restore(
        tauri_state.clone(),
        RestoreFileRequest {
            file_id: target_file_id.clone(),
            destination_path: "".into(),
            collision_policy: CollisionPolicyDto::Overwrite,
            passphrase: None,
        },
    )
    .await
    .expect("verify full restore");
    assert!(full_report.verified);
    assert!(full_report.sha256_match);

    // Restore to disk
    let dest_file = temp_dir.join("restored").join("restored_data.bin");
    let restore_res = restore_file(
        tauri_state,
        RestoreFileRequest {
            file_id: target_file_id,
            destination_path: dest_file.to_string_lossy().to_string(),
            collision_policy: CollisionPolicyDto::Overwrite,
            passphrase: None,
        },
    )
    .await
    .expect("restore_file");

    assert_eq!(restore_res.bytes_restored, file_content.len() as u64);
    assert_eq!(restore_res.outcome, FileRestoreOutcomeDto::Restored);

    let restored_bytes = fs::read(&dest_file).expect("read restored file");
    assert_eq!(restored_bytes, file_content);

    let _ = fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_checker_commands() {
    let (app, temp_dir) = setup_test_state();
    let tauri_state = app.state::<DesktopAppState>();

    let src_dir = temp_dir.join("checker_src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("doc.txt"), "Version 1 content").unwrap();

    let create_req = CreateProfileRequest {
        profile_id: "prof-check".into(),
        name: "Check Profile".into(),
        description: None,
        source_path: src_dir.to_string_lossy().to_string(),
    };
    let _ = create_backup_profile(tauri_state.clone(), create_req).expect("create profile");

    // Before backup: check status
    let status_before = check_file_status(
        tauri_state.clone(),
        CheckFileStatusRequest {
            profile_id: "prof-check".into(),
            relative_path: "doc.txt".into(),
        },
    )
    .expect("check_file_status");
    assert!(!status_before.is_backed_up);

    // Run backup
    let _ = start_backup(
        tauri_state.clone(),
        StartBackupRequest {
            profile_id: "prof-check".into(),
            passphrase: None,
        },
    )
    .await
    .expect("backup");

    // After backup: check status
    let status_after = check_file_status(
        tauri_state.clone(),
        CheckFileStatusRequest {
            profile_id: "prof-check".into(),
            relative_path: "doc.txt".into(),
        },
    )
    .expect("check_file_status");
    assert!(status_after.is_backed_up);
    assert!(status_after.file_id.is_some());

    // Incremental check: unchanged
    let needed = is_incremental_backup_needed(tauri_state.clone(), "prof-check".into())
        .await
        .expect("incremental needed");
    assert!(!needed);

    // Modify file
    fs::write(src_dir.join("doc.txt"), "Version 2 modified content").unwrap();
    let needed_after_mod = is_incremental_backup_needed(tauri_state, "prof-check".into())
        .await
        .expect("incremental needed");
    assert!(needed_after_mod);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_transfer_status_and_cancellation() {
    let (app, temp_dir) = setup_test_state();
    let tauri_state = app.state::<DesktopAppState>();

    let status = get_transfer_status(tauri_state.clone()).expect("get_transfer_status");
    assert_eq!(status.active_count, 0);
    assert_eq!(status.queued_count, 0);

    // Register active cancellation token
    let token = tauri_state.register_cancellation("op-test-cancel");
    assert!(!token.is_cancelled());

    let cancelled = cancel_operation(
        tauri_state,
        CancelOperationRequest {
            operation_id: "op-test-cancel".into(),
        },
    )
    .expect("cancel operation");

    assert!(cancelled);
    assert!(token.is_cancelled());

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_error_mapping_and_validation() {
    let (app, temp_dir) = setup_test_state();
    let tauri_state = app.state::<DesktopAppState>();

    // Non-existent profile query
    let err = get_backup_profile(tauri_state.clone(), "non-existent-profile".into()).unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");

    // Invalid profile ID (empty)
    let invalid_id_err = get_backup_profile(tauri_state.clone(), "".into()).unwrap_err();
    assert_eq!(invalid_id_err.code, "VALIDATION_ERROR");

    // Invalid source path
    let bad_path_req = CreateProfileRequest {
        profile_id: "prof-bad".into(),
        name: "Bad Path".into(),
        description: None,
        source_path: "C:\\NonExistentPath\\12345\\impossible".into(),
    };
    let err_path = create_backup_profile(tauri_state, bad_path_req).unwrap_err();
    assert_eq!(err_path.code, "VALIDATION_ERROR");

    let _ = fs::remove_dir_all(temp_dir);
}
