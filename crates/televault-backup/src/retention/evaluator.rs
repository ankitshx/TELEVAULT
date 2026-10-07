//! Deterministic retention policy evaluation engine.

use crate::retention::policy::RetentionPolicy;
use crate::retention::types::{
    RetentionAction, RetentionCandidate, RetentionDecision, RetentionEvaluation, RetentionReason,
};
use std::cmp::Ordering;
use televault_core::ids::ProfileId;
use televault_core::models::BackupStatus;

/// Pure evaluator calculating deterministic retention decisions for snapshot candidates.
pub struct RetentionEvaluator;

impl RetentionEvaluator {
    /// Evaluates snapshot candidates against the given retention policy.
    ///
    /// Pure functional calculation: Performs ZERO database writes, ZERO network operations,
    /// and ZERO filesystem mutations.
    pub fn evaluate(
        profile_id: &ProfileId,
        policy: &RetentionPolicy,
        candidates: &[RetentionCandidate],
        evaluated_at: &str,
    ) -> RetentionEvaluation {
        // 1. Sort candidates deterministically:
        //    Primary: created_at DESC (newest first)
        //    Secondary: snapshot_id DESC
        let mut sorted = candidates.to_vec();
        sorted.sort_by(|a, b| match b.created_at.cmp(&a.created_at) {
            Ordering::Equal => b.snapshot_id.as_str().cmp(a.snapshot_id.as_str()),
            other => other,
        });

        let total = sorted.len();
        if total == 0 {
            return RetentionEvaluation {
                profile_id: profile_id.clone(),
                evaluated_at: evaluated_at.to_string(),
                policy: policy.clone(),
                decisions: Vec::new(),
                snapshots_evaluated: 0,
                snapshots_kept: 0,
                snapshots_pruned: 0,
            };
        }

        // If policy is disabled, keep all snapshots unconditionally
        if !policy.enabled {
            let decisions: Vec<_> = sorted
                .iter()
                .map(|c| RetentionDecision {
                    snapshot_id: c.snapshot_id.clone(),
                    action: RetentionAction::Keep,
                    reason: RetentionReason::KeepWithinRetentionWindow,
                    description: "Retention policy is disabled; keeping snapshot".into(),
                    snapshot_status: c.status,
                    snapshot_created_at: c.created_at.clone(),
                    age_secs: c.age_secs,
                    version_count: c.version_count,
                })
                .collect();

            return RetentionEvaluation {
                profile_id: profile_id.clone(),
                evaluated_at: evaluated_at.to_string(),
                policy: policy.clone(),
                decisions,
                snapshots_evaluated: total,
                snapshots_kept: total,
                snapshots_pruned: 0,
            };
        }

        // 2. Identify special landmark snapshots
        let latest_successful_id = sorted
            .iter()
            .find(|c| c.status == BackupStatus::Completed)
            .map(|c| c.snapshot_id.clone());

        let mut decisions = Vec::with_capacity(total);
        let mut completed_rank = 0usize;
        let mut snapshots_kept = 0usize;
        let mut snapshots_pruned = 0usize;

        for (idx, candidate) in sorted.iter().enumerate() {
            // Rule 1: Active snapshot protection (Scanning / BackingUp / Locked)
            if candidate.is_active
                || candidate.status == BackupStatus::Scanning
                || candidate.status == BackupStatus::BackingUp
            {
                decisions.push(RetentionDecision {
                    snapshot_id: candidate.snapshot_id.clone(),
                    action: RetentionAction::Keep,
                    reason: RetentionReason::ProtectedActive,
                    description: "Snapshot is actively being created or in use".into(),
                    snapshot_status: candidate.status,
                    snapshot_created_at: candidate.created_at.clone(),
                    age_secs: candidate.age_secs,
                    version_count: candidate.version_count,
                });
                snapshots_kept += 1;
                continue;
            }

            // Rule 2: Sole snapshot protection
            if total == 1 {
                decisions.push(RetentionDecision {
                    snapshot_id: candidate.snapshot_id.clone(),
                    action: RetentionAction::Keep,
                    reason: RetentionReason::ProtectedSoleSnapshot,
                    description: "Sole snapshot for profile is preserved".into(),
                    snapshot_status: candidate.status,
                    snapshot_created_at: candidate.created_at.clone(),
                    age_secs: candidate.age_secs,
                    version_count: candidate.version_count,
                });
                snapshots_kept += 1;
                continue;
            }

            // Rule 3: Keep latest snapshot always
            if idx == 0 && policy.keep_latest_always {
                if candidate.status == BackupStatus::Completed {
                    completed_rank += 1;
                }
                decisions.push(RetentionDecision {
                    snapshot_id: candidate.snapshot_id.clone(),
                    action: RetentionAction::Keep,
                    reason: RetentionReason::KeepLatest,
                    description: "Most recent snapshot for profile is preserved".into(),
                    snapshot_status: candidate.status,
                    snapshot_created_at: candidate.created_at.clone(),
                    age_secs: candidate.age_secs,
                    version_count: candidate.version_count,
                });
                snapshots_kept += 1;
                continue;
            }

            // Rule 4: Keep latest successful snapshot
            if policy.keep_latest_successful
                && latest_successful_id.as_ref() == Some(&candidate.snapshot_id)
            {
                completed_rank += 1;
                decisions.push(RetentionDecision {
                    snapshot_id: candidate.snapshot_id.clone(),
                    action: RetentionAction::Keep,
                    reason: RetentionReason::KeepLatestSuccessful,
                    description: "Latest successful (Completed) recovery point is preserved".into(),
                    snapshot_status: candidate.status,
                    snapshot_created_at: candidate.created_at.clone(),
                    age_secs: candidate.age_secs,
                    version_count: candidate.version_count,
                });
                snapshots_kept += 1;
                continue;
            }

            // Completed snapshot evaluation
            if candidate.status == BackupStatus::Completed {
                completed_rank += 1;

                // Rule 5: Prune empty completed snapshots if configured
                if policy.prune_empty && candidate.version_count == 0 {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Prune,
                        reason: RetentionReason::PruneEmpty,
                        description: "Completed snapshot contains 0 associated file versions"
                            .into(),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_pruned += 1;
                    continue;
                }

                let within_age = match policy.keep_newer_than_secs {
                    Some(max_age) => candidate.age_secs <= max_age,
                    None => false,
                };

                let within_count = match policy.keep_latest_n {
                    Some(n) => completed_rank <= n as usize,
                    None => false,
                };

                // Union evaluation: keep if either rule matches (or if no limit configured)
                if policy.keep_newer_than_secs.is_none() && policy.keep_latest_n.is_none() {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Keep,
                        reason: RetentionReason::KeepWithinRetentionWindow,
                        description: "No age or count limits configured; snapshot preserved".into(),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_kept += 1;
                } else if within_age && within_count {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Keep,
                        reason: RetentionReason::KeepWithinRetentionWindow,
                        description: format!(
                            "Within retention window (age {}s) and count limit (rank #{})",
                            candidate.age_secs, completed_rank
                        ),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_kept += 1;
                } else if within_age {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Keep,
                        reason: RetentionReason::KeepWithinRetentionWindow,
                        description: format!(
                            "Within configured age window (age {}s <= limit {}s)",
                            candidate.age_secs,
                            policy.keep_newer_than_secs.unwrap()
                        ),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_kept += 1;
                } else if within_count {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Keep,
                        reason: RetentionReason::KeepCountLimit,
                        description: format!(
                            "Within latest {} snapshots limit (rank #{})",
                            policy.keep_latest_n.unwrap(),
                            completed_rank
                        ),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_kept += 1;
                } else {
                    // Prune decision
                    let (reason, desc) = match (policy.keep_newer_than_secs, policy.keep_latest_n) {
                        (Some(_), Some(max_n)) => (
                            RetentionReason::PruneExpired,
                            format!(
                                "Age {}s exceeds limit and rank #{} exceeds limit {}",
                                candidate.age_secs, completed_rank, max_n
                            ),
                        ),
                        (Some(max_age), None) => (
                            RetentionReason::PruneExpired,
                            format!(
                                "Snapshot age {}s exceeds configured limit of {}s",
                                candidate.age_secs, max_age
                            ),
                        ),
                        (None, Some(max_n)) => (
                            RetentionReason::PruneExcessSnapshot,
                            format!(
                                "Snapshot rank #{} exceeds latest {} limit",
                                completed_rank, max_n
                            ),
                        ),
                        (None, None) => (
                            RetentionReason::PruneExcessSnapshot,
                            format!("Snapshot rank #{} pruned by default policy", completed_rank),
                        ),
                    };

                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Prune,
                        reason,
                        description: desc,
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_pruned += 1;
                }
                continue;
            }

            // Non-completed snapshots (Failed / Cancelled / Idle)
            let within_age = match policy.keep_newer_than_secs {
                Some(max_age) => candidate.age_secs <= max_age,
                None => false,
            };

            if candidate.status == BackupStatus::Failed {
                if policy.prune_failed {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Prune,
                        reason: RetentionReason::PruneFailedSnapshot,
                        description: "Failed snapshot pruned by retention policy".into(),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_pruned += 1;
                } else if within_age || policy.keep_newer_than_secs.is_none() {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Keep,
                        reason: RetentionReason::KeepWithinRetentionWindow,
                        description: "Failed snapshot preserved (prune_failed is disabled)".into(),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_kept += 1;
                } else {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Prune,
                        reason: RetentionReason::PruneFailedSnapshot,
                        description: format!(
                            "Failed snapshot exceeds retention age window ({}s)",
                            candidate.age_secs
                        ),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_pruned += 1;
                }
            } else {
                // Cancelled or Idle
                if policy.prune_failed {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Prune,
                        reason: RetentionReason::PruneIncompleteSnapshot,
                        description: format!(
                            "Incomplete snapshot ({}) pruned by retention policy",
                            candidate.status
                        ),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_pruned += 1;
                } else if within_age || policy.keep_newer_than_secs.is_none() {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Keep,
                        reason: RetentionReason::KeepWithinRetentionWindow,
                        description: format!(
                            "Incomplete snapshot ({}) preserved (prune_failed is disabled)",
                            candidate.status
                        ),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_kept += 1;
                } else {
                    decisions.push(RetentionDecision {
                        snapshot_id: candidate.snapshot_id.clone(),
                        action: RetentionAction::Prune,
                        reason: RetentionReason::PruneIncompleteSnapshot,
                        description: format!(
                            "Incomplete snapshot ({}) exceeds retention age window ({}s)",
                            candidate.status, candidate.age_secs
                        ),
                        snapshot_status: candidate.status,
                        snapshot_created_at: candidate.created_at.clone(),
                        age_secs: candidate.age_secs,
                        version_count: candidate.version_count,
                    });
                    snapshots_pruned += 1;
                }
            }
        }

        RetentionEvaluation {
            profile_id: profile_id.clone(),
            evaluated_at: evaluated_at.to_string(),
            policy: policy.clone(),
            decisions,
            snapshots_evaluated: total,
            snapshots_kept,
            snapshots_pruned,
        }
    }
}
