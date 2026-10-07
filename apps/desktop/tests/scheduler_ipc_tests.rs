//! Comprehensive IPC integration tests for background scheduler commands.

use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use televault_desktop::commands::backup::*;
use televault_desktop::commands::scheduler::*;
use televault_desktop::dto::*;
use televault_desktop::state::DesktopAppState;

fn setup_test_state() -> (tauri::App<tauri::test::MockRuntime>, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("televault_sched_ipc_test_{nanos}"));
    fs::create_dir_all(&temp_dir).unwrap();

    let state = DesktopAppState::new_in_memory();
    let app = tauri::test::mock_app();
    app.manage(state);
    (app, temp_dir)
}

#[test]
fn test_scheduler_status_and_defaults() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let status = get_scheduler_status(state).expect("get_scheduler_status");
    assert_eq!(status.status, "stopped");
    assert_eq!(status.active_schedules_count, 0);
    assert!(status.running_profiles.is_empty());

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_scheduler_schedule_crud_lifecycle() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let source = temp_dir.join("sched_docs");
    fs::create_dir_all(&source).unwrap();

    // 1. Create a backup profile
    let profile = create_backup_profile(
        state.clone(),
        CreateProfileRequest {
            profile_id: "prof-sched-01".to_string(),
            name: "Work Documents".to_string(),
            description: Some("Automated scheduled work documents".to_string()),
            source_path: source.to_string_lossy().to_string(),
        },
    )
    .expect("create profile");

    // 2. Create schedule for profile
    let sched = create_schedule(
        state.clone(),
        CreateScheduleRequest {
            profile_id: profile.profile_id.clone(),
            schedule_type: "interval".to_string(),
            expression: "1h".to_string(),
            timezone: Some("utc".to_string()),
            enabled: Some(true),
        },
    )
    .expect("create schedule");

    assert_eq!(sched.profile_id, profile.profile_id);
    assert_eq!(sched.schedule_type, "interval");
    assert_eq!(sched.expression, "1h");
    assert!(sched.enabled);
    assert!(sched.next_run_at.is_some());

    // 3. Query schedule by ID
    let fetched = get_schedule(state.clone(), sched.schedule_id.clone()).expect("get schedule");
    assert_eq!(fetched.schedule_id, sched.schedule_id);

    // 4. List schedules
    let list = list_schedules(state.clone()).expect("list schedules");
    assert_eq!(list.len(), 1);

    // 5. Update schedule expression
    let updated = update_schedule(
        state.clone(),
        UpdateScheduleRequest {
            schedule_id: sched.schedule_id.clone(),
            schedule_type: None,
            expression: Some("6h".to_string()),
            timezone: None,
            enabled: None,
        },
    )
    .expect("update schedule");
    assert_eq!(updated.expression, "6h");

    // 6. Disable schedule
    let disabled = disable_schedule(state.clone(), sched.schedule_id.clone()).expect("disable");
    assert!(!disabled.enabled);
    assert!(disabled.next_run_at.is_none());

    // 7. Enable schedule
    let enabled = enable_schedule(state.clone(), sched.schedule_id.clone()).expect("enable");
    assert!(enabled.enabled);
    assert!(enabled.next_run_at.is_some());

    // 8. Delete schedule
    let deleted = delete_schedule(state.clone(), sched.schedule_id.clone()).expect("delete");
    assert!(deleted);

    let list_after = list_schedules(state.clone()).expect("list schedules");
    assert_eq!(list_after.len(), 0);

    let _ = fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_scheduler_manual_immediate_trigger_and_history() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let source = temp_dir.join("trigger_docs");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("data.txt"),
        b"Automated scheduler execution test data",
    )
    .unwrap();

    let profile = create_backup_profile(
        state.clone(),
        CreateProfileRequest {
            profile_id: "prof-trigger-01".to_string(),
            name: "Trigger Profile".to_string(),
            description: None,
            source_path: source.to_string_lossy().to_string(),
        },
    )
    .expect("create profile");

    let sched = create_schedule(
        state.clone(),
        CreateScheduleRequest {
            profile_id: profile.profile_id.clone(),
            schedule_type: "daily".to_string(),
            expression: "02:00".to_string(),
            timezone: Some("utc".to_string()),
            enabled: Some(true),
        },
    )
    .expect("create schedule");

    // Trigger immediate run
    let snap_id = run_schedule_now(state.clone(), sched.schedule_id.clone())
        .await
        .expect("run schedule now");
    assert!(snap_id.is_some());

    // Inspect execution history
    let history = get_schedule_history(state.clone(), sched.schedule_id.clone(), Some(10))
        .expect("get schedule history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, "completed");
    assert_eq!(history[0].snapshot_id, snap_id);

    let _ = fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_scheduler_manual_backup_coordination_prevents_duplicate() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let source = temp_dir.join("coord_docs");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("file.txt"), b"Coordination test").unwrap();

    let profile = create_backup_profile(
        state.clone(),
        CreateProfileRequest {
            profile_id: "prof-coord-01".to_string(),
            name: "Coordination Profile".to_string(),
            description: None,
            source_path: source.to_string_lossy().to_string(),
        },
    )
    .expect("create profile");

    let pid = televault_core::ids::ProfileId::new(&profile.profile_id).unwrap();

    // Manually lock profile in execution guard
    let _guard = state
        .scheduler_service
        .execution_guard()
        .try_acquire(&pid)
        .expect("acquire guard");

    // Attempting to manually start backup for this profile must fail with CONFLICT
    let res = start_backup(
        state.clone(),
        StartBackupRequest {
            profile_id: profile.profile_id.clone(),
            passphrase: None,
        },
    )
    .await;

    assert!(res.is_err());
    let err = res.err().unwrap();
    assert_eq!(err.code, "CONFLICT");

    drop(_guard);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_scheduler_invalid_expression_error_mapping() {
    let (app, temp_dir) = setup_test_state();
    let state = app.state::<DesktopAppState>();

    let source = temp_dir.join("invalid_expr_docs");
    fs::create_dir_all(&source).unwrap();

    let profile = create_backup_profile(
        state.clone(),
        CreateProfileRequest {
            profile_id: "prof-invalid-01".to_string(),
            name: "Invalid Expr Profile".to_string(),
            description: None,
            source_path: source.to_string_lossy().to_string(),
        },
    )
    .expect("create profile");

    // Invalid interval
    let res = create_schedule(
        state.clone(),
        CreateScheduleRequest {
            profile_id: profile.profile_id.clone(),
            schedule_type: "interval".to_string(),
            expression: "0m".to_string(),
            timezone: None,
            enabled: Some(true),
        },
    );
    assert!(res.is_err());
    let err = res.err().unwrap();
    assert_eq!(err.code, "INVALID_EXPRESSION");

    let _ = fs::remove_dir_all(temp_dir);
}
