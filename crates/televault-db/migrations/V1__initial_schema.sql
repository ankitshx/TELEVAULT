-- TELEVAULT Initial Database Schema
-- Version 1: Core Catalog, Profiles, Files, Manifests, Chunks, Snapshots, Versions, Transfer Jobs, and FTS5 Search

-- Backup profiles configured by the user
CREATE TABLE profiles (
    profile_id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    source_path TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Logical files tracked or backed up
CREATE TABLE files (
    file_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT REFERENCES profiles(profile_id) ON DELETE SET NULL,
    file_name TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    original_size INTEGER NOT NULL,
    mime_type TEXT,
    status TEXT NOT NULL,
    logical_file_hash TEXT,
    created_at TEXT NOT NULL,
    modified_at TEXT,
    created_timestamp TEXT NOT NULL,
    updated_timestamp TEXT NOT NULL,
    UNIQUE(profile_id, relative_path)
);

CREATE INDEX idx_files_profile_id ON files(profile_id);
CREATE INDEX idx_files_relative_path ON files(relative_path);
CREATE INDEX idx_files_file_name ON files(file_name);

-- Authoritative manifest records
CREATE TABLE manifests (
    manifest_id TEXT PRIMARY KEY NOT NULL,
    file_id TEXT NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
    manifest_version TEXT NOT NULL,
    serialized_manifest TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_manifests_file_id ON manifests(file_id);

-- Physical storage chunks (0..N per logical file)
CREATE TABLE chunks (
    chunk_id TEXT PRIMARY KEY NOT NULL,
    file_id TEXT NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
    manifest_id TEXT NOT NULL REFERENCES manifests(manifest_id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    plaintext_size INTEGER NOT NULL,
    stored_size INTEGER NOT NULL,
    integrity_hash TEXT NOT NULL,
    storage_reference TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(file_id, chunk_index)
);

CREATE INDEX idx_chunks_file_chunk_index ON chunks(file_id, chunk_index);
CREATE INDEX idx_chunks_manifest_id ON chunks(manifest_id);

-- Point-in-time backup snapshots
CREATE TABLE snapshots (
    snapshot_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE RESTRICT,
    status TEXT NOT NULL,
    metadata TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_snapshots_profile_id ON snapshots(profile_id);

-- File versions bound to snapshots and manifests
CREATE TABLE versions (
    version_id TEXT PRIMARY KEY NOT NULL,
    file_id TEXT NOT NULL REFERENCES files(file_id) ON DELETE RESTRICT,
    snapshot_id TEXT NOT NULL REFERENCES snapshots(snapshot_id) ON DELETE RESTRICT,
    manifest_id TEXT NOT NULL REFERENCES manifests(manifest_id) ON DELETE RESTRICT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_versions_file_id ON versions(file_id);
CREATE INDEX idx_versions_snapshot_id ON versions(snapshot_id);
CREATE INDEX idx_versions_manifest_id ON versions(manifest_id);

-- Transfer jobs for upload and download queues
CREATE TABLE transfer_jobs (
    job_id TEXT PRIMARY KEY NOT NULL,
    file_id TEXT NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
    chunk_id TEXT REFERENCES chunks(chunk_id) ON DELETE SET NULL,
    direction TEXT NOT NULL,
    status TEXT NOT NULL,
    progress INTEGER NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_transfer_jobs_file_id ON transfer_jobs(file_id);
CREATE INDEX idx_transfer_jobs_status ON transfer_jobs(status);
CREATE INDEX idx_transfer_jobs_updated_at ON transfer_jobs(updated_at);

-- FTS5 Full-Text Search Virtual Table for logical files
CREATE VIRTUAL TABLE files_fts USING fts5(
    file_id UNINDEXED,
    file_name,
    relative_path,
    tokenize='unicode61'
);

-- Triggers to synchronize files_fts with the files table
CREATE TRIGGER files_ai AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(file_id, file_name, relative_path)
    VALUES (new.file_id, new.file_name, new.relative_path);
END;

CREATE TRIGGER files_ad AFTER DELETE ON files BEGIN
    DELETE FROM files_fts WHERE file_id = old.file_id;
END;

CREATE TRIGGER files_au AFTER UPDATE ON files BEGIN
    DELETE FROM files_fts WHERE file_id = old.file_id;
    INSERT INTO files_fts(file_id, file_name, relative_path)
    VALUES (new.file_id, new.file_name, new.relative_path);
END;
