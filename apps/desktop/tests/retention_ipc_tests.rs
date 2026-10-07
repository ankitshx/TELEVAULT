//! Comprehensive IPC integration tests for retention policy engine commands.

use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use televault_core::ids::{ProfileId, SnapshotId};
use televault_core::models::BackupStatus;
use televault_db::models::SnapshotRecord;
use televault_desktop::commands::backup::*;
use televault_desktop::commands::retention::*;
use televault_desktop::dto::*;
use televault_desktop::state::DesktopAppState;

fn setup_test_state() -> (tauri::App<tauri::test::MockRuntime>, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("televault_retention_ipc_test_{nanos}"));
    fs::create_dir_all(&temp_dir).unwrap();

    let state = DesktopAppState::new_in_memory();
    let app = tauri::test::mock_app();
    app.manage(state);
    (app, temp_dir)
}

fn create_test_profile(
    state: tauri::State<'_, DesktopAppState>,
    temp_dir: &std::path::Path,
    id: &str,
) -> BackupProfileDto {
    let source = temp_dir.join(format!("src_{id}"));
    fs::create_dir_all(&source).unwrap();

    create_backup_profile(
        state,
        CreateProfileRequest {
            profile_id: id.to_string(),
            name: format!("Profile {id}"),
            description: Some("Test profile for retention IPC".to_string()),
            source_path: source.to_string_lossy().to_string(),
        },
    )
    .expect("create profile")
}

#[test]
fn test_retention_policy_defaults_and_get() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-ret-01");

    // Fetch default policy (when none explicitly saved)
    let policy = get_retention_policy(state.clone(), profile.profile_id.clone())
        .expect("get_retention_policy");

    assert_eq!(policy.profile_id, profile.profile_id);
    assert_eq!(policy.keep_latest_n, Some(10));
    assert!(policy.keep_latest_successful);
    assert!(policy.keep_latest_always);
    assert!(!policy.prune_failed);
    assert!(!policy.prune_empty);
    assert!(policy.enabled);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_retention_policy_set_and_persist() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-ret-02");

    // Configure new retention policy
    let updated = set_retention_policy(
        state.clone(),
        SetRetentionPolicyRequest {
            profile_id: profile.profile_id.clone(),
            keep_latest_n: Some(3),
            keep_newer_than_secs: Some(86400),
            keep_latest_successful: Some(true),
            keep_latest_always: Some(false),
            prune_failed: Some(true),
            prune_empty: Some(true),
            enabled: Some(true),
        },
    )
    .expect("set_retention_policy");

    assert_eq!(updated.keep_latest_n, Some(3));
    assert_eq!(updated.keep_newer_than_secs, Some(86400));
    assert!(!updated.keep_latest_always);
    assert!(updated.prune_failed);
    assert!(updated.prune_empty);

    // Verify persistence across subsequent read
    let fetched = get_retention_policy(state.clone(), profile.profile_id.clone())
        .expect("fetch updated policy");

    assert_eq!(fetched.keep_latest_n, Some(3));
    assert_eq!(fetched.keep_newer_than_secs, Some(86400));
    assert!(!fetched.keep_latest_always);
    assert!(fetched.prune_failed);
    assert!(fetched.prune_empty);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_retention_preview_dry_run_ipc() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-ret-03");
    let pid = ProfileId::new(&profile.profile_id).unwrap();

    // Create 4 snapshots: snap-1, snap-2, snap-3, snap-4
    for i in 1..=4 {
        let sid = SnapshotId::new(format!("snap-03-{i}")).unwrap();
        state
            .db
            .create_snapshot(&SnapshotRecord {
                snapshot_id: sid,
                profile_id: pid.clone(),
                status: BackupStatus::Completed,
                metadata: None,
                created_at: format!("2026-10-0{i}T12:00:00Z"),
            })
            .unwrap();
    }

    // Set retention policy to keep latest 2
    set_retention_policy(
        state.clone(),
        SetRetentionPolicyRequest {
            profile_id: profile.profile_id.clone(),
            keep_latest_n: Some(2),
            keep_newer_than_secs: None,
            keep_latest_successful: Some(true),
            keep_latest_always: Some(true),
            prune_failed: Some(false),
            prune_empty: Some(false),
            enabled: Some(true),
        },
    )
    .expect("set policy");

    // Preview retention
    let preview =
        preview_retention(state.clone(), profile.profile_id.clone()).expect("preview_retention");

    assert_eq!(preview.profile_id, profile.profile_id);
    assert_eq!(preview.snapshots_evaluated, 4);
    assert_eq!(preview.snapshots_kept, 2);
    assert_eq!(preview.snapshots_pruned, 2);
    assert_eq!(preview.decisions.len(), 4);

    // Verify dry-run made zero changes to database
    let all_snapshots = state.db.list_snapshots_by_profile(&pid).unwrap();
    assert_eq!(all_snapshots.len(), 4);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_retention_execute_pruning_and_history_ipc() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-ret-04");
    let pid = ProfileId::new(&profile.profile_id).unwrap();

    // Create 5 snapshots
    for i in 1..=5 {
        let sid = SnapshotId::new(format!("snap-04-{i}")).unwrap();
        state
            .db
            .create_snapshot(&SnapshotRecord {
                snapshot_id: sid,
                profile_id: pid.clone(),
                status: BackupStatus::Completed,
                metadata: None,
                created_at: format!("2026-10-0{i}T12:00:00Z"),
            })
            .unwrap();
    }

    // Set retention policy to keep latest 2
    set_retention_policy(
        state.clone(),
        SetRetentionPolicyRequest {
            profile_id: profile.profile_id.clone(),
            keep_latest_n: Some(2),
            keep_newer_than_secs: None,
            keep_latest_successful: Some(true),
            keep_latest_always: Some(true),
            prune_failed: Some(false),
            prune_empty: Some(false),
            enabled: Some(true),
        },
    )
    .expect("set policy");

    // Execute retention
    let result =
        execute_retention(state.clone(), profile.profile_id.clone()).expect("execute_retention");

    assert!(result.success);
    assert!(!result.dry_run);
    assert_eq!(result.pruned_snapshots.len(), 3);

    // Verify database only has 2 snapshots remaining
    let remaining = state.db.list_snapshots_by_profile(&pid).unwrap();
    assert_eq!(remaining.len(), 2);
    let remaining_ids: Vec<String> = remaining
        .iter()
        .map(|s| s.snapshot_id.to_string())
        .collect();
    assert!(remaining_ids.contains(&"snap-04-4".to_string()));
    assert!(remaining_ids.contains(&"snap-04-5".to_string()));

    // Verify retention history entry recorded
    let history = get_retention_history(state.clone(), profile.profile_id.clone(), Some(10))
        .expect("get_retention_history");

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].profile_id, profile.profile_id);
    assert_eq!(history[0].snapshots_evaluated, 5);
    assert_eq!(history[0].snapshots_kept, 2);
    assert_eq!(history[0].snapshots_pruned, 3);
    assert_eq!(history[0].status, "completed");

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_retention_conflict_with_active_backup() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let profile = create_test_profile(state.clone(), &temp_dir, "prof-ret-05");
    let pid = ProfileId::new(&profile.profile_id).unwrap();

    // Acquire execution guard lock simulating an in-progress backup run
    let guard = state.scheduler_service.execution_guard().try_acquire(&pid);
    assert!(guard.is_ok(), "Must acquire execution lock");

    // Attempting to execute retention should return a conflict error
    let err = execute_retention(state.clone(), profile.profile_id.clone())
        .expect_err("Must fail when backup is running");

    assert_eq!(err.code, "CONFLICT");
    assert!(err.message.contains("currently running"));

    // Drop guard and verify execute succeeds
    drop(guard);
    let result = execute_retention(state.clone(), profile.profile_id.clone())
        .expect("execute_retention succeeds after guard released");
    assert!(result.success);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_retention_invalid_profile_id() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let err = get_retention_policy(state.clone(), "".to_string())
        .expect_err("Empty profile ID must fail");

    assert_eq!(err.code, "VALIDATION_ERROR");

    let _ = fs::remove_dir_all(temp_dir);
}
