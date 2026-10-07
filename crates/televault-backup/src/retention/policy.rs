//! Retention policy configuration and validation models.

use crate::error::RetentionError;
use serde::{Deserialize, Serialize};
use televault_core::ids::ProfileId;
use televault_db::RetentionPolicyRecord;

/// Typed retention policy specifying preservation and pruning rules for a backup profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Associated profile identifier.
    pub profile_id: ProfileId,
    /// Keep the latest N snapshots (if specified).
    pub keep_latest_n: Option<u32>,
    /// Keep snapshots newer than a configured age in seconds (if specified).
    pub keep_newer_than_secs: Option<u64>,
    /// Unconditional preservation of the most recent successful (Completed) snapshot.
    pub keep_latest_successful: bool,
    /// Unconditional preservation of the latest snapshot regardless of age or status.
    pub keep_latest_always: bool,
    /// Whether failed or incomplete snapshots should be pruned.
    pub prune_failed: bool,
    /// Whether snapshots with zero versions should be pruned.
    pub prune_empty: bool,
    /// Whether this retention policy is actively enabled.
    pub enabled: bool,
}

impl RetentionPolicy {
    /// Creates a default retention policy for the given profile with production defaults:
    /// - Keep latest 10 snapshots
    /// - Keep snapshots newer than 30 days (2,592,000s)
    /// - Keep latest successful snapshot
    /// - Keep latest snapshot always
    pub fn default_for_profile(profile_id: ProfileId) -> Self {
        Self {
            profile_id,
            keep_latest_n: Some(10),
            keep_newer_than_secs: Some(30 * 86400),
            keep_latest_successful: true,
            keep_latest_always: true,
            prune_failed: false,
            prune_empty: false,
            enabled: true,
        }
    }

    /// Creates an empty/unconstrained policy for a profile.
    pub fn new(profile_id: ProfileId) -> Self {
        Self {
            profile_id,
            keep_latest_n: None,
            keep_newer_than_secs: None,
            keep_latest_successful: true,
            keep_latest_always: true,
            prune_failed: false,
            prune_empty: false,
            enabled: true,
        }
    }

    /// Configures the maximum number of recent snapshots to retain.
    pub fn with_keep_latest_n(mut self, n: u32) -> Self {
        self.keep_latest_n = Some(n);
        self
    }

    /// Configures the age threshold in seconds.
    pub fn with_keep_newer_than_secs(mut self, secs: u64) -> Self {
        self.keep_newer_than_secs = Some(secs);
        self
    }

    /// Sets whether to always preserve the latest successful snapshot.
    pub fn with_keep_latest_successful(mut self, keep: bool) -> Self {
        self.keep_latest_successful = keep;
        self
    }

    /// Sets whether to always preserve the latest snapshot regardless of age.
    pub fn with_keep_latest_always(mut self, keep: bool) -> Self {
        self.keep_latest_always = keep;
        self
    }

    /// Sets whether failed snapshots are pruned.
    pub fn with_prune_failed(mut self, prune: bool) -> Self {
        self.prune_failed = prune;
        self
    }

    /// Sets whether empty snapshots are pruned.
    pub fn with_prune_empty(mut self, prune: bool) -> Self {
        self.prune_empty = prune;
        self
    }

    /// Sets whether this policy is enabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Validates policy invariant rules.
    pub fn validate(&self) -> Result<(), RetentionError> {
        if let Some(n) = self.keep_latest_n {
            if n == 0 {
                return Err(RetentionError::InvalidPolicy(
                    "keep_latest_n must be greater than zero".into(),
                ));
            }
        }

        if let Some(s) = self.keep_newer_than_secs {
            if s == 0 {
                return Err(RetentionError::InvalidPolicy(
                    "keep_newer_than_secs must be greater than zero".into(),
                ));
            }
        }

        // Must retain at least one preservation rule if enabled
        if self.enabled
            && self.keep_latest_n.is_none()
            && self.keep_newer_than_secs.is_none()
            && !self.keep_latest_successful
            && !self.keep_latest_always
        {
            return Err(RetentionError::InvalidPolicy(
                "Policy must configure at least one preservation rule (keep_latest_n, keep_newer_than_secs, keep_latest_successful, or keep_latest_always)".into(),
            ));
        }

        Ok(())
    }

    /// Converts into a database `RetentionPolicyRecord`.
    pub fn to_db_record(&self, created_at: String, updated_at: String) -> RetentionPolicyRecord {
        RetentionPolicyRecord {
            policy_id: format!("retpol-{}", self.profile_id),
            profile_id: self.profile_id.clone(),
            keep_latest_n: self.keep_latest_n,
            keep_newer_than_secs: self.keep_newer_than_secs,
            keep_latest_successful: self.keep_latest_successful,
            keep_latest_always: self.keep_latest_always,
            prune_failed: self.prune_failed,
            prune_empty: self.prune_empty,
            enabled: self.enabled,
            created_at,
            updated_at,
        }
    }

    /// Reconstructs a `RetentionPolicy` from a database record.
    pub fn from_db_record(record: &RetentionPolicyRecord) -> Self {
        Self {
            profile_id: record.profile_id.clone(),
            keep_latest_n: record.keep_latest_n,
            keep_newer_than_secs: record.keep_newer_than_secs,
            keep_latest_successful: record.keep_latest_successful,
            keep_latest_always: record.keep_latest_always,
            prune_failed: record.prune_failed,
            prune_empty: record.prune_empty,
            enabled: record.enabled,
        }
    }
}
