//! Comprehensive retention policy engine and snapshot pruning tests for TELEVAULT Phase 12.

use chrono::{Duration, TimeZone, Utc};
use std::collections::HashSet;
use std::sync::Arc;
use televault_backup::retention::{
    RetentionAction, RetentionEngine, RetentionPolicy, RetentionReason,
};
use televault_core::ids::{ChunkId, FileId, ProfileId, SnapshotId, VersionId};
use televault_core::models::BackupStatus;
use televault_db::{
    ChunkRecord, Database, FileRecord, ProfileRecord, SnapshotRecord, VersionRecord,
};
use televault_manifest::{
    ChunkManifest, CompressionMetadata, IntegrityMetadata, LogicalFileMetadata, ManifestV1,
    ManifestVersion, StorageReference,
};

/// Helper fixture establishing a temporary in-memory database with a sample profile and files.
struct RetentionTestFixture {
    db: Arc<Database>,
    engine: RetentionEngine,
    profile_id: ProfileId,
    file_id: FileId,
    manifest_id: String,
}

impl RetentionTestFixture {
    fn new(profile_name: &str) -> Self {
        let db = Arc::new(Database::open_in_memory().expect("open memory db"));
        let engine = RetentionEngine::new(Arc::clone(&db));
        let profile_id = ProfileId::new(format!("prof-{}", profile_name)).unwrap();

        db.create_profile(&ProfileRecord {
            profile_id: profile_id.clone(),
            name: profile_name.into(),
            description: None,
            source_path: format!("D:\\{}", profile_name),
            enabled: true,
            created_at: "2026-10-01T00:00:00Z".into(),
            updated_at: "2026-10-01T00:00:00Z".into(),
        })
        .expect("create profile");

        let file_id = FileId::new("f-test-01").unwrap();
        db.create_file(&FileRecord {
            file_id: file_id.clone(),
            profile_id: Some(profile_id.clone()),
            file_name: "document.pdf".into(),
            relative_path: "document.pdf".into(),
            original_size: 1024,
            mime_type: Some("application/pdf".into()),
            status: "backed_up".into(),
            logical_file_hash: Some("sha256:abc123hash".into()),
            created_at: "2026-10-01T00:00:00Z".into(),
            modified_at: None,
            created_timestamp: "2026-10-01T00:00:00Z".into(),
            updated_timestamp: "2026-10-01T00:00:00Z".into(),
        })
        .expect("create file");

        let manifest_id = "man-01".to_string();
        let chunk_id = ChunkId::new("chunk-01").unwrap();
        let manifest = ManifestV1 {
            manifest_version: ManifestVersion::V1,
            manifest_id: manifest_id.clone(),
            logical_file: LogicalFileMetadata {
                file_id: file_id.clone(),
                file_name: "document.pdf".into(),
                relative_path: "document.pdf".into(),
                original_size: 1024,
                created_at: None,
                modified_at: None,
                mime_type: Some("application/pdf".into()),
            },
            encryption: None,
            compression: CompressionMetadata::none(),
            integrity: IntegrityMetadata::sha256(
                "1111111111111111111111111111111111111111111111111111111111111111",
            ),
            chunks: vec![ChunkManifest {
                chunk_id: chunk_id.clone(),
                index: 0,
                plaintext_size: 1024,
                stored_size: 1024,
                integrity: IntegrityMetadata::sha256(
                    "1111111111111111111111111111111111111111111111111111111111111111",
                ),
                storage_reference: StorageReference::Telegram {
                    chat_id: -1001234567890,
                    message_id: 42,
                    file_id: "telegram_doc_file_id_immutable".into(),
                },
            }],
        };
        db.save_manifest(&manifest).expect("save manifest");

        db.create_chunk(&ChunkRecord {
            chunk_id,
            file_id: file_id.clone(),
            manifest_id: manifest_id.clone(),
            chunk_index: 0,
            plaintext_size: 1024,
            stored_size: 1024,
            integrity_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                .into(),
            storage_reference:
                "StorageReference::Telegram { chat_id: -1001234567890, message_id: 42 }".into(),
            status: "verified".into(),
            created_at: "2026-10-01T00:00:00Z".into(),
            updated_at: "2026-10-01T00:00:00Z".into(),
        })
        .expect("create chunk");

        Self {
            db,
            engine,
            profile_id,
            file_id,
            manifest_id,
        }
    }

    fn add_snapshot(
        &self,
        snapshot_id: &str,
        created_at: &str,
        status: BackupStatus,
        has_versions: bool,
    ) -> SnapshotId {
        let sid = SnapshotId::new(snapshot_id).unwrap();
        self.db
            .create_snapshot(&SnapshotRecord {
                snapshot_id: sid.clone(),
                profile_id: self.profile_id.clone(),
                status,
                metadata: None,
                created_at: created_at.into(),
            })
            .expect("create snapshot");

        if has_versions {
            let vid = VersionId::new(format!("ver-{}", snapshot_id)).unwrap();
            self.db
                .create_version(&VersionRecord {
                    version_id: vid,
                    file_id: self.file_id.clone(),
                    snapshot_id: sid.clone(),
                    manifest_id: self.manifest_id.clone(),
                    status: "active".into(),
                    created_at: created_at.into(),
                })
                .expect("create version");
        }

        sid
    }
}

// =========================================================================
// Tests
// =========================================================================

#[test]
fn test_01_keep_latest_n() {
    let fixture = RetentionTestFixture::new("keep-latest-n");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    // Create 5 snapshots from oldest to newest
    let s1 = fixture.add_snapshot(
        "snap-1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s2 = fixture.add_snapshot(
        "snap-2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s3 = fixture.add_snapshot(
        "snap-3",
        "2026-10-03T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s4 = fixture.add_snapshot(
        "snap-4",
        "2026-10-04T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s5 = fixture.add_snapshot(
        "snap-5",
        "2026-10-05T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    // Retention policy: Keep latest 3 snapshots, no age limit
    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_latest_n(3)
        .with_keep_latest_always(true)
        .with_keep_latest_successful(true);

    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval");

    assert_eq!(eval.snapshots_evaluated, 5);
    assert_eq!(eval.snapshots_kept, 3);
    assert_eq!(eval.snapshots_pruned, 2);

    let pruned_ids: Vec<_> = eval
        .decisions
        .iter()
        .filter(|d| d.action == RetentionAction::Prune)
        .map(|d| d.snapshot_id.clone())
        .collect();

    assert!(pruned_ids.contains(&s1));
    assert!(pruned_ids.contains(&s2));

    let kept_ids: Vec<_> = eval
        .decisions
        .iter()
        .filter(|d| d.action == RetentionAction::Keep)
        .map(|d| d.snapshot_id.clone())
        .collect();

    assert!(kept_ids.contains(&s3));
    assert!(kept_ids.contains(&s4));
    assert!(kept_ids.contains(&s5));
}

#[test]
fn test_02_age_based_retention() {
    let fixture = RetentionTestFixture::new("age-based");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    // S1: 9 days old (777,600s)
    let s1 = fixture.add_snapshot(
        "snap-old",
        "2026-10-01T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    // S2: 4 days old (345,600s)
    let s2 = fixture.add_snapshot(
        "snap-mid",
        "2026-10-06T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    // S3: 1 day old (86,400s)
    let s3 = fixture.add_snapshot(
        "snap-new",
        "2026-10-09T12:00:00Z",
        BackupStatus::Completed,
        true,
    );

    // Keep snapshots newer than 5 days (432,000s)
    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_newer_than_secs(5 * 86400)
        .with_keep_latest_always(false)
        .with_keep_latest_successful(false);

    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval");

    assert_eq!(eval.snapshots_evaluated, 3);
    assert_eq!(eval.snapshots_kept, 2); // S2 and S3 kept
    assert_eq!(eval.snapshots_pruned, 1); // S1 pruned

    let s1_dec = eval.decisions.iter().find(|d| d.snapshot_id == s1).unwrap();
    assert_eq!(s1_dec.action, RetentionAction::Prune);
    assert_eq!(s1_dec.reason, RetentionReason::PruneExpired);

    let s2_dec = eval.decisions.iter().find(|d| d.snapshot_id == s2).unwrap();
    assert_eq!(s2_dec.action, RetentionAction::Keep);
    assert_eq!(s2_dec.reason, RetentionReason::KeepWithinRetentionWindow);

    let s3_dec = eval.decisions.iter().find(|d| d.snapshot_id == s3).unwrap();
    assert_eq!(s3_dec.action, RetentionAction::Keep);
}

#[test]
fn test_03_latest_successful_snapshot_protection() {
    let fixture = RetentionTestFixture::new("latest-successful-protection");
    let now = Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();

    // S1: An old successful snapshot (19 days old)
    let s1 = fixture.add_snapshot(
        "snap-success-old",
        "2026-10-01T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    // S2: Newer failed snapshot (5 days old)
    let _s2 = fixture.add_snapshot(
        "snap-fail-1",
        "2026-10-15T12:00:00Z",
        BackupStatus::Failed,
        true,
    );
    // S3: Newest failed snapshot (1 day old)
    let _s3 = fixture.add_snapshot(
        "snap-fail-2",
        "2026-10-19T12:00:00Z",
        BackupStatus::Failed,
        true,
    );

    // Policy: Keep snapshots newer than 3 days, and keep latest 1
    // Without latest successful protection, S1 would be pruned!
    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_latest_n(1)
        .with_keep_newer_than_secs(3 * 86400)
        .with_keep_latest_successful(true)
        .with_keep_latest_always(true)
        .with_prune_failed(true);

    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval");

    let s1_dec = eval.decisions.iter().find(|d| d.snapshot_id == s1).unwrap();
    assert_eq!(s1_dec.action, RetentionAction::Keep);
    assert_eq!(s1_dec.reason, RetentionReason::KeepLatestSuccessful);
}

#[test]
fn test_04_active_snapshot_protection() {
    let fixture = RetentionTestFixture::new("active-protection");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    // S1: Completed snapshot
    let _s1 = fixture.add_snapshot(
        "snap-comp",
        "2026-10-01T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    // S2: Currently in BackingUp status
    let s2 = fixture.add_snapshot(
        "snap-active-db",
        "2026-10-02T12:00:00Z",
        BackupStatus::BackingUp,
        true,
    );
    // S3: Completed in DB, but passed in active_snapshots set (e.g. being restored)
    let s3 = fixture.add_snapshot(
        "snap-active-restore",
        "2026-10-03T12:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let mut active_set = HashSet::new();
    active_set.insert(s3.clone());

    // Aggressive policy wanting to prune everything older than 1 second
    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_newer_than_secs(1)
        .with_keep_latest_n(1)
        .with_keep_latest_always(false)
        .with_keep_latest_successful(false);

    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &active_set, now)
        .expect("eval");

    let s2_dec = eval.decisions.iter().find(|d| d.snapshot_id == s2).unwrap();
    assert_eq!(s2_dec.action, RetentionAction::Keep);
    assert_eq!(s2_dec.reason, RetentionReason::ProtectedActive);

    let s3_dec = eval.decisions.iter().find(|d| d.snapshot_id == s3).unwrap();
    assert_eq!(s3_dec.action, RetentionAction::Keep);
    assert_eq!(s3_dec.reason, RetentionReason::ProtectedActive);
}

#[test]
fn test_05_failed_snapshot_handling() {
    let fixture = RetentionTestFixture::new("failed-handling");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let _s1 = fixture.add_snapshot(
        "snap-good",
        "2026-10-01T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s2 = fixture.add_snapshot(
        "snap-fail",
        "2026-10-02T12:00:00Z",
        BackupStatus::Failed,
        true,
    );
    let _s3 = fixture.add_snapshot(
        "snap-good-2",
        "2026-10-03T12:00:00Z",
        BackupStatus::Completed,
        true,
    );

    // Case A: prune_failed = true
    let policy_prune = RetentionPolicy::new(fixture.profile_id.clone())
        .with_prune_failed(true)
        .with_keep_latest_always(false);

    let eval_prune = fixture
        .engine
        .evaluate_retention(
            &fixture.profile_id,
            Some(&policy_prune),
            &HashSet::new(),
            now,
        )
        .expect("eval");

    let s2_dec = eval_prune
        .decisions
        .iter()
        .find(|d| d.snapshot_id == s2)
        .unwrap();
    assert_eq!(s2_dec.action, RetentionAction::Prune);
    assert_eq!(s2_dec.reason, RetentionReason::PruneFailedSnapshot);

    // Case B: prune_failed = false, within retention window
    let policy_keep = RetentionPolicy::new(fixture.profile_id.clone())
        .with_prune_failed(false)
        .with_keep_newer_than_secs(30 * 86400);

    let eval_keep = fixture
        .engine
        .evaluate_retention(
            &fixture.profile_id,
            Some(&policy_keep),
            &HashSet::new(),
            now,
        )
        .expect("eval");

    let s2_dec_keep = eval_keep
        .decisions
        .iter()
        .find(|d| d.snapshot_id == s2)
        .unwrap();
    assert_eq!(s2_dec_keep.action, RetentionAction::Keep);
}

#[test]
fn test_06_incomplete_snapshot_handling() {
    let fixture = RetentionTestFixture::new("incomplete-handling");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let _s1 = fixture.add_snapshot(
        "snap-c1",
        "2026-10-01T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s2 = fixture.add_snapshot(
        "snap-cancelled",
        "2026-10-02T12:00:00Z",
        BackupStatus::Cancelled,
        true,
    );
    let _s3 = fixture.add_snapshot(
        "snap-c2",
        "2026-10-03T12:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_prune_failed(true)
        .with_keep_latest_always(false);

    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval");

    let s2_dec = eval.decisions.iter().find(|d| d.snapshot_id == s2).unwrap();
    assert_eq!(s2_dec.action, RetentionAction::Prune);
    assert_eq!(s2_dec.reason, RetentionReason::PruneIncompleteSnapshot);
}

#[test]
fn test_07_deterministic_ordering() {
    let fixture = RetentionTestFixture::new("deterministic-order");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    // Insert snapshots in non-sequential or identical timestamp order
    let _s1 = fixture.add_snapshot(
        "snap-z",
        "2026-10-05T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let _s2 = fixture.add_snapshot(
        "snap-a",
        "2026-10-05T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let _s3 = fixture.add_snapshot(
        "snap-m",
        "2026-10-05T12:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let _s4 = fixture.add_snapshot(
        "snap-old",
        "2026-10-01T12:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::default_for_profile(fixture.profile_id.clone());

    let eval1 = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval 1");

    let eval2 = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval 2");

    // Both evaluations must match 100% identically in order and decisions
    assert_eq!(eval1.decisions, eval2.decisions);
    // Identical timestamp tie-break: snap-z, snap-m, snap-a
    assert_eq!(eval1.decisions[0].snapshot_id.as_str(), "snap-z");
    assert_eq!(eval1.decisions[1].snapshot_id.as_str(), "snap-m");
    assert_eq!(eval1.decisions[2].snapshot_id.as_str(), "snap-a");
    assert_eq!(eval1.decisions[3].snapshot_id.as_str(), "snap-old");
}

#[test]
fn test_08_dry_run_produces_no_changes() {
    let fixture = RetentionTestFixture::new("dry-run");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let s1 = fixture.add_snapshot(
        "snap-1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let _s2 = fixture.add_snapshot(
        "snap-2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_latest_n(1)
        .with_keep_latest_always(true);

    let result = fixture
        .engine
        .execute_retention(
            &fixture.profile_id,
            Some(&policy),
            &HashSet::new(),
            true, // dry_run
            now,
        )
        .expect("dry run execute");

    assert!(result.dry_run);
    assert_eq!(result.evaluation.snapshots_pruned, 1);
    assert!(result.pruned_snapshots.is_empty());
    assert_eq!(result.pruned_versions_count, 0);

    // Verify database remains untouched
    assert!(fixture.db.get_snapshot(&s1).unwrap().is_some());
    assert_eq!(
        fixture
            .db
            .list_snapshots_by_profile(&fixture.profile_id)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn test_09_actual_pruning_removes_only_approved_local_records() {
    let fixture = RetentionTestFixture::new("actual-pruning");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let s1 = fixture.add_snapshot(
        "snap-prune",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s2 = fixture.add_snapshot(
        "snap-keep",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_latest_n(1)
        .with_keep_latest_always(true);

    let result = fixture
        .engine
        .execute_retention(
            &fixture.profile_id,
            Some(&policy),
            &HashSet::new(),
            false, // actual execution
            now,
        )
        .expect("actual execute");

    assert!(!result.dry_run);
    assert_eq!(result.pruned_snapshots, vec![s1.clone()]);
    assert_eq!(result.pruned_versions_count, 1);

    // S1 pruned, S2 kept
    assert!(fixture.db.get_snapshot(&s1).unwrap().is_none());
    assert!(fixture.db.get_snapshot(&s2).unwrap().is_some());
    assert_eq!(fixture.db.count_versions_by_snapshot(&s1).unwrap(), 0);
    assert_eq!(fixture.db.count_versions_by_snapshot(&s2).unwrap(), 1);

    // History record logged
    let history = fixture
        .engine
        .list_retention_history(&fixture.profile_id, 10)
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].snapshots_pruned, 1);
    assert_eq!(history[0].status, "completed");
}

#[test]
fn test_10_remote_telegram_references_remain_untouched() {
    let fixture = RetentionTestFixture::new("remote-immutability");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let s1 = fixture.add_snapshot(
        "snap-prune-me",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let _s2 = fixture.add_snapshot(
        "snap-keep-me",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    // Fetch chunk before retention
    let chunks_before = fixture.db.list_chunks_by_file(&fixture.file_id).unwrap();
    assert_eq!(chunks_before.len(), 1);
    assert!(chunks_before[0].storage_reference.contains("Telegram"));

    // Manifest before
    let manifest_before = fixture
        .db
        .get_manifest(&fixture.manifest_id)
        .unwrap()
        .unwrap();
    assert_eq!(manifest_before.chunks.len(), 1);

    let policy = RetentionPolicy::new(fixture.profile_id.clone()).with_keep_latest_n(1);
    fixture
        .engine
        .execute_retention(
            &fixture.profile_id,
            Some(&policy),
            &HashSet::new(),
            false,
            now,
        )
        .expect("execute");

    // Verify S1 was deleted
    assert!(fixture.db.get_snapshot(&s1).unwrap().is_none());

    // Verify files, chunks, manifests and remote references are COMPLETELY intact!
    let chunks_after = fixture.db.list_chunks_by_file(&fixture.file_id).unwrap();
    assert_eq!(chunks_after.len(), 1);
    assert_eq!(
        chunks_after[0].storage_reference,
        chunks_before[0].storage_reference
    );
    assert_eq!(
        chunks_after[0].integrity_hash,
        chunks_before[0].integrity_hash
    );

    let manifest_after = fixture
        .db
        .get_manifest(&fixture.manifest_id)
        .unwrap()
        .unwrap();
    assert_eq!(manifest_after.chunks.len(), 1);
    assert_eq!(
        manifest_after.chunks[0].storage_reference,
        manifest_before.chunks[0].storage_reference
    );
}

#[test]
fn test_11_transaction_failure_does_not_leave_partial_pruning() {
    let fixture = RetentionTestFixture::new("tx-failure");

    let s1 = fixture.add_snapshot(
        "snap-1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s2 = fixture.add_snapshot(
        "snap-2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let non_existent = SnapshotId::new("snap-missing-err").unwrap();

    // Call prune_snapshots_transactional with valid s1 and invalid non_existent
    let result = fixture
        .db
        .prune_snapshots_transactional(&[s1.clone(), non_existent], None);
    assert!(result.is_err());

    // Assert s1 was NOT pruned (atomic rollback)
    assert!(fixture.db.get_snapshot(&s1).unwrap().is_some());
    assert!(fixture.db.get_snapshot(&s2).unwrap().is_some());
    assert_eq!(fixture.db.count_versions_by_snapshot(&s1).unwrap(), 1);
}

#[test]
fn test_12_idempotent_repeated_retention() {
    let fixture = RetentionTestFixture::new("idempotent");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let _s1 = fixture.add_snapshot(
        "snap-1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let _s2 = fixture.add_snapshot(
        "snap-2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let _s3 = fixture.add_snapshot(
        "snap-3",
        "2026-10-03T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::new(fixture.profile_id.clone()).with_keep_latest_n(2);

    // Run 1: prunes snap-1
    let run1 = fixture
        .engine
        .execute_retention(
            &fixture.profile_id,
            Some(&policy),
            &HashSet::new(),
            false,
            now,
        )
        .expect("run 1");
    assert_eq!(run1.pruned_snapshots.len(), 1);

    // Run 2 immediately: no new snapshots, should prune ZERO
    let run2 = fixture
        .engine
        .execute_retention(
            &fixture.profile_id,
            Some(&policy),
            &HashSet::new(),
            false,
            now,
        )
        .expect("run 2");
    assert_eq!(run2.pruned_snapshots.len(), 0);
    assert_eq!(run2.pruned_versions_count, 0);
    assert_eq!(
        fixture
            .db
            .list_snapshots_by_profile(&fixture.profile_id)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn test_13_profile_isolation() {
    let fixture_a = RetentionTestFixture::new("iso-a");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let snap_a1 = fixture_a.add_snapshot(
        "snap-a1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let snap_a2 = fixture_a.add_snapshot(
        "snap-a2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    // Add profile B in the same database
    let pid_b = ProfileId::new("prof-iso-b").unwrap();
    fixture_a
        .db
        .create_profile(&ProfileRecord {
            profile_id: pid_b.clone(),
            name: "iso-b".into(),
            description: None,
            source_path: "D:\\iso-b".into(),
            enabled: true,
            created_at: "2026-10-01T00:00:00Z".into(),
            updated_at: "2026-10-01T00:00:00Z".into(),
        })
        .expect("create b");

    let snap_b1 = SnapshotId::new("snap-b1").unwrap();
    fixture_a
        .db
        .create_snapshot(&SnapshotRecord {
            snapshot_id: snap_b1.clone(),
            profile_id: pid_b.clone(),
            status: BackupStatus::Completed,
            metadata: None,
            created_at: "2026-10-01T10:00:00Z".into(),
        })
        .expect("snap b1");

    // Execute retention on profile A to keep latest 1
    let policy_a = RetentionPolicy::new(fixture_a.profile_id.clone()).with_keep_latest_n(1);
    let res = fixture_a
        .engine
        .execute_retention(
            &fixture_a.profile_id,
            Some(&policy_a),
            &HashSet::new(),
            false,
            now,
        )
        .expect("exec a");

    assert_eq!(res.pruned_snapshots, vec![snap_a1.clone()]);
    assert!(fixture_a.db.get_snapshot(&snap_a1).unwrap().is_none());
    assert!(fixture_a.db.get_snapshot(&snap_a2).unwrap().is_some());

    // Profile B snapshot is 100% intact!
    assert!(fixture_a.db.get_snapshot(&snap_b1).unwrap().is_some());
    assert_eq!(
        fixture_a
            .db
            .list_snapshots_by_profile(&pid_b)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn test_14_large_snapshot_history_performance_behavior() {
    let fixture = RetentionTestFixture::new("large-history");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    // Create 100 snapshots
    for i in 1..=100 {
        let sid = format!("snap-{:03}", i);
        let created = format!("2026-09-{:02}T10:00:00Z", (i % 28) + 1);
        fixture.add_snapshot(&sid, &created, BackupStatus::Completed, false);
    }

    assert_eq!(
        fixture
            .db
            .list_snapshots_by_profile(&fixture.profile_id)
            .unwrap()
            .len(),
        100
    );

    let start = std::time::Instant::now();
    let policy = RetentionPolicy::new(fixture.profile_id.clone()).with_keep_latest_n(20);
    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval 100");
    let elapsed = start.elapsed();

    assert_eq!(eval.snapshots_evaluated, 100);
    assert_eq!(eval.snapshots_kept, 20);
    assert_eq!(eval.snapshots_pruned, 80);
    // Performance: evaluation over 100 snapshots must take under 100ms
    assert!(
        elapsed.as_millis() < 100,
        "Evaluation took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_15_empty_history() {
    let fixture = RetentionTestFixture::new("empty-history");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let policy = RetentionPolicy::default_for_profile(fixture.profile_id.clone());
    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval empty");

    assert_eq!(eval.snapshots_evaluated, 0);
    assert_eq!(eval.snapshots_kept, 0);
    assert_eq!(eval.snapshots_pruned, 0);
    assert!(eval.decisions.is_empty());

    let res = fixture
        .engine
        .execute_retention(
            &fixture.profile_id,
            Some(&policy),
            &HashSet::new(),
            false,
            now,
        )
        .expect("exec empty");
    assert_eq!(res.pruned_snapshots.len(), 0);
}

#[test]
fn test_16_fewer_snapshots_than_retention_limit() {
    let fixture = RetentionTestFixture::new("fewer-than-limit");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    fixture.add_snapshot(
        "snap-1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    fixture.add_snapshot(
        "snap-2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::new(fixture.profile_id.clone()).with_keep_latest_n(5);
    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval");

    assert_eq!(eval.snapshots_evaluated, 2);
    assert_eq!(eval.snapshots_kept, 2);
    assert_eq!(eval.snapshots_pruned, 0);
}

#[test]
fn test_17_exactly_n_snapshots() {
    let fixture = RetentionTestFixture::new("exactly-n");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    fixture.add_snapshot(
        "snap-1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    fixture.add_snapshot(
        "snap-2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    fixture.add_snapshot(
        "snap-3",
        "2026-10-03T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::new(fixture.profile_id.clone()).with_keep_latest_n(3);
    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval");

    assert_eq!(eval.snapshots_evaluated, 3);
    assert_eq!(eval.snapshots_kept, 3);
    assert_eq!(eval.snapshots_pruned, 0);
}

#[test]
fn test_18_boundary_timestamp_behavior() {
    let fixture = RetentionTestFixture::new("boundary-timestamp");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();
    let max_age_secs = 5 * 86400; // 5 days

    // Snap exact: exactly 5 days old
    let exact_boundary_time = now - Duration::seconds(max_age_secs as i64);
    let s_exact = fixture.add_snapshot(
        "snap-exact",
        &exact_boundary_time.to_rfc3339(),
        BackupStatus::Completed,
        true,
    );

    // Snap over: 5 days + 1 second old
    let over_boundary_time = now - Duration::seconds((max_age_secs + 1) as i64);
    let s_over = fixture.add_snapshot(
        "snap-over",
        &over_boundary_time.to_rfc3339(),
        BackupStatus::Completed,
        true,
    );

    // Snap under: 5 days - 1 second old
    let under_boundary_time = now - Duration::seconds((max_age_secs - 1) as i64);
    let s_under = fixture.add_snapshot(
        "snap-under",
        &under_boundary_time.to_rfc3339(),
        BackupStatus::Completed,
        true,
    );

    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_newer_than_secs(max_age_secs)
        .with_keep_latest_always(false)
        .with_keep_latest_successful(false);

    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &HashSet::new(), now)
        .expect("eval boundary");

    let exact_dec = eval
        .decisions
        .iter()
        .find(|d| d.snapshot_id == s_exact)
        .unwrap();
    assert_eq!(
        exact_dec.action,
        RetentionAction::Keep,
        "Exact boundary must be kept"
    );

    let under_dec = eval
        .decisions
        .iter()
        .find(|d| d.snapshot_id == s_under)
        .unwrap();
    assert_eq!(
        under_dec.action,
        RetentionAction::Keep,
        "Under boundary must be kept"
    );

    let over_dec = eval
        .decisions
        .iter()
        .find(|d| d.snapshot_id == s_over)
        .unwrap();
    assert_eq!(
        over_dec.action,
        RetentionAction::Prune,
        "Over boundary must be pruned"
    );
}

#[test]
fn test_19_concurrent_operation_safety() {
    let fixture = RetentionTestFixture::new("concurrency-safety");
    let now = Utc.with_ymd_and_hms(2026, 10, 10, 12, 0, 0).unwrap();

    let _s1 = fixture.add_snapshot(
        "snap-1",
        "2026-10-01T10:00:00Z",
        BackupStatus::Completed,
        true,
    );
    let s2 = fixture.add_snapshot(
        "snap-2",
        "2026-10-02T10:00:00Z",
        BackupStatus::Completed,
        true,
    );

    // Pass s2 in active_snapshots set simulating a concurrent restore or in-flight backup
    let mut active = HashSet::new();
    active.insert(s2.clone());

    let policy = RetentionPolicy::new(fixture.profile_id.clone())
        .with_keep_latest_n(1)
        .with_keep_latest_always(false);

    let eval = fixture
        .engine
        .evaluate_retention(&fixture.profile_id, Some(&policy), &active, now)
        .expect("eval concurrent");

    let s2_dec = eval.decisions.iter().find(|d| d.snapshot_id == s2).unwrap();
    assert_eq!(s2_dec.action, RetentionAction::Keep);
    assert_eq!(s2_dec.reason, RetentionReason::ProtectedActive);
}
