//! Production-grade retention policy orchestration and metadata pruning engine.

use crate::error::RetentionError;
use crate::retention::evaluator::RetentionEvaluator;
use crate::retention::policy::RetentionPolicy;
use crate::retention::types::{
    RetentionAction, RetentionCandidate, RetentionEvaluation, RetentionResult,
};
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::sync::Arc;
use televault_core::ids::{ProfileId, SnapshotId};
use televault_core::models::BackupStatus;
use televault_db::{Database, RetentionHistoryRecord};

/// Retention policy engine coordinating policy lifecycle, dry-run evaluations,
/// safety constraints, and transactional snapshot pruning.
pub struct RetentionEngine {
    db: Arc<Database>,
}

impl RetentionEngine {
    /// Creates a new `RetentionEngine` backed by the embedded database.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Retrieves the configured retention policy for a profile, or the default policy if none set.
    pub fn get_retention_policy(
        &self,
        profile_id: &ProfileId,
    ) -> Result<RetentionPolicy, RetentionError> {
        let record = self
            .db
            .get_retention_policy(profile_id)
            .map_err(RetentionError::from)?;

        match record {
            Some(rec) => Ok(RetentionPolicy::from_db_record(&rec)),
            None => Ok(RetentionPolicy::default_for_profile(profile_id.clone())),
        }
    }

    /// Validates and persists a retention policy for a profile.
    pub fn save_retention_policy(&self, policy: &RetentionPolicy) -> Result<(), RetentionError> {
        policy.validate()?;

        // Verify profile exists
        let profile = self
            .db
            .get_profile(&policy.profile_id)
            .map_err(RetentionError::from)?;
        if profile.is_none() {
            return Err(RetentionError::ProfileNotFound(
                policy.profile_id.to_string(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let record = policy.to_db_record(now.clone(), now);
        self.db
            .save_retention_policy(&record)
            .map_err(RetentionError::from)?;

        Ok(())
    }

    /// Deletes the custom retention policy for a profile, resetting it to default.
    pub fn delete_retention_policy(&self, profile_id: &ProfileId) -> Result<(), RetentionError> {
        self.db
            .delete_retention_policy(profile_id)
            .map_err(RetentionError::from)?;
        Ok(())
    }

    /// Lists retention execution audit history records for a profile.
    pub fn list_retention_history(
        &self,
        profile_id: &ProfileId,
        limit: usize,
    ) -> Result<Vec<RetentionHistoryRecord>, RetentionError> {
        self.db
            .list_retention_history(profile_id, limit)
            .map_err(RetentionError::from)
    }

    /// Evaluates retention policy for a profile without making any destructive modifications.
    pub fn evaluate_retention(
        &self,
        profile_id: &ProfileId,
        custom_policy: Option<&RetentionPolicy>,
        active_snapshots: &HashSet<SnapshotId>,
        now: DateTime<Utc>,
    ) -> Result<RetentionEvaluation, RetentionError> {
        let effective_policy = match custom_policy {
            Some(p) => {
                p.validate()?;
                p.clone()
            }
            None => self.get_retention_policy(profile_id)?,
        };

        let snapshots = self
            .db
            .list_snapshots_by_profile(profile_id)
            .map_err(RetentionError::from)?;

        let mut candidates = Vec::with_capacity(snapshots.len());
        for snap in snapshots {
            let version_count = self
                .db
                .count_versions_by_snapshot(&snap.snapshot_id)
                .map_err(RetentionError::from)?;

            let age_secs = parse_snapshot_age_secs(&snap.created_at, now);
            let is_active = active_snapshots.contains(&snap.snapshot_id)
                || snap.status == BackupStatus::Scanning
                || snap.status == BackupStatus::BackingUp;

            candidates.push(RetentionCandidate {
                snapshot_id: snap.snapshot_id,
                profile_id: snap.profile_id,
                created_at: snap.created_at,
                status: snap.status,
                version_count,
                is_active,
                age_secs,
            });
        }

        let evaluated_at = now.to_rfc3339();
        Ok(RetentionEvaluator::evaluate(
            profile_id,
            &effective_policy,
            &candidates,
            &evaluated_at,
        ))
    }

    /// Executes retention policy evaluation and metadata pruning for a profile.
    ///
    /// If `dry_run` is true, performs pure evaluation without deleting records.
    /// If `dry_run` is false, transactionally prunes local snapshot & version records
    /// and records an audit log in `retention_history`.
    ///
    /// Never touches remote Telegram backups.
    pub fn execute_retention(
        &self,
        profile_id: &ProfileId,
        custom_policy: Option<&RetentionPolicy>,
        active_snapshots: &HashSet<SnapshotId>,
        dry_run: bool,
        now: DateTime<Utc>,
    ) -> Result<RetentionResult, RetentionError> {
        // Verify profile exists
        let profile = self
            .db
            .get_profile(profile_id)
            .map_err(RetentionError::from)?;
        if profile.is_none() {
            return Err(RetentionError::ProfileNotFound(profile_id.to_string()));
        }

        // 1. Evaluate policy
        let evaluation =
            self.evaluate_retention(profile_id, custom_policy, active_snapshots, now)?;
        let executed_at = now.to_rfc3339();

        if dry_run {
            // Optional: record dry-run in retention history for auditability
            let pruned_ids: Vec<SnapshotId> = evaluation
                .decisions
                .iter()
                .filter(|d| d.action == RetentionAction::Prune)
                .map(|d| d.snapshot_id.clone())
                .collect();

            let history_record = RetentionHistoryRecord {
                history_id: format!("rethist-dry-{}", uuid_or_timestamp()),
                profile_id: profile_id.clone(),
                executed_at: executed_at.clone(),
                dry_run: true,
                snapshots_evaluated: evaluation.snapshots_evaluated as u32,
                snapshots_kept: evaluation.snapshots_kept as u32,
                snapshots_pruned: evaluation.snapshots_pruned as u32,
                pruned_snapshot_ids: serde_json::to_string(&pruned_ids).unwrap_or_default(),
                decisions_summary: serde_json::to_string(&evaluation.decisions).unwrap_or_default(),
                status: "completed".into(),
                error_message: None,
            };
            let _ = self.db.record_retention_history(&history_record);

            return Ok(RetentionResult {
                profile_id: profile_id.clone(),
                executed_at,
                dry_run: true,
                evaluation,
                pruned_snapshots: Vec::new(),
                pruned_versions_count: 0,
                success: true,
                error_message: None,
            });
        }

        // 2. Identify candidates approved for pruning
        let mut snapshots_to_prune = Vec::new();
        for decision in &evaluation.decisions {
            if decision.action == RetentionAction::Prune {
                // Safety invariant check: Verify snapshot is not active
                if active_snapshots.contains(&decision.snapshot_id)
                    || decision.snapshot_status == BackupStatus::Scanning
                    || decision.snapshot_status == BackupStatus::BackingUp
                {
                    return Err(RetentionError::SnapshotProtected(
                        decision.snapshot_id.to_string(),
                        "Snapshot became active during evaluation".into(),
                    ));
                }

                snapshots_to_prune.push(decision.snapshot_id.clone());
            }
        }

        // If no snapshots to prune, record clean completion and return
        if snapshots_to_prune.is_empty() {
            let history_record = RetentionHistoryRecord {
                history_id: format!("rethist-{}", uuid_or_timestamp()),
                profile_id: profile_id.clone(),
                executed_at: executed_at.clone(),
                dry_run: false,
                snapshots_evaluated: evaluation.snapshots_evaluated as u32,
                snapshots_kept: evaluation.snapshots_kept as u32,
                snapshots_pruned: 0,
                pruned_snapshot_ids: "[]".into(),
                decisions_summary: serde_json::to_string(&evaluation.decisions).unwrap_or_default(),
                status: "completed".into(),
                error_message: None,
            };
            self.db
                .record_retention_history(&history_record)
                .map_err(RetentionError::from)?;

            return Ok(RetentionResult {
                profile_id: profile_id.clone(),
                executed_at,
                dry_run: false,
                evaluation,
                pruned_snapshots: Vec::new(),
                pruned_versions_count: 0,
                success: true,
                error_message: None,
            });
        }

        // 3. Atomically prune snapshots and versions inside transaction
        let history_record = RetentionHistoryRecord {
            history_id: format!("rethist-{}", uuid_or_timestamp()),
            profile_id: profile_id.clone(),
            executed_at: executed_at.clone(),
            dry_run: false,
            snapshots_evaluated: evaluation.snapshots_evaluated as u32,
            snapshots_kept: evaluation.snapshots_kept as u32,
            snapshots_pruned: snapshots_to_prune.len() as u32,
            pruned_snapshot_ids: serde_json::to_string(&snapshots_to_prune).unwrap_or_default(),
            decisions_summary: serde_json::to_string(&evaluation.decisions).unwrap_or_default(),
            status: "completed".into(),
            error_message: None,
        };

        let pruned_versions_count = self
            .db
            .prune_snapshots_transactional(&snapshots_to_prune, Some(&history_record))
            .map_err(RetentionError::from)?;

        Ok(RetentionResult {
            profile_id: profile_id.clone(),
            executed_at,
            dry_run: false,
            evaluation,
            pruned_snapshots: snapshots_to_prune,
            pruned_versions_count,
            success: true,
            error_message: None,
        })
    }
}

/// Helper function to parse ISO-8601 creation string and compute age in seconds relative to `now`.
fn parse_snapshot_age_secs(created_at: &str, now: DateTime<Utc>) -> u64 {
    let parsed = DateTime::parse_from_rfc3339(created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(created_at, "%Y-%m-%dT%H:%M:%SZ")
                .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
        });

    match parsed {
        Ok(t) => {
            if now >= t {
                (now - t).num_seconds().max(0) as u64
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

/// Generates a simple timestamp string for unique ID generation.
fn uuid_or_timestamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}-{}", now.as_secs(), now.subsec_nanos())
}
