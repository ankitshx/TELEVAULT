-- TELEVAULT Schema Migration V4
-- Verification audit history and findings persistence

CREATE TABLE verification_history (
    history_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    level INTEGER NOT NULL,
    status TEXT NOT NULL,
    is_restore_ready INTEGER NOT NULL,
    total_files INTEGER NOT NULL DEFAULT 0,
    total_manifests INTEGER NOT NULL DEFAULT 0,
    total_chunks INTEGER NOT NULL DEFAULT 0,
    healthy_chunks INTEGER NOT NULL DEFAULT 0,
    corrupted_chunks INTEGER NOT NULL DEFAULT 0,
    missing_chunks INTEGER NOT NULL DEFAULT 0,
    ownership_violations INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    findings_json TEXT NOT NULL,
    verified_at TEXT NOT NULL
);

CREATE INDEX idx_verification_history_profile_id ON verification_history(profile_id);
CREATE INDEX idx_verification_history_target ON verification_history(target_type, target_id);
CREATE INDEX idx_verification_history_verified_at ON verification_history(verified_at);
