-- TELEVAULT Schema Migration V3
-- Retention policies and retention pruning history audit

CREATE TABLE retention_policies (
    policy_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    keep_latest_n INTEGER,
    keep_newer_than_secs INTEGER,
    keep_latest_successful INTEGER NOT NULL DEFAULT 1,
    keep_latest_always INTEGER NOT NULL DEFAULT 1,
    prune_failed INTEGER NOT NULL DEFAULT 0,
    prune_empty INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(profile_id)
);

CREATE INDEX idx_retention_policies_profile_id ON retention_policies(profile_id);

CREATE TABLE retention_history (
    history_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    executed_at TEXT NOT NULL,
    dry_run INTEGER NOT NULL,
    snapshots_evaluated INTEGER NOT NULL DEFAULT 0,
    snapshots_kept INTEGER NOT NULL DEFAULT 0,
    snapshots_pruned INTEGER NOT NULL DEFAULT 0,
    pruned_snapshot_ids TEXT NOT NULL,
    decisions_summary TEXT NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT
);

CREATE INDEX idx_retention_history_profile_id ON retention_history(profile_id);
CREATE INDEX idx_retention_history_executed_at ON retention_history(executed_at);
