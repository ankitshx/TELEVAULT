use rusqlite::Connection;
use telecloud_core::{Result, TeleCloudError};

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );

        -- Folders
        CREATE TABLE IF NOT EXISTS folders (
            id TEXT PRIMARY KEY,
            parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);

        -- Logical Files
        CREATE TABLE IF NOT EXISTS files (
            id TEXT PRIMARY KEY,
            folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL,
            name TEXT NOT NULL,
            size INTEGER NOT NULL,
            sha256 TEXT NOT NULL,
            mime_type TEXT,
            is_favorite INTEGER DEFAULT 0,
            is_encrypted INTEGER DEFAULT 0,
            manifest_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_files_folder ON files(folder_id);
        CREATE INDEX IF NOT EXISTS idx_files_sha256 ON files(sha256);
        CREATE INDEX IF NOT EXISTS idx_files_favorite ON files(is_favorite);

        -- File Chunks
        CREATE TABLE IF NOT EXISTS file_chunks (
            id TEXT PRIMARY KEY,
            file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            chunk_index INTEGER NOT NULL,
            byte_offset INTEGER NOT NULL,
            size INTEGER NOT NULL,
            sha256 TEXT NOT NULL,
            storage_provider TEXT NOT NULL,
            remote_chat_id INTEGER NOT NULL,
            remote_message_id INTEGER NOT NULL,
            is_primary INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_file_index ON file_chunks(file_id, chunk_index);

        -- Persistent Transfers
        CREATE TABLE IF NOT EXISTS transfers (
            id TEXT PRIMARY KEY,
            file_id TEXT REFERENCES files(id) ON DELETE CASCADE,
            direction TEXT NOT NULL,
            local_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            total_bytes INTEGER NOT NULL,
            transferred_bytes INTEGER NOT NULL,
            state TEXT NOT NULL,
            speed_bytes_per_sec INTEGER DEFAULT 0,
            eta_seconds INTEGER,
            active_chunk_index INTEGER,
            total_chunks INTEGER,
            error_message TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_transfers_state ON transfers(state);

        -- Favorites table
        CREATE TABLE IF NOT EXISTS favorites (
            file_id TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
            created_at TEXT NOT NULL
        );

        -- Recent items table
        CREATE TABLE IF NOT EXISTS recent_items (
            id TEXT PRIMARY KEY,
            file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            action TEXT NOT NULL,
            accessed_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_recent_accessed ON recent_items(accessed_at DESC);

        -- Manifests storage
        CREATE TABLE IF NOT EXISTS manifests (
            file_id TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
            version INTEGER NOT NULL,
            manifest_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        -- Integrity verification records
        CREATE TABLE IF NOT EXISTS integrity_records (
            file_id TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
            last_verified_at TEXT NOT NULL,
            is_valid INTEGER NOT NULL,
            details TEXT
        );

        -- App Settings & Configuration
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- Sessions
        CREATE TABLE IF NOT EXISTS sessions (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- FTS5 Full-Text Search Virtual Table
        CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
            id UNINDEXED,
            name,
            tokenize = 'unicode61'
        );

        -- Triggers to synchronize files table with FTS5 index automatically
        CREATE TRIGGER IF NOT EXISTS trg_files_ai AFTER INSERT ON files BEGIN
            INSERT INTO files_fts(id, name) VALUES (new.id, new.name);
        END;

        CREATE TRIGGER IF NOT EXISTS trg_files_ad AFTER DELETE ON files BEGIN
            DELETE FROM files_fts WHERE id = old.id;
        END;

        CREATE TRIGGER IF NOT EXISTS trg_files_au AFTER UPDATE OF name ON files BEGIN
            DELETE FROM files_fts WHERE id = old.id;
            INSERT INTO files_fts(id, name) VALUES (new.id, new.name);
        END;
        "
    ).map_err(|e| TeleCloudError::Database {
        operation: "run_migrations".to_string(),
        message: e.to_string(),
        recoverable: false,
    })?;

    Ok(())
}
