use chrono::{TimeZone, Utc};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use televault_backup::engine::BackupEngine;
use televault_core::ids::ProfileId;
use televault_db::{Database, ProfileRecord};
use televault_scheduler::clock::{Clock, MockClock};
use televault_scheduler::error::SchedulerError;
use televault_scheduler::expression::validate_expression;
use televault_scheduler::service::SchedulerService;
use televault_scheduler::types::{
    CreateScheduleParams, ScheduleType, TimezoneStrategy, UpdateScheduleParams,
};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

/// Test fixture bundle providing mock storage, database, and scheduler.
struct TestFixture {
    service: Arc<SchedulerService>,
    db: Arc<Database>,
    clock: Arc<MockClock>,
    temp_dir: PathBuf,
    source_dir: PathBuf,
    profile_id: ProfileId,
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

fn setup_test_fixture() -> TestFixture {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("televault_scheduler_test_{nanos}"));
    let staging_dir = temp_dir.join("staging");
    let source_dir = temp_dir.join("source");
    fs::create_dir_all(&staging_dir).expect("create staging dir");
    fs::create_dir_all(&source_dir).expect("create source dir");

    // Create sample file to back up
    fs::write(source_dir.join("doc.txt"), b"Hello scheduled backup world!")
        .expect("write test file");

    let db = Arc::new(Database::open_in_memory().expect("open in-memory db"));
    let storage_provider = Arc::new(MockStorageProvider::new());
    let temp_manager = Arc::new(TempPayloadManager::new(staging_dir));
    let transfer_engine = Arc::new(TransferEngine::new(
        storage_provider,
        TransferEngineConfig::default(),
        None,
    ));
    let backup_engine = Arc::new(BackupEngine::new(
        Arc::clone(&db),
        transfer_engine,
        temp_manager,
    ));

    let profile_id = ProfileId::new("profile-test-01").unwrap();
    let profile_record = ProfileRecord {
        profile_id: profile_id.clone(),
        name: "Test Profile".to_string(),
        description: Some("Automated test profile".to_string()),
        source_path: source_dir.to_string_lossy().to_string(),
        enabled: true,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };
    db.create_profile(&profile_record).expect("create profile");

    let initial_time = Utc.with_ymd_and_hms(2026, 10, 7, 12, 0, 0).unwrap();
    let clock = Arc::new(MockClock::new(initial_time));
    let service = Arc::new(SchedulerService::new(
        Arc::clone(&db),
        backup_engine,
        Some(clock.clone()),
    ));

    TestFixture {
        service,
        db,
        clock,
        temp_dir,
        source_dir,
        profile_id,
    }
}

#[test]
fn test_schedule_validation_exhaustive() {
    // Valid interval
    assert!(validate_expression(ScheduleType::Interval, "15m").is_ok());
    assert!(validate_expression(ScheduleType::Interval, "1h").is_ok());
    assert!(validate_expression(ScheduleType::Interval, "24h").is_ok());
    assert!(validate_expression(ScheduleType::Interval, "3600").is_ok());

    // Invalid interval
    assert!(validate_expression(ScheduleType::Interval, "30s").is_err()); // Under minimum 60s
    assert!(validate_expression(ScheduleType::Interval, "0m").is_err());
    assert!(validate_expression(ScheduleType::Interval, "-5m").is_err());
    assert!(validate_expression(ScheduleType::Interval, "abc").is_err());

    // Valid daily
    assert!(validate_expression(ScheduleType::Daily, "02:00").is_ok());
    assert!(validate_expression(ScheduleType::Daily, "23:59").is_ok());
    assert!(validate_expression(ScheduleType::Daily, "00:00").is_ok());

    // Invalid daily
    assert!(validate_expression(ScheduleType::Daily, "24:00").is_err());
    assert!(validate_expression(ScheduleType::Daily, "12:60").is_err());
    assert!(validate_expression(ScheduleType::Daily, "invalid").is_err());

    // Valid weekly
    assert!(validate_expression(ScheduleType::Weekly, "Sun@03:00").is_ok());
    assert!(validate_expression(ScheduleType::Weekly, "Monday 14:00").is_ok());

    // Invalid weekly
    assert!(validate_expression(ScheduleType::Weekly, "Funday@12:00").is_err());
    assert!(validate_expression(ScheduleType::Weekly, "Mon@25:00").is_err());

    // Valid cron
    assert!(validate_expression(ScheduleType::Cron, "*/15 * * * *").is_ok());
    assert!(validate_expression(ScheduleType::Cron, "0 2 * * *").is_ok());
    assert!(validate_expression(ScheduleType::Cron, "0 3 * * 0").is_ok());

    // Invalid cron
    assert!(validate_expression(ScheduleType::Cron, "* * *").is_err());
    assert!(validate_expression(ScheduleType::Cron, "60 * * * *").is_err());
    assert!(validate_expression(ScheduleType::Cron, "* 25 * * *").is_err());
}

#[test]
fn test_schedule_persistence_lifecycle() {
    let fixture = setup_test_fixture();
    let service = &fixture.service;

    // 1. Create schedule
    let schedule = service
        .create_schedule(CreateScheduleParams {
            schedule_id: None,
            profile_id: fixture.profile_id.clone(),
            schedule_type: ScheduleType::Interval,
            expression: "1h".to_string(),
            timezone: Some(TimezoneStrategy::Utc),
            enabled: Some(true),
        })
        .expect("create schedule");

    assert_eq!(schedule.schedule_type, ScheduleType::Interval);
    assert_eq!(schedule.expression, "1h");
    assert!(schedule.enabled);
    assert!(schedule.next_run_at.is_some());

    // 2. Query schedule
    let retrieved = service
        .get_schedule(&schedule.schedule_id)
        .expect("get schedule");
    assert_eq!(retrieved.schedule_id, schedule.schedule_id);

    // 3. Update schedule expression to 2h
    let updated = service
        .update_schedule(UpdateScheduleParams {
            schedule_id: schedule.schedule_id.clone(),
            schedule_type: None,
            expression: Some("2h".to_string()),
            timezone: None,
            enabled: None,
        })
        .expect("update schedule");
    assert_eq!(updated.expression, "2h");

    // 4. Disable schedule
    let disabled = service
        .disable_schedule(&schedule.schedule_id)
        .expect("disable schedule");
    assert!(!disabled.enabled);
    assert!(disabled.next_run_at.is_none());

    // 5. Re-enable schedule
    let re_enabled = service
        .enable_schedule(&schedule.schedule_id)
        .expect("enable schedule");
    assert!(re_enabled.enabled);
    assert!(re_enabled.next_run_at.is_some());

    // 6. Delete schedule
    let deleted = service
        .delete_schedule(&schedule.schedule_id)
        .expect("delete schedule");
    assert!(deleted);
    assert!(service.get_schedule(&schedule.schedule_id).is_err());
}

#[test]
fn test_duplicate_execution_prevention_via_guard() {
    let fixture = setup_test_fixture();
    let guard = fixture.service.execution_guard();
    let profile_a = &fixture.profile_id;
    let profile_b = ProfileId::new("profile-other").unwrap();

    // 1. Acquire execution lock for profile A
    let lock_a = guard.try_acquire(profile_a).expect("acquire profile A");
    assert!(guard.is_running(profile_a));

    // 2. Concurrent acquire for profile A fails
    let conflict = guard.try_acquire(profile_a);
    assert!(matches!(
        conflict,
        Err(SchedulerError::ProfileAlreadyRunning(_))
    ));

    // 3. Independent profile B can acquire concurrently
    let lock_b = guard.try_acquire(&profile_b).expect("acquire profile B");
    assert!(guard.is_running(&profile_b));

    // 4. Dropping lock_a releases profile A while B is still active
    drop(lock_a);
    assert!(!guard.is_running(profile_a));
    assert!(guard.is_running(&profile_b));

    drop(lock_b);
    assert!(!guard.is_running(&profile_b));
}

#[test]
fn test_real_automated_backup_execution_and_history_tracking() {
    let fixture = setup_test_fixture();
    let service = &fixture.service;

    // 1. Create a schedule
    let schedule = service
        .create_schedule(CreateScheduleParams {
            schedule_id: None,
            profile_id: fixture.profile_id.clone(),
            schedule_type: ScheduleType::Interval,
            expression: "1h".to_string(),
            timezone: Some(TimezoneStrategy::Utc),
            enabled: Some(true),
        })
        .expect("create schedule");

    // 2. Trigger execution immediately on demand
    let snapshot_id = service
        .trigger_schedule_now(&schedule.schedule_id)
        .expect("trigger schedule now");
    assert!(snapshot_id.is_some());

    // 3. Inspect schedule status
    let updated_sched = service
        .get_schedule(&schedule.schedule_id)
        .expect("get schedule");
    assert_eq!(updated_sched.last_status.as_deref(), Some("completed"));
    assert!(updated_sched.last_run_at.is_some());
    assert!(updated_sched.next_run_at.is_some());

    // 4. Inspect execution history recorded in SQLite
    let history = service
        .get_history(&schedule.schedule_id, 10)
        .expect("get history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, "completed");
    assert_eq!(history[0].snapshot_id, snapshot_id);
    assert!(history[0].files_processed >= 1);
}

#[test]
fn test_multiple_independent_schedules() {
    let fixture = setup_test_fixture();
    let service = &fixture.service;

    // Register second profile
    let profile_b_id = ProfileId::new("profile-b-test").unwrap();
    let profile_b_record = ProfileRecord {
        profile_id: profile_b_id.clone(),
        name: "Profile B".to_string(),
        description: None,
        source_path: fixture.source_dir.to_string_lossy().to_string(),
        enabled: true,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };
    fixture
        .db
        .create_profile(&profile_b_record)
        .expect("create profile B");

    // Create schedule for Profile A
    let sched_a = service
        .create_schedule(CreateScheduleParams {
            schedule_id: None,
            profile_id: fixture.profile_id.clone(),
            schedule_type: ScheduleType::Daily,
            expression: "02:00".to_string(),
            timezone: Some(TimezoneStrategy::Utc),
            enabled: Some(true),
        })
        .expect("create sched A");

    // Create schedule for Profile B
    let sched_b = service
        .create_schedule(CreateScheduleParams {
            schedule_id: None,
            profile_id: profile_b_id.clone(),
            schedule_type: ScheduleType::Interval,
            expression: "6h".to_string(),
            timezone: Some(TimezoneStrategy::Utc),
            enabled: Some(true),
        })
        .expect("create sched B");

    let all_schedules = service.list_schedules().expect("list schedules");
    assert_eq!(all_schedules.len(), 2);

    let profile_a_schedules = service
        .list_schedules_for_profile(&fixture.profile_id)
        .expect("list for profile A");
    assert_eq!(profile_a_schedules.len(), 1);
    assert_eq!(profile_a_schedules[0].schedule_id, sched_a.schedule_id);

    let profile_b_schedules = service
        .list_schedules_for_profile(&profile_b_id)
        .expect("list for profile B");
    assert_eq!(profile_b_schedules.len(), 1);
    assert_eq!(profile_b_schedules[0].schedule_id, sched_b.schedule_id);
}

#[test]
fn test_profile_deletion_cascades_to_schedules() {
    let fixture = setup_test_fixture();
    let service = &fixture.service;

    let sched = service
        .create_schedule(CreateScheduleParams {
            schedule_id: None,
            profile_id: fixture.profile_id.clone(),
            schedule_type: ScheduleType::Interval,
            expression: "1h".to_string(),
            timezone: Some(TimezoneStrategy::Utc),
            enabled: Some(true),
        })
        .expect("create schedule");

    // Delete profile from database
    fixture
        .db
        .delete_profile(&fixture.profile_id)
        .expect("delete profile");

    // Foreign key CASCADE in SQLite automatically deleted the schedule
    let res = service.get_schedule(&sched.schedule_id);
    assert!(res.is_err());
}

#[tokio::test]
async fn test_scheduler_service_lifecycle_and_graceful_shutdown() {
    let fixture = setup_test_fixture();
    let service = &fixture.service;

    // Start background scheduler loop
    service.start().expect("start scheduler");
    assert_eq!(
        service.status(),
        televault_scheduler::types::SchedulerServiceStatus::Running
    );

    // Give scheduler loop a moment to run
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Gracefully stop scheduler
    service.stop().await.expect("stop scheduler");
    assert_eq!(
        service.status(),
        televault_scheduler::types::SchedulerServiceStatus::Stopped
    );
}

#[tokio::test]
async fn test_clock_advance_and_missed_schedule_recovery() {
    let fixture = setup_test_fixture();
    let service = &fixture.service;

    // Create a 1-hour interval schedule
    let sched = service
        .create_schedule(CreateScheduleParams {
            schedule_id: None,
            profile_id: fixture.profile_id.clone(),
            schedule_type: ScheduleType::Interval,
            expression: "1h".to_string(),
            timezone: Some(TimezoneStrategy::Utc),
            enabled: Some(true),
        })
        .expect("create schedule");

    let initial_next = sched.next_run_at.unwrap();

    // Advance clock past next_run_at (e.g. app was closed / idle for 2 hours)
    fixture.clock.advance(chrono::Duration::hours(2));

    // Verify clock advancement
    assert!(fixture.clock.now_utc() > initial_next);

    // Trigger due execution
    let snap_id = service
        .trigger_schedule_now(&sched.schedule_id)
        .expect("catch-up execution");
    assert!(snap_id.is_some());

    // Next run is now calculated deterministically from the advanced time
    let updated = service
        .get_schedule(&sched.schedule_id)
        .expect("get schedule");
    assert!(updated.next_run_at.unwrap() > fixture.clock.now_utc());
}
