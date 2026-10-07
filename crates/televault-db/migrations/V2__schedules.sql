-- TELEVAULT Schema Migration V2
-- Background scheduler and recurring backup execution tracking

CREATE TABLE schedules (
    schedule_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    schedule_type TEXT NOT NULL,
    expression TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'local',
    enabled INTEGER NOT NULL DEFAULT 1,
    next_run_at TEXT,
    last_run_at TEXT,
    last_status TEXT,
    last_error_code TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_schedules_profile_id ON schedules(profile_id);
CREATE INDEX idx_schedules_enabled_next_run ON schedules(enabled, next_run_at);

CREATE TABLE schedule_history (
    history_id TEXT PRIMARY KEY NOT NULL,
    schedule_id TEXT NOT NULL REFERENCES schedules(schedule_id) ON DELETE CASCADE,
    profile_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL,
    snapshot_id TEXT,
    files_processed INTEGER NOT NULL DEFAULT 0,
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    error_code TEXT,
    error_message TEXT
);

CREATE INDEX idx_schedule_history_schedule_id ON schedule_history(schedule_id);
CREATE INDEX idx_schedule_history_started_at ON schedule_history(started_at);
