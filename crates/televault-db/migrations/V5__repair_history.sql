-- TELEVAULT Schema Migration V5
-- Remote repair audit history and chunk replacement tracking

CREATE TABLE repair_history (
    repair_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    snapshot_id TEXT REFERENCES snapshots(snapshot_id) ON DELETE SET NULL,
    file_id TEXT NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
    manifest_id TEXT NOT NULL REFERENCES manifests(manifest_id) ON DELETE CASCADE,
    chunk_id TEXT NOT NULL REFERENCES chunks(chunk_id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    repair_type TEXT NOT NULL,
    finding_code TEXT NOT NULL,
    old_storage_reference TEXT NOT NULL,
    new_storage_reference TEXT NOT NULL,
    status TEXT NOT NULL,
    bytes_processed INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    repaired_at TEXT NOT NULL
);

CREATE INDEX idx_repair_history_profile_id ON repair_history(profile_id);
CREATE INDEX idx_repair_history_file_id ON repair_history(file_id);
CREATE INDEX idx_repair_history_chunk_id ON repair_history(chunk_id);
CREATE INDEX idx_repair_history_repaired_at ON repair_history(repaired_at);
