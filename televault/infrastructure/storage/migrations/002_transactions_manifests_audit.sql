-- Migration 002: Persistent transactions, tamper-evident manifests, audit ledger, and receipts

CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    sha256 TEXT,
    size INTEGER DEFAULT 0,
    mode TEXT DEFAULT 'original',
    state TEXT NOT NULL,
    primary_channel_id INTEGER,
    primary_message_id INTEGER,
    mirror_channel_id INTEGER,
    mirror_message_id INTEGER,
    record_id TEXT,
    manifest_generation INTEGER DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transactions_state ON transactions(state);

CREATE TABLE IF NOT EXISTS manifests (
    generation INTEGER PRIMARY KEY,
    vault_id TEXT NOT NULL,
    previous_hash TEXT NOT NULL,
    manifest_hash TEXT NOT NULL,
    record_ids TEXT NOT NULL,
    total_files INTEGER NOT NULL,
    total_bytes INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_events (
    event_id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    operation TEXT NOT NULL,
    record_id TEXT,
    result TEXT NOT NULL,
    details TEXT,
    previous_hash TEXT NOT NULL,
    current_hash TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_events(timestamp);

CREATE TABLE IF NOT EXISTS backup_receipts (
    backup_id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    files TEXT NOT NULL,
    total_size INTEGER NOT NULL,
    primary_message_id INTEGER,
    mirror_message_id INTEGER,
    sha256 TEXT NOT NULL,
    manifest_generation INTEGER NOT NULL,
    sha256_verified INTEGER DEFAULT 1
);
