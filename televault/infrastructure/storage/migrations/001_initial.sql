-- Initial schema for TeleVault local metadata index
CREATE TABLE IF NOT EXISTS file_records (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    original_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size INTEGER NOT NULL,
    mtime REAL NOT NULL,
    state TEXT NOT NULL,
    primary_channel_id INTEGER,
    primary_message_id INTEGER,
    mirror_channel_id INTEGER,
    mirror_message_id INTEGER,
    local_status TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    mode TEXT NOT NULL DEFAULT 'original',
    tags TEXT NOT NULL DEFAULT '[]',
    recovery_path TEXT,
    parts TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_file_records_sha256 ON file_records(sha256);
CREATE INDEX IF NOT EXISTS idx_file_records_state ON file_records(state);
CREATE INDEX IF NOT EXISTS idx_file_records_original_path ON file_records(original_path);
