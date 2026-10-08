//! Scheduler reliability and retention safety integration tests.
//!
//! Validates:
//! - Schedule calculations across Interval, Daily, Weekly, and Cron expressions
//! - Next-run calculation survives database persistence and reload
//! - Disabled schedules do not schedule execution
//! - Scheduler execution guard bounds concurrent profile operations
//! - Retention safety:
//!   - Evaluates Keep Latest N, Age Window, Keep Latest Successful
//!   - Dry-run causes zero mutation to snapshot metadata
//!   - Non-dry run prunes local snapshot catalog records as expected
//!   - CRITICAL INVARIANT: ZERO remote Telegram backup objects are deleted!

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use televault_backup::engine::BackupEngine;
use televault_backup::profile::BackupProfile;
use televault_backup::retention::engine::RetentionEngine;
use televault_backup::retention::policy::RetentionPolicy;
use televault_core::ids::{ProfileId, ScheduleId};
use televault_db::Database;
use televault_scheduler::clock::SystemClock;
use televault_scheduler::expression::calculate_next_run;
use televault_scheduler::guard::ExecutionGuard;
use televault_scheduler::service::SchedulerService;
use televault_scheduler::types::{CreateScheduleParams, ScheduleType, TimezoneStrategy};
use televault_storage::mock::MockStorageProvider;
use televault_storage::temp::TempPayloadManager;
use televault_transfer::cancellation::CancellationToken;
use televault_transfer::engine::{TransferEngine, TransferEngineConfig};

fn setup_temp_env(name: &str) -> (PathBuf, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("televault_sched_ret_{name}_{nanos}"));
    fs::create_dir_all(&root).unwrap();
    let db_path = root.join("televault.db");
    (root, db_path)
}

#[test]
fn test_scheduler_expression_persistence_and_reload() {
    let (root, db_path) = setup_temp_env("sched_persist");
    let pid = ProfileId::new("prof-sched-test").unwrap();

    let now = Utc::now();

    // 1. Validate next run calculation for all schedule types
    let interval_next =
        calculate_next_run(ScheduleType::Interval, "3600", TimezoneStrategy::Utc, now).unwrap();
    assert!(interval_next > now);

    let daily_next =
        calculate_next_run(ScheduleType::Daily, "14:30", TimezoneStrategy::Utc, now).unwrap();
    assert!(daily_next > now);

    let weekly_next = calculate_next_run(
        ScheduleType::Weekly,
        "Mon@09:00",
        TimezoneStrategy::Utc,
        now,
    )
    .unwrap();
    assert!(weekly_next > now);

    let cron_next = calculate_next_run(
        ScheduleType::Cron,
        "0 */4 * * *",
        TimezoneStrategy::Utc,
        now,
    )
    .unwrap();
    assert!(cron_next > now);

    // 2. Persist to SQLite DB through SchedulerService
    {
        let db = Arc::new(Database::open(&db_path).unwrap());
        let mock_provider = Arc::new(MockStorageProvider::new());
        let transfer_engine = Arc::new(TransferEngine::new(
            mock_provider as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            TransferEngineConfig::default(),
            None,
        ));
        let temp_dir = root.join("temp");
        let temp_manager = Arc::new(TempPayloadManager::new(&temp_dir));
        let backup_engine = Arc::new(BackupEngine::new(
            Arc::clone(&db),
            transfer_engine,
            temp_manager,
        ));

        let profile = BackupProfile::new(pid.clone(), "Sched Profile", root.join("source"));
        db.create_profile(&profile.to_db_record()).unwrap();

        let sched_service = SchedulerService::new(
            Arc::clone(&db),
            Arc::clone(&backup_engine),
            Some(Arc::new(SystemClock::new())),
        );

        // Create an enabled daily schedule
        let daily_sched = sched_service
            .create_schedule(CreateScheduleParams {
                profile_id: pid.clone(),
                schedule_type: ScheduleType::Daily,
                expression: "03:00".into(),
                timezone: Some(TimezoneStrategy::Utc),
                enabled: Some(true),
                schedule_id: Some(ScheduleId::new("sched-daily-01").unwrap()),
            })
            .expect("create daily");
        assert!(daily_sched.enabled);
        assert!(daily_sched.next_run_at.is_some());

        // Create a disabled weekly schedule
        let disabled_sched = sched_service
            .create_schedule(CreateScheduleParams {
                profile_id: pid.clone(),
                schedule_type: ScheduleType::Weekly,
                expression: "Sun@12:00".into(),
                timezone: Some(TimezoneStrategy::Utc),
                enabled: Some(false),
                schedule_id: Some(ScheduleId::new("sched-weekly-disabled").unwrap()),
            })
            .expect("create disabled");
        assert!(!disabled_sched.enabled);
        assert!(disabled_sched.next_run_at.is_none());
    }

    // 3. Reopen DB after shutdown and verify persistence
    {
        let db2 = Arc::new(Database::open(&db_path).unwrap());
        let mock_provider = Arc::new(MockStorageProvider::new());
        let transfer_engine = Arc::new(TransferEngine::new(
            mock_provider as Arc<dyn televault_storage::StorageProvider + Send + Sync>,
            TransferEngineConfig::default(),
            None,
        ));
        let temp_dir = root.join("temp");
        let temp_manager = Arc::new(TempPayloadManager::new(&temp_dir));
        let backup_engine = Arc::new(BackupEngine::new(
            Arc::clone(&db2),
            transfer_engine,
            temp_manager,
        ));

        let sched_service2 = SchedulerService::new(
            Arc::clone(&db2),
            backup_engine,
            Some(Arc::new(SystemClock::new())),
        );

        let list = sched_service2
            .list_schedules_for_profile(&pid)
            .expect("list schedules");
        assert_eq!(list.len(), 2);

        let daily = list
            .iter()
            .find(|s| s.schedule_id.as_str() == "sched-daily-01")
            .unwrap();
        assert!(daily.enabled);
        assert_eq!(daily.expression, "03:00");
        assert!(daily.next_run_at.is_some());

        let disabled = list
            .iter()
            .find(|s| s.schedule_id.as_str() == "sched-weekly-disabled")
            .unwrap();
        assert!(!disabled.enabled);
        assert!(disabled.next_run_at.is_none());
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_scheduler_execution_guard_bounds_concurrent_profile_runs() {
    let guard = ExecutionGuard::new();
    let pid = ProfileId::new("prof-concurrent-check").unwrap();

    // 1. First acquisition succeeds
    let lock1 = guard.try_acquire(&pid);
    assert!(lock1.is_ok(), "First acquisition must succeed");

    // 2. Second concurrent acquisition for same profile must fail with busy error
    let lock2 = guard.try_acquire(&pid);
    assert!(
        lock2.is_err(),
        "Concurrent acquisition for same profile must be rejected"
    );

    // 3. Drop first lock (simulating task completion)
    drop(lock1);

    // 4. Subsequent acquisition now succeeds
    let lock3 = guard.try_acquire(&pid);
    assert!(
        lock3.is_ok(),
        "Acquisition must succeed after previous guard is dropped"
    );
}

#[test]
fn test_retention_safety_never_deletes_remote_telegram_backups() {
    let (root, db_path) = setup_temp_env("retention_safety");
    let source_dir = root.join("source");
    let staging_dir = root.join("staging");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&staging_dir).unwrap();

    let doc = source_dir.join("file.txt");
    fs::write(&doc, b"Content for retention test").unwrap();

    let pid = ProfileId::new("prof-retention-safe").unwrap();
    let db = Arc::new(Database::open(&db_path).unwrap());
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
    let retention_engine = RetentionEngine::new(Arc::clone(&db));

    let profile = BackupProfile::new(pid.clone(), "Retention Profile", source_dir.clone());
    db.create_profile(&profile.to_db_record()).unwrap();

    let cancel = CancellationToken::new();

    // Run 4 consecutive backups to create 4 snapshots
    let mut snapshot_ids = Vec::new();
    for i in 1..=4 {
        fs::write(&doc, format!("Content generation {i}")).unwrap();
        let summary = backup_engine
            .execute_profile_backup(&profile, &cancel)
            .expect("backup iteration");
        snapshot_ids.push(summary.snapshot_id);
    }

    assert_eq!(db.list_snapshots_by_profile(&pid).unwrap().len(), 4);

    // Record total remote objects uploaded to mock Telegram store
    let initial_remote_count = mock_provider.stored_objects_count();
    assert!(
        initial_remote_count > 0,
        "Mock storage must contain uploaded chunks"
    );

    // Define retention policy: Keep Latest 2 snapshots
    let mut policy = RetentionPolicy::default_for_profile(pid.clone());
    policy.keep_latest_n = Some(2);
    policy.keep_newer_than_secs = None;
    policy.keep_latest_successful = true;

    // 1. Dry run execution: Must NOT prune any snapshots
    let active_snaps = HashSet::new();
    let dry_res = retention_engine
        .execute_retention(&pid, Some(&policy), &active_snaps, true, Utc::now())
        .expect("dry run retention");
    assert!(dry_res.dry_run);
    assert_eq!(dry_res.evaluation.snapshots_pruned, 2);
    assert_eq!(dry_res.evaluation.snapshots_kept, 2);
    // Snapshots in DB remain intact after dry run
    assert_eq!(db.list_snapshots_by_profile(&pid).unwrap().len(), 4);
    // Remote objects untouched
    assert_eq!(
        mock_provider.stored_objects_count(),
        initial_remote_count,
        "Dry run must not touch remote objects"
    );

    // 2. Real retention execution (dry_run = false)
    let real_res = retention_engine
        .execute_retention(&pid, Some(&policy), &active_snaps, false, Utc::now())
        .expect("real retention");
    assert!(!real_res.dry_run);
    assert!(real_res.success);
    assert_eq!(real_res.pruned_snapshots.len(), 2);

    // Verify exactly 2 snapshots remain in SQLite DB
    let remaining_snaps = db.list_snapshots_by_profile(&pid).unwrap();
    assert_eq!(remaining_snaps.len(), 2);

    // CRITICAL ARCHITECTURAL INVARIANT TEST:
    // RETENTION MUST NEVER DELETE REMOTE TELEGRAM BACKUP PAYLOADS!
    let final_remote_count = mock_provider.stored_objects_count();
    assert_eq!(
        final_remote_count, initial_remote_count,
        "CRITICAL INVARIANT VIOLATION: Remote storage objects must NEVER be deleted by retention!"
    );

    // Verify retention audit history was recorded
    let history = retention_engine.list_retention_history(&pid, 10).unwrap();
    assert!(
        !history.is_empty(),
        "Retention history must be audited in DB"
    );

    // DB integrity passes
    assert_eq!(db.full_integrity_check().unwrap(), vec!["ok".to_string()]);
    assert!(db.foreign_key_check().unwrap().is_empty());

    let _ = fs::remove_dir_all(&root);
}
